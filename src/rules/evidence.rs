//! RG08-RG11: whether the test actually demonstrates anything.

use super::privilege::is_read_method;
use super::{Ctx, Raw, snippet};
use crate::finding::Severity;
use crate::identity::{compact, key};
use crate::ir::{Expr, ExprKind, Lit};
use crate::trace::{Call, CallClass, OpKind, Phase};
use std::collections::{BTreeMap, BTreeSet};

/// Methods whose result is evidence of value moving. Matched as lowercase
/// prefixes.
const MEASURE_PREFIXES: &[&str] = &[
    "balanceof",
    "balance",
    "getbalance",
    "balances",
    "totalsupply",
    "totalassets",
    "getreserves",
    "reserves",
    "convertto",
    "preview",
    "maxwithdraw",
    "maxredeem",
    "totaldebt",
    "totalborrow",
    "borrowbalance",
    "debtof",
    "sharesof",
    "getpriceperfullshare",
    "pricepershare",
    "exchangerate",
    "getvirtualprice",
    "earned",
    "claimable",
    "getaccountsnapshot",
    "getaccountliquidity",
    "userinfo",
];

fn is_measure(method: &str) -> bool {
    let lower = method.to_ascii_lowercase();
    MEASURE_PREFIXES.iter().any(|p| lower.starts_with(p))
}

const COMPARE_OPS: &[&str] = &["==", "!=", "<", "<=", ">", ">="];

fn is_log_call(c: &Call<'_>) -> bool {
    c.class == CallClass::Log
}

/// RG08: no assertion anywhere in the test, its helpers or its callbacks.
pub fn no_assertion(ctx: &Ctx<'_, '_>, out: &mut Vec<Raw>) {
    let test = ctx.trace.test;
    if test.name.starts_with("testFail") {
        return;
    }
    let mut asserts = 0;
    let mut logs = 0;
    for op in &ctx.trace.ops {
        if !op.phase.is_exploit() {
            continue;
        }
        match &op.kind {
            OpKind::Call(c) if c.class == CallClass::Assert => asserts += 1,
            OpKind::Call(c) if is_log_call(c) => logs += 1,
            OpKind::Emit { event, .. } if event.starts_with("log") => logs += 1,
            _ => {}
        }
    }
    if asserts > 0 {
        return;
    }
    let (severity, message) = if logs > 0 {
        (
            Severity::Warn,
            format!(
                "`{}` logs {logs} value(s) but asserts none. The test passes whatever those values are; a reader has to trust the numbers.",
                test.name
            ),
        )
    } else {
        (
            Severity::Error,
            format!(
                "`{}` contains no assertion and logs nothing. It passes whatever the protocol does.",
                test.name
            ),
        )
    };
    out.push(Raw {
        rule: "RG08",
        severity,
        unit: ctx.test_unit,
        pos: test.pos,
        message,
        evidence: format!("function {}(...)", test.name),
    });
}

/// RG09: an assertion that cannot fail.
pub fn tautological_assertion(ctx: &Ctx<'_, '_>, out: &mut Vec<Raw>) {
    for op in &ctx.trace.ops {
        if !op.phase.is_exploit() {
            continue;
        }
        let Some(c) = op.call() else { continue };
        if c.class != CallClass::Assert {
            continue;
        }
        let reason = match c.method.as_str() {
            "assertTrue" | "assertFalse" | "assert" | "require" => c
                .args
                .first()
                .and_then(|cond| tautology_of_condition(ctx, cond)),
            m if m.starts_with("assert") && c.args.len() >= 2 => {
                tautology_of_pair(ctx, &c.args[0], &c.args[1])
            }
            _ => None,
        };
        let Some(reason) = reason else { continue };
        out.push(Raw {
            rule: "RG09",
            severity: Severity::Error,
            unit: op.unit,
            pos: op.pos,
            message: format!("This assertion cannot fail: {reason}."),
            evidence: snippet(&c.expr.text, 120),
        });
    }
}

fn tautology_of_pair(ctx: &Ctx<'_, '_>, a: &Expr, b: &Expr) -> Option<String> {
    if compact(&a.text) == compact(&b.text) {
        return Some(format!("both sides are `{}`", compact(&a.text)));
    }
    if ctx.env.is_constant(a) && ctx.env.is_constant(b) {
        return Some(format!(
            "`{}` and `{}` are both values the test wrote itself",
            compact(&a.text),
            compact(&b.text)
        ));
    }
    None
}

fn tautology_of_condition(ctx: &Ctx<'_, '_>, cond: &Expr) -> Option<String> {
    match &cond.kind {
        ExprKind::Lit(Lit::Bool(true)) => Some("the condition is the literal `true`".to_string()),
        ExprKind::Binary(l, op, r) if COMPARE_OPS.contains(&op.as_str()) => {
            tautology_of_pair(ctx, l, r)
        }
        _ if ctx.env.is_constant(cond) => Some(format!("`{}` is a constant", compact(&cond.text))),
        _ => None,
    }
}

/// Measurement reads inside an expression: calls to value-reading methods and
/// `.balance` member reads. Maps the read's canonical text to its subject:
/// whose balance or state it reads.
fn reads_in(e: &Expr, out: &mut BTreeMap<String, String>) {
    match &e.kind {
        ExprKind::Call { callee, args, .. } => {
            if let ExprKind::Member(receiver, m) = &callee.kind
                && is_measure(m)
            {
                let subject = args.first().map(key).unwrap_or_else(|| key(receiver));
                out.insert(compact(&e.text), subject);
            }
        }
        ExprKind::Member(base, m) if m == "balance" => {
            out.insert(compact(&e.text), key(base));
        }
        _ => {}
    }
    for c in e.children() {
        reads_in(c, out);
    }
}

/// `.balance` reads in an expression, without descending into nested calls:
/// a nested call is its own op and its arguments are read there.
fn balance_reads_shallow(e: &Expr, out: &mut BTreeSet<String>) {
    match &e.kind {
        ExprKind::Member(_, m) if m == "balance" => {
            out.insert(compact(&e.text));
        }
        ExprKind::Call { .. } => {}
        _ => {
            for c in e.children() {
                balance_reads_shallow(c, out);
            }
        }
    }
}

fn nested_calls(e: &Expr, out: &mut BTreeSet<*const Expr>) {
    if matches!(e.kind, ExprKind::Call { .. }) {
        out.insert(e as *const Expr);
    }
    for c in e.children() {
        nested_calls(c, out);
    }
}

fn idents_in(e: &Expr, out: &mut BTreeSet<String>) {
    if let ExprKind::Ident(n) = &e.kind {
        out.insert(n.clone());
    }
    for c in e.children() {
        idents_in(c, out);
    }
}

fn is_zero(e: &Expr) -> bool {
    matches!(e.kind, ExprKind::Lit(Lit::Number)) && e.text.trim() == "0"
}

/// Whether an assertion makes a directional claim: "more than", "less than".
/// Those are the claims a pre-existing balance can satisfy on its own.
/// Equality to an exact value is not one (a starting balance would break it),
/// and neither is an upper bound of zero ("drained").
/// `require` is left out: in a test it is almost always a precondition guard.
fn is_directional_claim(c: &Call<'_>) -> bool {
    let directional = |op: &str, a: &Expr, b: &Expr| match op {
        ">" | ">=" => true,
        "<" | "<=" => !(is_zero(a) || is_zero(b)),
        _ => false,
    };
    match c.method.as_str() {
        "assertTrue" | "assert" => match c.args.first().map(|e| &e.kind) {
            Some(ExprKind::Binary(l, op, r)) => directional(op, l, r),
            _ => false,
        },
        "assertGt" | "assertGtDecimal" if c.args.len() >= 2 => true,
        "assertGe" | "assertGeDecimal" if c.args.len() >= 2 => true,
        "assertLt" | "assertLtDecimal" if c.args.len() >= 2 => {
            directional("<", &c.args[0], &c.args[1])
        }
        "assertLe" | "assertLeDecimal" if c.args.len() >= 2 => {
            directional("<=", &c.args[0], &c.args[1])
        }
        _ => false,
    }
}

/// One directional assertion and the measurements it depends on.
struct Claim {
    op: usize,
    /// read text -> subject
    reads: BTreeMap<String, String>,
}

/// RG10: a directional assertion whose every measurement is read once, after
/// the attack.
pub fn no_baseline(ctx: &Ctx<'_, '_>, out: &mut Vec<Raw>) {
    let ops = &ctx.trace.ops;
    // Every measurement read, in order: (op index, canonical text).
    let mut reads: Vec<(usize, String)> = Vec::new();
    // Ops that change state: deployments and external calls that are not reads.
    let mut writes: Vec<usize> = Vec::new();
    // Which measurements each local was computed from.
    let mut provenance: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    // Contracts the harness deploys: they start with nothing, so an absolute
    // read of them is already a difference.
    let mut fresh: BTreeSet<String> = BTreeSet::new();
    // Addresses the harness funds with a cheat; never "fresh".
    let mut dealt: BTreeSet<String> = BTreeSet::new();
    let mut claims: Vec<Claim> = Vec::new();
    // A call written inside an assertion or a log is a read, whatever its name.
    let mut observed: BTreeSet<*const Expr> = BTreeSet::new();
    for op in ops.iter() {
        let args: &[Expr] = match &op.kind {
            OpKind::Call(c) if matches!(c.class, CallClass::Assert | CallClass::Log) => c.args,
            OpKind::Emit { args, .. } => args,
            _ => continue,
        };
        for a in args {
            nested_calls(a, &mut observed);
        }
    }

    for (i, op) in ops.iter().enumerate() {
        match &op.kind {
            OpKind::Call(c) => {
                let mut here = BTreeSet::new();
                let is_observed = observed.contains(&(c.expr as *const Expr));
                match &c.class {
                    CallClass::External if is_measure(&c.method) => {
                        here.insert(compact(&c.expr.text));
                    }
                    CallClass::External if !is_read_method(&c.method) && !is_observed => {
                        writes.push(i)
                    }
                    CallClass::Deploy(_) => writes.push(i),
                    CallClass::Cheat | CallClass::StdCheat
                        if matches!(c.method.as_str(), "deal" | "hoax" | "startHoax") =>
                    {
                        let to = if c.method == "deal" && c.args.len() >= 3 {
                            c.args.get(1)
                        } else {
                            c.args.first()
                        };
                        if let Some(to) = to {
                            dealt.insert(key(to));
                        }
                    }
                    _ => {}
                }
                for a in c.args {
                    balance_reads_shallow(a, &mut here);
                }
                for r in here {
                    reads.push((i, r));
                }
                if c.class == CallClass::Assert
                    && op.phase == Phase::Test
                    && is_directional_claim(c)
                {
                    let mut used = BTreeMap::new();
                    let mut names = BTreeSet::new();
                    for a in c.args {
                        reads_in(a, &mut used);
                        idents_in(a, &mut names);
                    }
                    for n in names {
                        if let Some(p) = provenance.get(&n) {
                            used.extend(p.iter().map(|(k, v)| (k.clone(), v.clone())));
                        }
                    }
                    if !used.is_empty() {
                        claims.push(Claim { op: i, reads: used });
                    }
                }
            }
            OpKind::Assign {
                target,
                value: Some(value),
                ..
            } => {
                if ctx.env.deployed_type(target).is_some() {
                    fresh.insert(target.clone());
                }
                let mut from = BTreeMap::new();
                reads_in(value, &mut from);
                let mut here = BTreeSet::new();
                balance_reads_shallow(value, &mut here);
                for r in here {
                    reads.push((i, r));
                }
                let mut names = BTreeSet::new();
                idents_in(value, &mut names);
                for n in names {
                    if let Some(p) = provenance.get(&n) {
                        from.extend(p.iter().map(|(k, v)| (k.clone(), v.clone())));
                    }
                }
                provenance.insert(target.clone(), from);
            }
            OpKind::Emit { args, .. } => {
                let mut here = BTreeSet::new();
                for a in args.iter() {
                    balance_reads_shallow(a, &mut here);
                }
                for r in here {
                    reads.push((i, r));
                }
            }
            _ => {}
        }
    }

    let first_test_write = writes
        .iter()
        .copied()
        .find(|&w| ops[w].phase == Phase::Test);
    let occurrences = |r: &str, until: usize| -> Vec<usize> {
        reads
            .iter()
            .filter(|(i, t)| *i <= until && t == r)
            .map(|(i, _)| *i)
            .collect()
    };
    let has_baseline = |r: &str, until: usize| {
        let at = occurrences(r, until);
        match (at.first(), at.last()) {
            (Some(&first), Some(&last)) if first < last => {
                writes.iter().any(|&w| w > first && w < last)
            }
            _ => false,
        }
    };
    let mut relevant = Vec::new();
    for claim in &claims {
        // A check made before the exploit changes anything is a precondition
        // ("the victim holds funds"), not a claim about the exploit's effect.
        let after_attack = claim.reads.keys().any(|r| {
            let last = occurrences(r, claim.op).last().copied().unwrap_or(claim.op);
            first_test_write.is_some_and(|w| last > w)
        });
        if !after_attack {
            continue;
        }
        let baselined = claim.reads.iter().any(|(r, subject)| {
            has_baseline(r, claim.op) || (fresh.contains(subject) && !dealt.contains(subject))
        });
        if baselined {
            return;
        }
        relevant.push(claim);
    }
    let Some(claim) = relevant.first() else {
        return;
    };
    let op = &ops[claim.op];
    let shown: Vec<String> = claim
        .reads
        .keys()
        .take(2)
        .map(|r| format!("`{r}`"))
        .collect();
    out.push(Raw {
        rule: "RG10",
        severity: Severity::Warn,
        unit: op.unit,
        pos: op.pos,
        message: format!(
            "The asserted value {} is read only after the attack. Without a before-value the test cannot tell a gain from a balance that was already there.",
            shown.join(" and ")
        ),
        evidence: op
            .call()
            .map(|c| snippet(&c.expr.text, 120))
            .unwrap_or_default(),
    });
}

/// RG11: a fork created without a block number and never rolled to one.
pub fn unpinned_fork(ctx: &Ctx<'_, '_>, out: &mut Vec<Raw>) {
    let ops = &ctx.trace.ops;
    for (i, op) in ops.iter().enumerate() {
        let Some(c) = op.call() else { continue };
        if c.class != CallClass::Cheat
            || !matches!(c.method.as_str(), "createSelectFork" | "createFork")
            || c.args.len() != 1
        {
            continue;
        }
        let rolled = ops[i + 1..].iter().any(|later| {
            later.call().is_some_and(|l| {
                l.class == CallClass::Cheat && l.method == "rollFork" && !l.args.is_empty()
            })
        });
        if rolled {
            continue;
        }
        out.push(Raw {
            rule: "RG11",
            severity: Severity::Warn,
            unit: op.unit,
            pos: op.pos,
            message: "The fork has no block number, so it runs against the chain as it is today. Balances, prices and the bug itself can change before a triager runs it.".to_string(),
            evidence: snippet(&c.expr.text, 120),
        });
    }
}
