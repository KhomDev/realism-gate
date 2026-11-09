//! The Move subset of the rules. Same ids as Solidity: a privileged signer
//! is still RG02, a test-only mint paid to the attacker is still RG06.

use super::{Call, TestFn};
use crate::finding::Severity;
use crate::identity::{is_attacker_name, role_word};
use crate::rules::Raw;

const CAPS: &[&str] = &["AdminCap", "TreasuryCap", "MintCap"];

pub fn check_test(unit: usize, test: &TestFn) -> Vec<Raw> {
    let mut out = Vec::new();
    admin_cap(unit, test, &mut out);
    privileged_signer(unit, test, &mut out);
    mint_for_testing(unit, test, &mut out);
    if test.expected_failure && !has_assert(test) {
        let call = test.calls.first();
        out.push(Raw {
            rule: "RG18",
            severity: Severity::Warn,
            unit,
            pos: call.map(|c| c.pos).unwrap_or(test.pos),
            message: format!(
                "`{}` is `#[expected_failure]` and asserts nothing. The pass condition is an abort, which does not show that value moved or that funds are stuck.",
                test.name
            ),
            evidence: format!("fun {}(...)", test.name),
        });
    } else if !has_assert(test) {
        out.push(Raw {
            rule: "RG08",
            severity: Severity::Error,
            unit,
            pos: test.pos,
            message: format!(
                "`{}` contains no `assert!`. It passes whatever the module does.",
                test.name
            ),
            evidence: format!("fun {}(...)", test.name),
        });
    }
    no_baseline(unit, test, &mut out);
    out
}

fn has_assert(test: &TestFn) -> bool {
    test.calls.iter().any(|c| c.name == "assert")
}

fn looks_like_attacker(text: &str) -> bool {
    is_attacker_name(text)
        || text
            .split(|c: char| !c.is_ascii_alphanumeric())
            .any(is_attacker_name)
}

fn looks_privileged(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    if lower.contains("@0x1") || lower.contains("@0x0") || lower.contains("aptos_framework") {
        return true;
    }
    role_word(text).is_some()
        || text
            .split(|c: char| !c.is_ascii_alphanumeric())
            .any(|w| role_word(w).is_some())
}

fn sender_of(test: &TestFn, index: usize) -> Option<&Call> {
    test.calls[..index]
        .iter()
        .rev()
        .find(|c| matches!(c.name.as_str(), "next_tx" | "begin"))
}

fn last_arg(call: &Call) -> &str {
    call.args
        .trim_start_matches('(')
        .trim_end_matches(')')
        .rsplit(',')
        .next()
        .unwrap_or("")
        .trim()
}

fn admin_cap(unit: usize, test: &TestFn, out: &mut Vec<Raw>) {
    for (name, addr) in &test.signers {
        if looks_like_attacker(name) && looks_privileged(addr) {
            out.push(Raw {
                rule: "RG01",
                severity: Severity::Error,
                unit,
                pos: test.pos,
                message: format!(
                    "The attacker signer `{name}` is bound to `{addr}`, a framework or root address. The test is signed by an account that holds privileges an outsider does not."
                ),
                evidence: format!("{name} = {addr}"),
            });
            return;
        }
    }
    for (i, call) in test.calls.iter().enumerate() {
        let cap = CAPS
            .iter()
            .any(|c| call.type_args.contains(c) || call.text.contains(c));
        if !cap {
            continue;
        }
        let sender = sender_of(test, i).map(last_arg).unwrap_or("");
        let to_attacker = looks_like_attacker(&call.args) || looks_like_attacker(sender);
        let takes = call.name.contains("take") || call.name.contains("transfer");
        if takes && to_attacker {
            out.push(Raw {
                rule: "RG01",
                severity: Severity::Error,
                unit,
                pos: call.pos,
                message: format!(
                    "`{}` moves an admin, treasury or mint cap while the sender or the recipient reads as the attacker. From here the test shows what that cap can do.",
                    call.name
                ),
                evidence: trim(&call.text),
            });
            return;
        }
    }
    // A cap was taken, and a later transfer pays it to the attacker.
    let took_cap = test
        .calls
        .iter()
        .any(|c| CAPS.iter().any(|cap| c.type_args.contains(cap)) && c.name.contains("take"));
    if !took_cap {
        return;
    }
    if let Some(call) = test.calls.iter().find(|c| {
        matches!(c.name.as_str(), "public_transfer" | "transfer") && looks_like_attacker(&c.args)
    }) {
        out.push(Raw {
            rule: "RG01",
            severity: Severity::Error,
            unit,
            pos: call.pos,
            message:
                "A cap taken in this test is transferred to the attacker. The harness handed the attacker a privilege."
                    .to_string(),
            evidence: trim(&call.text),
        });
    }
}

fn privileged_signer(unit: usize, test: &TestFn, out: &mut Vec<Raw>) {
    let mut seen_privileged = false;
    for call in &test.calls {
        if matches!(call.name.as_str(), "begin" | "next_tx") {
            let sender = last_arg(call);
            if looks_privileged(sender) {
                if seen_privileged {
                    out.push(Raw {
                        rule: "RG02",
                        severity: Severity::Warn,
                        unit,
                        pos: call.pos,
                        message: format!(
                            "`{}` switches the sender back to `{sender}` after the privileged setup transaction. A later transaction as that signer is a step an attacker cannot sign.",
                            call.name
                        ),
                        evidence: trim(&call.text),
                    });
                    return;
                }
                seen_privileged = true;
            }
        }
    }
    let privileged_params: Vec<&str> = test
        .params
        .iter()
        .filter(|p| {
            looks_privileged(p)
                || test
                    .signers
                    .iter()
                    .any(|(n, a)| n == *p && looks_privileged(a))
        })
        .map(String::as_str)
        .collect();
    if privileged_params.is_empty() {
        return;
    }
    for call in &test.calls {
        if matches!(
            call.name.as_str(),
            "begin" | "next_tx" | "init" | "initialize" | "setup" | "end"
        ) {
            continue;
        }
        if privileged_params
            .iter()
            .any(|p| arg_mentions(&call.args, p))
        {
            out.push(Raw {
                rule: "RG02",
                severity: Severity::Warn,
                unit,
                pos: call.pos,
                message: format!(
                    "`{}` is called with a privileged signer. That signer is the admin or the framework account, not an unprivileged attacker.",
                    call.name
                ),
                evidence: trim(&call.text),
            });
            return;
        }
    }
}

fn arg_mentions(args: &str, name: &str) -> bool {
    args.split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .any(|w| w == name)
}

fn mint_for_testing(unit: usize, test: &TestFn, out: &mut Vec<Raw>) {
    let mints: Vec<&Call> = test
        .calls
        .iter()
        .filter(|c| {
            let n = c.name.to_ascii_lowercase();
            n.contains("mint_for_test") || n.contains("mint_to_for_test")
        })
        .collect();
    if mints.is_empty() {
        return;
    }
    let paid_to_attacker = test.calls.iter().any(|c| {
        matches!(c.name.as_str(), "public_transfer" | "transfer" | "deposit")
            && looks_like_attacker(&c.args)
    }) || mints.iter().any(|c| looks_like_attacker(&c.args));
    if !paid_to_attacker {
        return;
    }
    let call = mints[0];
    out.push(Raw {
        rule: "RG06",
        severity: Severity::Warn,
        unit,
        pos: call.pos,
        message: format!(
            "`{}` mints coins with a test-only helper and the test gives them to the attacker. The coins were not minted by the protocol's treasury. If they are only the attacker's budget, say so in an allow comment; if the profit is those coins, the test has not shown an extraction.",
            call.name
        ),
        evidence: trim(&call.text),
    });
}

fn no_baseline(unit: usize, test: &TestFn, out: &mut Vec<Raw>) {
    let mut seen_read = false;
    for call in &test.calls {
        if call.name != "assert" {
            if call.text.contains("value")
                || call.name == "balance"
                || call.text.contains("balance")
            {
                seen_read = true;
            }
            continue;
        }
        // `>=` contains `>`. A comparison against a name `before` is a baseline.
        let against_before = call.args.contains("before");
        let measures = call.args.contains("value") || call.args.contains("balance");
        if measures && call.args.contains('>') && !against_before && !seen_read {
            out.push(Raw {
                rule: "RG10",
                severity: Severity::Warn,
                unit,
                pos: call.pos,
                message:
                    "The assertion compares a coin value or a balance with a bound, and nothing in the test read that value before the attack. A gain is a difference."
                        .to_string(),
                evidence: trim(&call.text),
            });
            return;
        }
    }
}

fn trim(text: &str) -> String {
    crate::rules::snippet(text, 120)
}
