//! RG01-RG03: who the exploit runs as.

use super::{Ctx, Raw, Sender, snippet};
use crate::finding::Severity;
use crate::identity::{Class, compact, is_privileged_getter, key};
use crate::ir::ExprKind;
use crate::trace::{CallClass, OpKind, Phase};
use std::collections::{BTreeMap, BTreeSet};

/// Methods that give a contract's control to someone else, or give it up.
const HANDOVER_METHODS: &[&str] = &[
    "transferOwnership",
    "renounceOwnership",
    "renounceRole",
    "revokeRole",
    "changeAdmin",
    "transferAdmin",
    "setOwner",
    "setAdmin",
    "setGovernance",
    "beginDefaultAdminTransfer",
];

/// Method-name prefixes that read state without changing it. Used only to
/// leave reads out of "calls made as the deployer".
const READ_PREFIXES: &[&str] = &[
    "get",
    "balance",
    "total",
    "preview",
    "convert",
    "is",
    "has",
    "can",
    "max",
    "decimals",
    "symbol",
    "name",
    "allowance",
    "owner",
    "price",
    "quote",
    "calc",
    "compute",
    "view",
    "read",
    "peek",
    "latest",
    "current",
    "reserves",
    "supply",
    "nonces",
    "domain",
    "version",
];

pub fn is_read_method(method: &str) -> bool {
    let lower = method.to_ascii_lowercase();
    READ_PREFIXES.iter().any(|p| lower.starts_with(p))
        || method.chars().all(|c| !c.is_ascii_lowercase())
}

/// Methods whose names say they are gated: an owner sweep, a setter, a
/// pause, an upgrade. A local `new Vault()` followed by `deposit` is the
/// ordinary test harness; `sweep` from that same sender is the deployer
/// using a privilege. The name is all a static read can see.
pub fn is_access_controlled_method(method: &str) -> bool {
    if method.is_empty() || is_read_method(method) {
        return false;
    }
    let lower = method.to_ascii_lowercase();
    const EXACT: &[&str] = &[
        "sweep",
        "rescue",
        "recover",
        "pause",
        "unpause",
        "upgrade",
        "upgradeto",
        "upgradetoandcall",
        "kill",
        "shutdown",
        "migrate",
        "authorize",
        "emergencywithdraw",
        "withdrawfees",
        "collectfees",
        "transferownership",
        "renounceownership",
        "changeadmin",
    ];
    if EXACT.contains(&lower.as_str()) {
        return true;
    }
    const PREFIXES: &[&str] = &[
        "set",
        "update",
        "configure",
        "grant",
        "revoke",
        "renounce",
        "pause",
        "unpause",
        "upgrade",
        "emergency",
        "rescue",
        "admin",
    ];
    if PREFIXES
        .iter()
        .any(|p| lower.starts_with(p) && lower.len() > p.len())
    {
        return true;
    }
    crate::identity::role_word(method).is_some()
}

/// Whether the grant at op `i` is made by someone with the power to grant:
/// a privileged address or a protocol contract the harness impersonates, or
/// the test contract on a contract it deployed itself. A grant the attacker
/// makes from an unprivileged sender is not a harness shortcut; it is the
/// exploit (missing access control), and RG01 leaves it alone.
fn granted_by_authority(ctx: &Ctx<'_, '_>, i: usize) -> bool {
    let env = ctx.env;
    match ctx.senders.get(i).copied().flatten() {
        Some(Sender::Switched(e)) => {
            !env.is_attacker(&key(e))
                && (env.privileged_expr(e).is_some() || matches!(env.class(e), Class::Protocol(_)))
        }
        Some(Sender::This) => ctx.trace.ops[i]
            .call()
            .and_then(|c| c.receiver)
            .is_some_and(|r| env.deployed_type(&key(r)).is_some()),
        None => false,
    }
}

/// RG01: the attacker identity is granted a role, or is a privileged address.
pub fn attacker_holds_privilege(ctx: &Ctx<'_, '_>, out: &mut Vec<Raw>) {
    let env = ctx.env;
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for g in &env.grants {
        if env.is_attacker(&g.grantee)
            && granted_by_authority(ctx, g.op)
            && seen.insert(g.grantee.clone())
        {
            let op = &ctx.trace.ops[g.op];
            out.push(Raw {
                rule: "RG01",
                severity: Severity::Error,
                unit: op.unit,
                pos: op.pos,
                message: format!(
                    "The harness grants the attacker `{}` a role. From here on the PoC shows what that role can do, not what an outsider can.",
                    g.grantee
                ),
                evidence: snippet(&g.call_text, 120),
            });
        }
    }
    for (i, op) in ctx.trace.ops.iter().enumerate() {
        let OpKind::Assign {
            target,
            value: Some(value),
            ..
        } = &op.kind
        else {
            continue;
        };
        if !env.is_attacker(target) || seen.contains(target) {
            continue;
        }
        if let ExprKind::Call { callee, .. } = &value.kind
            && let ExprKind::Member(_, method) = &callee.kind
            && is_privileged_getter(method)
        {
            seen.insert(target.clone());
            out.push(Raw {
                        rule: "RG01",
                        severity: Severity::Error,
                        unit: ctx.op_unit(i),
                        pos: op.pos,
                        message: format!(
                            "The attacker `{target}` is the address `{}` returns: the attacker is the privileged role.",
                            compact(&value.text)
                        ),
                        evidence: snippet(&value.text, 120),
                    });
        }
    }
}

/// RG02: the exploit body runs a step as a privileged address or as a
/// protocol contract.
pub fn privileged_step_in_exploit(ctx: &Ctx<'_, '_>, out: &mut Vec<Raw>) {
    let env = ctx.env;
    // identity -> (first op, count, reason)
    let mut hits: BTreeMap<String, (usize, usize, String)> = BTreeMap::new();
    for &(i, arg) in &ctx.switches {
        if !ctx.trace.ops[i].phase.is_exploit() {
            continue;
        }
        let k = key(arg);
        if env.is_attacker(&k) || env.declared_privileged.contains(&k) {
            continue;
        }
        let reason = match env.privileged_expr(arg) {
            Some(p) => Some(p.reason),
            None => match env.class(arg) {
                Class::Protocol(t) if t == "address" => Some(format!(
                    "`{k}` is a hard-coded address that is neither the attacker nor a known user, so the exploit is impersonating someone it does not control"
                )),
                Class::Protocol(t) => Some(format!(
                    "`{k}` is a contract under review ({t}); no attacker can send a transaction from it"
                )),
                _ => None,
            },
        };
        if let Some(reason) = reason {
            let entry = hits.entry(k).or_insert((i, 0, reason));
            entry.1 += 1;
        }
    }
    for (k, (i, count, reason)) in hits {
        let op = &ctx.trace.ops[i];
        let steps = if count == 1 {
            "a step".to_string()
        } else {
            format!("{count} steps")
        };
        out.push(Raw {
            rule: "RG02",
            severity: Severity::Warn,
            unit: op.unit,
            pos: op.pos,
            message: format!(
                "The exploit runs {steps} as `{k}`: {reason}. An attacker cannot sign as this address. Moving the prank into setUp() does not make it the attacker's action; a prank there next to a direct storage write is RG19. Declare the role in an operator realism.yaml outside the repo under review if the claim relies on that role acting."
            ),
            evidence: op
                .call()
                .map(|c| snippet(&c.expr.text, 120))
                .unwrap_or_default(),
        });
    }
}

/// RG19: `setUp()` both pranks as a role and writes protocol storage.
///
/// A setter called as `owner` in `setUp()` is still configuration. The bypass
/// this rule exists for is moving the exploit's owner prank and a `vm.store`
/// (or `stdstore` / `setArbitraryStorage` / `copyStorage`) into `setUp()`,
/// where RG02 does not look and where RG04 used to be only a warning.
pub fn setup_hides_storage_write(ctx: &Ctx<'_, '_>, out: &mut Vec<Raw>) {
    let writes: Vec<usize> = ctx
        .trace
        .ops
        .iter()
        .enumerate()
        .filter(|(_, op)| {
            op.phase == Phase::Setup && super::injection::protocol_storage_write(ctx, op).is_some()
        })
        .map(|(i, _)| i)
        .collect();
    if writes.is_empty() {
        return;
    }
    let mut pranks = Vec::new();
    for &(i, arg) in &ctx.switches {
        let op = &ctx.trace.ops[i];
        if op.phase != Phase::Setup {
            continue;
        }
        let k = key(arg);
        if ctx.env.is_attacker(&k) {
            continue;
        }
        if ctx.env.privileged_expr(arg).is_some() {
            pranks.push((i, k));
        }
    }
    if pranks.is_empty() {
        return;
    }
    let (i, k) = &pranks[0];
    let op = &ctx.trace.ops[*i];
    let write = &ctx.trace.ops[writes[0]];
    let how = write
        .call()
        .map(|c| snippet(&c.expr.text, 80))
        .unwrap_or_default();
    out.push(Raw {
        rule: "RG19",
        severity: Severity::Error,
        unit: op.unit,
        pos: op.pos,
        message: format!(
            "setUp() pranks as `{k}` and also writes protocol storage (`{how}`). Moving the owner prank and the storage write out of the test body hides them from the exploit rules. An attacker cannot sign as `{k}`, and cannot write that slot."
        ),
        evidence: op
            .call()
            .map(|c| snippet(&c.expr.text, 120))
            .unwrap_or_default(),
    });
}

/// RG03: the protocol is deployed by the test contract, which then runs the
/// exploit as itself.
pub fn deployer_is_attacker(ctx: &Ctx<'_, '_>, out: &mut Vec<Raw>) {
    let env = ctx.env;
    let ops = &ctx.trace.ops;
    // key -> deployed type, for contracts deployed by the test contract.
    let mut owned: BTreeMap<String, String> = BTreeMap::new();
    let mut last_deploy: Option<(usize, String)> = None;
    for (i, op) in ops.iter().enumerate() {
        match &op.kind {
            OpKind::Call(c) => {
                if let CallClass::Deploy(t) = &c.class {
                    last_deploy = Some((i, t.clone()));
                    continue;
                }
                if c.class == CallClass::External
                    && HANDOVER_METHODS.contains(&c.method.as_str())
                    && let Some(r) = c.receiver
                {
                    owned.remove(&key(r));
                }
            }
            OpKind::Assign {
                target,
                value: Some(value),
                ..
            } => {
                let Some(t) = env.deployed_type(target) else {
                    continue;
                };
                let Some((d, dt)) = &last_deploy else {
                    continue;
                };
                if *dt != t || !value.text.contains("new") {
                    continue;
                }
                if !matches!(ctx.senders[*d], Some(Sender::This)) {
                    continue;
                }
                if !matches!(env.class_of(target), Class::Protocol(_)) {
                    continue;
                }
                // `new Vault(owner)`: the constructor hands control to a role.
                let deploy = ops[*d].call().expect("deploy op is a call");
                let handed = deploy.args.iter().any(|a| env.privileged_expr(a).is_some());
                if !handed {
                    owned.insert(target.clone(), t);
                }
            }
            _ => {}
        }
    }
    if owned.is_empty() {
        return;
    }
    let mut ordinary: Option<usize> = None;
    let mut ordinary_count = 0usize;
    let mut privileged: Option<usize> = None;
    let mut privileged_count = 0usize;
    let mut privileged_methods: BTreeSet<String> = BTreeSet::new();
    let mut targets: BTreeSet<String> = BTreeSet::new();
    for (i, op) in ops.iter().enumerate() {
        if op.phase != Phase::Test {
            continue;
        }
        let Some(c) = op.call() else { continue };
        if c.class != CallClass::External || is_read_method(&c.method) {
            continue;
        }
        let Some(r) = c.receiver else { continue };
        let rk = key(r);
        if !owned.contains_key(&rk) || !matches!(ctx.senders[i], Some(Sender::This)) {
            continue;
        }
        targets.insert(rk);
        if is_access_controlled_method(&c.method) {
            privileged_count += 1;
            privileged_methods.insert(c.method.clone());
            privileged.get_or_insert(i);
        } else {
            ordinary_count += 1;
            ordinary.get_or_insert(i);
        }
    }
    let names: Vec<String> = targets
        .iter()
        .map(|t| format!("`{t}` ({})", owned[t]))
        .collect();
    let Some(i) = privileged.or(ordinary) else {
        return;
    };
    // A warning only when the deployer-sender actually calls something whose
    // name says it is gated. Everything else is the normal local deployment
    // and is a note, so a tree of `new Protocol()` harnesses does not fail
    // a run that fails on warnings.
    let (severity, message) = if privileged_count > 0 {
        let methods: Vec<&str> = privileged_methods.iter().map(String::as_str).collect();
        (
            Severity::Warn,
            format!(
                "{privileged_count} access-controlled call(s) ({}) run as the test contract, which deployed {} and still holds whatever its constructor gave the deployer. The PoC proves the deployer can make that call. Attack from a separate address.",
                methods.join(", "),
                names.join(", ")
            ),
        )
    } else {
        (
            Severity::Note,
            format!(
                "{ordinary_count} call(s) in the exploit run as the test contract, which deployed {}. None of them is named like an access-controlled function, so this is the ordinary local-deployment pattern, not evidence the attacker used a privilege. It is a note. Attack from a separate address if one of those calls is in fact gated.",
                names.join(", ")
            ),
        )
    };
    out.push(Raw {
        rule: "RG03",
        severity,
        unit: ops[i].unit,
        pos: ops[i].pos,
        message,
        evidence: ops[i]
            .call()
            .map(|c| snippet(&c.expr.text, 120))
            .unwrap_or_default(),
    });
}
