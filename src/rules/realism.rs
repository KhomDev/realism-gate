//! RG15-RG18: whether the numbers and the setup are something a real
//! deployment could have produced.
//!
//! These rules are coarser than RG01-RG11. Each one states, in its page, the
//! cases where the static pattern is present and the conclusion is still
//! wrong. They do not read the chain and they do not read a protocol's docs.

use super::privilege::is_access_controlled_method;
use super::{Ctx, Raw, snippet};
use crate::finding::Severity;
use crate::identity::{Class, key};
use crate::ir::{Expr, ExprKind};
use crate::trace::{CallClass, Phase};
use regex::Regex;
use std::sync::LazyLock;

/// Raw-unit bound for a deal or a mint that is implausible on any chain:
/// 10^30. At 18 decimals that is a trillion tokens; as wei it is far more
/// ether than has ever existed. `type(uint256).max` is the same class.
const ABSURD_AMOUNT: u128 = 1_000_000_000_000_000_000_000_000_000_000; // 10^30

/// On a fork pinned to a block, a deal or mint at or above 10^27 overwrites
/// whatever that block actually held with a round fantasy number. The tool
/// cannot read the block, so the bound is fixed and documented as such.
const FORK_AMOUNT: u128 = 1_000_000_000_000_000_000_000_000_000; // 10^27

static SCIENTIFIC: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(\d[\d_]*)(?:\.(\d+))?e(\d+)").expect("scientific"));

/// The largest magnitude written in `text`, if any number or `type(..).max`
/// is there. `None` when the text has no numeric literal.
fn max_magnitude(text: &str) -> Option<u128> {
    let lower = text.to_ascii_lowercase();
    if lower.contains("type(") && lower.contains(".max") {
        return Some(u128::MAX);
    }
    let mut best: Option<u128> = None;
    let mut consumed = vec![false; text.len()];
    for m in SCIENTIFIC.find_iter(text) {
        for b in consumed.iter_mut().take(m.end()).skip(m.start()) {
            *b = true;
        }
        let caps = SCIENTIFIC.captures(m.as_str()).expect("matched");
        let mag = scientific(
            caps.get(1).unwrap().as_str(),
            caps.get(2).map(|c| c.as_str()).unwrap_or(""),
            caps.get(3).unwrap().as_str(),
        );
        if let Some(mag) = mag {
            best = Some(best.map_or(mag, |b| b.max(mag)));
        }
    }
    // Integers that were not the mantissa of a scientific literal.
    let mut digits = String::new();
    let flush = |digits: &mut String, best: &mut Option<u128>| {
        if digits.is_empty() {
            return;
        }
        let clean: String = digits.chars().filter(|c| *c != '_').collect();
        if let Ok(n) = clean.parse::<u128>() {
            *best = Some(best.map_or(n, |b| b.max(n)));
        }
        digits.clear();
    };
    for (i, ch) in text.char_indices() {
        if consumed[i] {
            flush(&mut digits, &mut best);
            continue;
        }
        if ch.is_ascii_digit() || (ch == '_' && !digits.is_empty()) {
            digits.push(ch);
        } else {
            flush(&mut digits, &mut best);
        }
    }
    flush(&mut digits, &mut best);
    best
}

fn scientific(int_part: &str, frac: &str, exp: &str) -> Option<u128> {
    let int_part: String = int_part.chars().filter(|c| *c != '_').collect();
    let exp: u32 = exp.parse().ok()?;
    let digits = format!("{int_part}{frac}");
    let digits = digits.trim_start_matches('0');
    if digits.is_empty() {
        return Some(0);
    }
    let scale = exp as i32 - frac.len() as i32;
    if scale < 0 {
        let keep = digits.len() as i32 + scale;
        if keep <= 0 {
            return Some(0);
        }
        return digits[..keep as usize].parse().ok();
    }
    let n: u128 = digits.parse().ok()?;
    n.checked_mul(10u128.checked_pow(scale as u32)?)
}

fn is_profit_word(name: &str) -> bool {
    let words = crate::identity::words(name);
    if words
        .iter()
        .any(|w| w == "profit" || w == "pnl" || w == "proceeds" || w == "extracted")
    {
        return true;
    }
    // `gain` but not `feeGain`: a fee measurement is not an attacker profit.
    words.iter().any(|w| w == "gain") && words.first().is_none_or(|w| w != "fee")
}

fn expr_is_profit(e: &Expr) -> bool {
    let mut names = Vec::new();
    idents(e, &mut names);
    names.iter().any(|n| is_profit_word(n)) || is_profit_word(&e.text)
}

fn idents(e: &Expr, out: &mut Vec<String>) {
    if let ExprKind::Ident(n) = &e.kind {
        out.push(n.clone());
    }
    for c in e.children() {
        idents(c, out);
    }
}

fn definition_text(ctx: &Ctx<'_, '_>, e: &Expr) -> String {
    let mut names = Vec::new();
    idents(e, &mut names);
    let mut parts = vec![e.text.clone()];
    for n in names {
        if let Some(values) = ctx.env.assigns.get(&n) {
            for v in values {
                parts.push(v.text.clone());
            }
        }
    }
    parts.join(" ")
}

fn is_cost_name(name: &str) -> bool {
    crate::identity::words(name).iter().any(|w| {
        matches!(
            w.as_str(),
            "fee" | "fees" | "slippage" | "gasprice" | "protocolfee"
        )
    })
}

/// A fee the test computes (`fee = amount * bps / DENOM`), not a fee tier
/// constant and not the word "fee" inside `SupportingFeeOnTransfer`.
fn cost_applied_to_profit(ctx: &Ctx<'_, '_>, cost: &str) -> bool {
    ctx.trace.ops.iter().any(|op| {
        let text = match &op.kind {
            crate::trace::OpKind::Call(c) => c.expr.text.as_str(),
            crate::trace::OpKind::Assign { value: Some(v), .. } => v.text.as_str(),
            _ => return false,
        };
        text.contains(cost)
            && crate::identity::words(text).iter().any(|w| {
                matches!(
                    w.as_str(),
                    "profit" | "pnl" | "proceeds" | "extracted" | "gain"
                )
            })
    })
}

fn computed_costs(ctx: &Ctx<'_, '_>) -> Vec<String> {
    let mut names = Vec::new();
    for op in &ctx.trace.ops {
        match &op.kind {
            crate::trace::OpKind::Assign {
                target,
                value: Some(v),
                ..
            } if is_cost_name(target) => {
                let t = v.text.as_str();
                if t.contains('*') || t.contains('/') || t.contains('-') {
                    names.push(target.clone());
                }
            }
            crate::trace::OpKind::Call(c) if c.method == "gasleft" => names.push("gas".to_string()),
            _ => {}
        }
    }
    names
}

fn nets_cost(text: &str, costs: &[String]) -> bool {
    let l = text.to_ascii_lowercase();
    let subtracts = l.contains('-') || l.contains("sub(");
    if !subtracts {
        return false;
    }
    costs.iter().any(|c| text.contains(c.as_str())) || l.contains("gas")
}

fn fee_repaid(text: &str) -> bool {
    let l = text.to_ascii_lowercase();
    l.contains("+ fee") || l.contains("+fee") || l.contains("+  fee")
}

fn trace_text(ctx: &Ctx<'_, '_>) -> String {
    ctx.trace
        .ops
        .iter()
        .filter_map(|op| match &op.kind {
            crate::trace::OpKind::Call(c) => Some(c.expr.text.clone()),
            crate::trace::OpKind::Assign { target, value, .. } => {
                let mut s = target.clone();
                if let Some(v) = value {
                    s.push(' ');
                    s.push_str(&v.text);
                }
                Some(s)
            }
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// A sibling test asserts the flow loses money once a fee applies. The
/// postmortem shape: one test asserts a profit with no fee instruction, and
/// another asserts the taker delta is negative "under fee drag".
fn file_concedes_fee_loss(source: &str) -> bool {
    let l = source.to_ascii_lowercase();
    // Phrases, not a bare `assertLt` next to the word fee. An upper bound on
    // a profit (`assertLt(profit, 2 ether)`) beside a fee-on-transfer router
    // is still a profit.
    l.contains("fee drag")
        || l.contains("must not profit")
        || l.contains("under fee")
        || (l.contains("not profit") && l.contains("fee"))
}

/// RG15: a profit assertion that ignores a fee or a gas cost the test itself
/// computes, or that a sibling test shows wipes the profit out.
pub fn profit_ignores_costs(ctx: &Ctx<'_, '_>, out: &mut Vec<Raw>) {
    let body = trace_text(ctx);
    let repaid = fee_repaid(&body);
    let source = &ctx.project.units[ctx.suite.unit].source;
    let concession = file_concedes_fee_loss(source);
    let costs = computed_costs(ctx);
    // A cost that appears in the same expression as the profit (another
    // assertion subtracting it, for example) is already in the claim.
    let costs: Vec<String> = costs
        .into_iter()
        .filter(|c| !cost_applied_to_profit(ctx, c))
        .collect();
    if repaid || (costs.is_empty() && !concession) {
        return;
    }
    for op in &ctx.trace.ops {
        if !op.phase.is_exploit() {
            continue;
        }
        let Some(c) = op.call() else { continue };
        if !matches!(
            c.method.as_str(),
            "assertGt" | "assertGtDecimal" | "assertGe" | "assertGeDecimal"
        ) {
            continue;
        }
        let Some(arg) = c.args.first() else { continue };
        if !expr_is_profit(arg) {
            continue;
        }
        let defs = definition_text(ctx, arg);
        if nets_cost(&defs, &costs) {
            continue;
        }
        // Gas is a different asset from a token balance. Only require it to
        // be subtracted when the profit itself is an ether balance.
        let ether_profit =
            defs.contains(".balance") && !defs.to_ascii_lowercase().contains("balanceof");
        let gas_only = !costs.is_empty() && costs.iter().all(|n| n == "gas");
        if gas_only && !ether_profit && !concession {
            continue;
        }
        let why = if concession && costs.is_empty() {
            "another test in this file says the flow does not profit once a fee applies, and this assertion does not subtract that fee"
        } else if gas_only {
            "the test reads gas and the profit is an ether balance that does not subtract it"
        } else {
            "the test computes a fee, slippage or gas cost that this profit expression does not subtract"
        };
        out.push(Raw {
            rule: "RG15",
            severity: Severity::Warn,
            unit: op.unit,
            pos: op.pos,
            message: format!(
                "The profit assertion is not net of costs: {why}. A balance read is often already net of a pool's fee; this fires because the test itself introduces a cost and then asserts a profit that ignores it."
            ),
            evidence: snippet(&c.expr.text, 120),
        });
    }
}

fn is_value_injection(method: &str, class: &CallClass) -> bool {
    let lower = method.to_ascii_lowercase();
    matches!(
        (class, lower.as_str()),
        (CallClass::StdCheat | CallClass::Cheat, "deal")
            | (CallClass::External, "mint" | "mintto" | "_mint")
    )
}

fn fork_is_pinned(ctx: &Ctx<'_, '_>) -> bool {
    ctx.trace.ops.iter().any(|op| {
        op.call().is_some_and(|c| {
            matches!(
                c.method.as_str(),
                "createSelectFork" | "createFork" | "rollFork"
            ) && c.args.len() >= 2
        })
    })
}

/// RG16: a deal or mint whose amount is implausible, either absolutely or
/// because a pinned fork's real balance is being overwritten with a huge
/// round number. The bound is fixed; the chain is not read.
pub fn implausible_amount(ctx: &Ctx<'_, '_>, out: &mut Vec<Raw>) {
    let pinned = fork_is_pinned(ctx);
    for op in &ctx.trace.ops {
        let Some(c) = op.call() else { continue };
        if !is_value_injection(&c.method, &c.class) {
            continue;
        }
        let text = c
            .args
            .last()
            .map(|a| a.text.as_str())
            .unwrap_or(c.expr.text.as_str());
        let Some(mag) = max_magnitude(text) else {
            continue;
        };
        let absurd = mag >= ABSURD_AMOUNT;
        let fork_large = pinned && mag >= FORK_AMOUNT;
        if !absurd && !fork_large {
            continue;
        }
        let why = if absurd {
            "the amount is at least 10^30 raw units (or `type(uint256).max`), which is not a plausible balance or trade size"
                .to_string()
        } else {
            "the test pins a fork and then deals or mints at least 10^27 raw units, overwriting whatever that block held. The tool does not read the chain, so it cannot check the real balance; the bound is the fixed one documented for this rule".to_string()
        };
        out.push(Raw {
            rule: "RG16",
            severity: Severity::Warn,
            unit: op.unit,
            pos: op.pos,
            message: format!("`{}` injects an implausible amount: {why}.", c.method),
            evidence: snippet(&c.expr.text, 120),
        });
    }
}

/// Protocol actions that set up somebody else's behavior: a maker shipping a
/// strategy, a dock, a push that is not the attacker's own transfer. Ordinary
/// user calls (deposit, swap, transfer) are not in this list.
const SETUP_ACTIONS: &[&str] = &[
    "ship",
    "dock",
    "undock",
    "push",
    "addstrategy",
    "setstrategy",
    "registerstrategy",
    "setroute",
    "sethook",
    "addhook",
    "registermaker",
    "setmaker",
];

fn is_setup_action(method: &str) -> bool {
    let lower = method.to_ascii_lowercase();
    SETUP_ACTIONS.contains(&lower.as_str())
}

fn is_stand_in(ty: &str) -> bool {
    let words = crate::identity::words(ty);
    words.iter().any(|w| {
        matches!(
            w.as_str(),
            "strategy" | "composite" | "malicious" | "hook" | "route"
        )
    })
}

fn evidenced(ctx: &Ctx<'_, '_>, name: &str) -> bool {
    ctx.env
        .deployment_evidence
        .iter()
        .any(|e| e.eq_ignore_ascii_case(name))
}

/// RG17: the test builds a victim's or a third party's behavior, or deploys a
/// stand-in for it, and `deployment.evidence` does not cover that contract
/// or actor.
pub fn unevidenced_setup(ctx: &Ctx<'_, '_>, out: &mut Vec<Raw>) {
    let env = ctx.env;
    for (i, op) in ctx.trace.ops.iter().enumerate() {
        let Some(c) = op.call() else { continue };
        if let CallClass::Deploy(ty) = &c.class {
            if !is_stand_in(ty) {
                continue;
            }
            if evidenced(ctx, ty) {
                continue;
            }
            out.push(Raw {
                rule: "RG17",
                severity: Severity::Warn,
                unit: op.unit,
                pos: op.pos,
                message: format!(
                    "The test deploys `{ty}`, a stand-in for someone else's contract or strategy, and `deployment.evidence` in realism.yaml does not cover it. The PoC shows what this harness constructed, not a deployment or a user the evidence points at."
                ),
                evidence: snippet(&c.expr.text, 120),
            });
            continue;
        }
        if c.class != CallClass::External || !is_setup_action(&c.method) {
            continue;
        }
        let Some(receiver) = c.receiver else { continue };
        // `array.push` is not a protocol action. A dynamic array's type
        // contains `[]`; the name heuristic would otherwise read the element
        // type as a protocol contract.
        if !matches!(env.class(receiver), Class::Protocol(_)) {
            continue;
        }
        let rk = key(receiver);
        if env.type_of(&rk).is_some_and(|t| t.contains('['))
            || matches!(c.method.as_str(), "push" | "pop")
                && c.receiver.is_some_and(|r| r.text.contains('['))
        {
            continue;
        }
        let sender = ctx.sender_key(i).unwrap_or_else(|| "this".to_string());
        if evidenced(ctx, &rk) || evidenced(ctx, &sender) || evidenced(ctx, &c.method) {
            continue;
        }
        // An access-controlled setter done in setUp by the deployer is
        // configuration, which RG02 already leaves alone. `ship` / `dock` /
        // `push` are the actions a test uses to invent a counterparty.
        if op.phase == Phase::Setup && is_access_controlled_method(&c.method) {
            continue;
        }
        let actor = if sender == "this" {
            "the test contract".to_string()
        } else {
            format!("`{sender}`")
        };
        let method = &c.method;
        out.push(Raw {
            rule: "RG17",
            severity: Severity::Warn,
            unit: op.unit,
            pos: op.pos,
            message: format!(
                "{actor} calls `{rk}.{method}`, which sets up a counterparty's behavior, and `deployment.evidence` does not cover `{rk}` or `{sender}`. Nothing in the test shows this is how a real deployment or a real user behaves. The tool cannot read the protocol's docs; cite them under `deployment.evidence` if they do."
            ),
            evidence: snippet(&c.expr.text, 120),
        });
    }
}

fn is_revert_assertion(method: &str) -> bool {
    matches!(
        method,
        "expectRevert" | "expectPartialRevert" | "expectRevertCall"
    )
}

/// RG18: the only assertion is that a call reverts. A revert is not an impact.
pub fn revert_only(ctx: &Ctx<'_, '_>, out: &mut Vec<Raw>) {
    let test = ctx.trace.test;
    if test.name.starts_with("testFail") {
        return;
    }
    let mut reverts: Vec<usize> = Vec::new();
    let mut other = 0usize;
    for (i, op) in ctx.trace.ops.iter().enumerate() {
        if !op.phase.is_exploit() {
            continue;
        }
        let Some(c) = op.call() else { continue };
        if c.class != CallClass::Assert {
            continue;
        }
        if is_revert_assertion(&c.method) {
            reverts.push(i);
        } else {
            other += 1;
        }
    }
    if reverts.is_empty() || other > 0 {
        return;
    }
    let i = reverts[0];
    let op = &ctx.trace.ops[i];
    out.push(Raw {
        rule: "RG18",
        severity: Severity::Warn,
        unit: op.unit,
        pos: op.pos,
        message: format!(
            "`{}` only asserts that a call reverts ({} expectRevert). A revert shows the call failed. It does not show that value moved, that funds are stuck, or that anyone lost anything. Assert the impact, or the finding is a denial of service with no demonstrated loss.",
            test.name,
            reverts.len()
        ),
        evidence: op
            .call()
            .map(|c| snippet(&c.expr.text, 120))
            .unwrap_or_default(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn magnitudes() {
        assert_eq!(
            max_magnitude("1_000e18"),
            Some(1_000_000_000_000_000_000_000)
        );
        assert_eq!(
            max_magnitude("10_000_000e18"),
            Some(10_000_000_000_000_000_000_000_000)
        );
        assert!(max_magnitude("1e30").unwrap() >= ABSURD_AMOUNT);
        assert!(max_magnitude("type(uint256).max").unwrap() >= ABSURD_AMOUNT);
        assert!(max_magnitude("1000e18").unwrap() < FORK_AMOUNT);
        assert_eq!(max_magnitude("no numbers here"), None);
    }

    #[test]
    fn profit_words_ignore_a_fee_measurement() {
        assert!(is_profit_word("takerProfit"));
        assert!(is_profit_word("profit"));
        assert!(!is_profit_word("feeGainA"));
        assert!(!is_profit_word("balanceOf"));
    }
}
