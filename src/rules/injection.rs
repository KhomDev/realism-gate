//! RG04-RG07: state the PoC creates instead of reaching.

use super::{Ctx, Raw, snippet};
use crate::finding::Severity;
use crate::identity::{Class, key};
use crate::ir::{Expr, ExprKind};
use crate::trace::{Call, CallClass, Op};

/// The severity for a cheat used in the exploit versus in setUp.
fn by_phase(op: &Op<'_>, exploit: Severity, setup: Severity) -> Severity {
    if op.phase.is_exploit() {
        exploit
    } else {
        setup
    }
}

fn lower(s: Severity) -> Severity {
    match s {
        Severity::Error => Severity::Warn,
        _ => Severity::Note,
    }
}

fn where_(op: &Op<'_>) -> &'static str {
    if op.phase.is_exploit() {
        "in the exploit"
    } else {
        "in setUp"
    }
}

/// Targets a cheat may rewrite without making the PoC unrealistic: the
/// attacker's own addresses, the test contract, test doubles, and helpers
/// written next to the test.
fn is_own(class: &Class) -> bool {
    matches!(
        class,
        Class::Attacker | Class::This | Class::Mock | Class::Helper
    )
}

fn idents(e: &Expr, out: &mut Vec<String>) {
    if let ExprKind::Ident(n) = &e.kind {
        out.push(n.clone());
    }
    for c in e.children() {
        idents(c, out);
    }
}

fn cheat<'a, 'p>(op: &'a Op<'p>, names: &[&str]) -> Option<&'a Call<'p>> {
    let c = op.call()?;
    (c.class == CallClass::Cheat && names.contains(&c.method.as_str())).then_some(c)
}

/// Names of the forge-std storage helper. A call chain that starts here and
/// ends in `checked_write` or `write` is a storage write.
const STDSTORE_NAMES: &[&str] = &["stdstore", "stdStorage"];

/// A storage write in this op, and the address it writes, when that address
/// is not the attacker's own.
pub(crate) fn protocol_storage_write<'a, 'p>(
    ctx: &Ctx<'p, '_>,
    op: &'a Op<'p>,
) -> Option<(&'a Call<'p>, Option<&'p Expr>, &'static str)> {
    let c = op.call()?;
    let method = c.method.as_str();
    let (kind, target) = if c.class == CallClass::Cheat && method == "store" {
        ("vm.store", c.args.first())
    } else if method == "checked_write"
        || (method == "write" && callee_mentions(c.expr, STDSTORE_NAMES))
    {
        ("stdstore", stdstore_target(c.expr))
    } else if method == "setArbitraryStorage" || method == "setArbitraryStorageAt" {
        ("setArbitraryStorage", c.args.first())
    } else if method == "copyStorage" || method == "copyStorages" {
        ("copyStorage", c.args.first())
    } else {
        return None;
    };
    if let Some(target) = target {
        let class = ctx.env.class(target);
        if is_own(&class) {
            return None;
        }
    }
    Some((c, target, kind))
}

fn callee_mentions(e: &Expr, names: &[&str]) -> bool {
    let hit = |s: &str| names.iter().any(|n| s.eq_ignore_ascii_case(n));
    match &e.kind {
        ExprKind::Ident(n) => hit(n),
        ExprKind::Member(base, m) => hit(m) || callee_mentions(base, names),
        ExprKind::Call { callee, .. } => callee_mentions(callee, names),
        _ => false,
    }
}

/// The address passed to `.target(...)` in a `stdstore` chain.
fn stdstore_target(e: &Expr) -> Option<&Expr> {
    if let ExprKind::Call { callee, args, .. } = &e.kind {
        if let ExprKind::Member(_, method) = &callee.kind
            && method == "target"
        {
            return args.first();
        }
        if let Some(found) = stdstore_target(callee) {
            return Some(found);
        }
    }
    if let ExprKind::Member(base, _) = &e.kind {
        return stdstore_target(base);
    }
    None
}

/// RG04: a direct storage write into a contract under review.
///
/// The same severity in `setUp()` and in the exploit. Moving `vm.store` into
/// `setUp()` used to drop it to a warning, which does not fail a run.
pub fn storage_write(ctx: &Ctx<'_, '_>, out: &mut Vec<Raw>) {
    for op in &ctx.trace.ops {
        let Some((c, target, kind)) = protocol_storage_write(ctx, op) else {
            continue;
        };
        let funding = kind == "vm.store" && slot_names_a_user(ctx, c);
        let mut severity = Severity::Error;
        if funding {
            severity = lower(severity);
        }
        let detail = if funding {
            " The slot names an attacker or user address, so this may be manual funding; an operator allow outside the repo under review is how to record that."
        } else {
            " The PoC has written the state it claims to reach. `setUp()` does not make the write something an attacker can do."
        };
        let who = target.map(key).unwrap_or_else(|| "a contract".to_string());
        let spelled = match kind {
            "vm.store" => "vm.store".to_string(),
            "stdstore" => "stdstore".to_string(),
            other => format!("`{other}`"),
        };
        out.push(Raw {
            rule: "RG04",
            severity,
            unit: op.unit,
            pos: op.pos,
            message: format!(
                "{spelled} writes storage of `{who}` {}, bypassing every check its code would make.{detail}",
                where_(op)
            ),
            evidence: snippet(&c.expr.text, 120),
        });
    }
}

fn slot_names_a_user(ctx: &Ctx<'_, '_>, c: &Call<'_>) -> bool {
    // The slot is usually computed into a local first:
    // `bytes32 slot = keccak256(abi.encode(attacker, 0));`. Follow locals
    // back two levels to see whose slot it is.
    let mut names = Vec::new();
    if let Some(slot) = c.args.get(1) {
        idents(slot, &mut names);
    }
    for _ in 0..2 {
        let mut more = Vec::new();
        for n in &names {
            for v in ctx.env.assigns.get(n).into_iter().flatten() {
                idents(v, &mut more);
            }
        }
        names.extend(more);
    }
    names.iter().any(|n| {
        matches!(
            ctx.env.class_of(n),
            Class::Attacker | Class::Eoa | Class::This
        )
    })
}

/// RG05: `vm.etch` or `deployCodeTo` over an address the protocol uses.
pub fn code_replacement(ctx: &Ctx<'_, '_>, out: &mut Vec<Raw>) {
    for op in &ctx.trace.ops {
        let Some(c) = op.call() else { continue };
        let target = match (&c.class, c.method.as_str()) {
            (CallClass::Cheat, "etch") => c.args.first(),
            (CallClass::StdCheat, "deployCodeTo") if c.args.len() >= 2 => c.args.last(),
            _ => continue,
        };
        let Some(target) = target else { continue };
        // Code placed at a user's address is how a harness recreates an
        // attacker's historical contract or an EIP-7702 delegation.
        let class = ctx.env.class(target);
        if is_own(&class) || class == Class::Eoa {
            continue;
        }
        out.push(Raw {
            rule: "RG05",
            // Capped at warn: on the labelled corpora it was right on 2 of 5
            // flags, below the 80% floor for an error (docs/precision.md).
            severity: Severity::Warn,
            unit: op.unit,
            pos: op.pos,
            message: format!(
                "`{}` gets replacement bytecode {}. From here on the PoC runs against code the deployed system does not have.",
                key(target),
                where_(op)
            ),
            evidence: snippet(&c.expr.text, 120),
        });
    }
}

/// RG06: `deal` into a contract under review.
pub fn balance_injection(ctx: &Ctx<'_, '_>, out: &mut Vec<Raw>) {
    for op in &ctx.trace.ops {
        let Some(c) = op.call() else { continue };
        let recipient = match (&c.class, c.method.as_str(), c.args.len()) {
            (CallClass::Cheat, "deal", 2) => &c.args[0],
            (CallClass::StdCheat, "deal", 2) => &c.args[0],
            (CallClass::StdCheat, "deal", 3 | 4) => &c.args[1],
            _ => continue,
        };
        let Class::Protocol(ty) = ctx.env.class(recipient) else {
            continue;
        };
        let asset = if c.args.len() == 2 {
            "ether".to_string()
        } else {
            format!("`{}`", key(&c.args[0]))
        };
        out.push(Raw {
            rule: "RG06",
            severity: by_phase(op, Severity::Error, Severity::Warn),
            unit: op.unit,
            pos: op.pos,
            message: format!(
                "deal() sets the {asset} balance of `{}` ({ty}) {}. It transfers nothing and overwrites the balance the contract had, so its internal accounting never sees it. Fund the attacker and transfer instead.",
                key(recipient),
                where_(op)
            ),
            evidence: snippet(&c.expr.text, 120),
        });
    }
}

/// RG07: `vm.mockCall` on a dependency.
pub fn mocked_call(ctx: &Ctx<'_, '_>, out: &mut Vec<Raw>) {
    for op in &ctx.trace.ops {
        let Some(c) = cheat(
            op,
            &["mockCall", "mockCallRevert", "mockCalls", "mockFunction"],
        ) else {
            continue;
        };
        let Some(target) = c.args.first() else {
            continue;
        };
        if is_own(&ctx.env.class(target)) {
            continue;
        }
        out.push(Raw {
            rule: "RG07",
            severity: Severity::Warn,
            unit: op.unit,
            pos: op.pos,
            message: format!(
                "vm.{} makes `{}` answer with a value its code would not produce ({}). If the impact flows through that value, the PoC assumes the dependency misbehaves; it does not show the attacker can make it.",
                c.method,
                key(target),
                where_(op)
            ),
            evidence: snippet(&c.expr.text, 120),
        });
    }
}
