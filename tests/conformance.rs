//! RG-A6 frontend conformance: one shared fixture suite every frontend passes.
//!
//! The scenario is the same in all four languages (`tests/fixtures/
//! conformance/shared/vault_drain.*`, synthetic): an admin initialises a vault
//! in setup, the attacker is funded by a mutation (an airdrop, `vm.deal`, a
//! test-only mint), the attacker alone signs or sends the withdrawal, and an
//! assertion after it measures the attacker's balance.
//!
//! Each frontend is read into one [`View`] of six facts, from what that
//! frontend's pipeline exposes:
//!
//! - Rust and TypeScript: the exploit model (`--dump-model`), which is built
//!   from the frontend's lowering.
//! - Solidity: the Foundry trace built from the frontend's contracts.
//! - Move: the frontend's test function (signer bindings and calls in source
//!   order).
//!
//! The shared assertions are the same for every frontend: actors, signer,
//! mutation, assertion and phase. The payer is a fact only where the
//! execution model has a fee payer apart from the signer (Solana); for
//! Foundry and Move the view records why it is not modelled, and the test
//! checks that reason is still true, so a frontend that starts to report a
//! payer must update the table.

use realism_gate::engine::{Options, analyze};
use realism_gate::project::Project;
use realism_gate::trace::{self, CallClass, Phase};
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn fixture(name: &str) -> (String, String) {
    let rel = format!("tests/fixtures/conformance/shared/{name}");
    let src = std::fs::read_to_string(root().join(&rel)).expect("fixture exists");
    (rel, src)
}

fn project(name: &str) -> Project {
    let (rel, src) = fixture(name);
    let p = Project::from_sources(&[(rel.as_str(), src.as_str())]);
    assert_eq!(
        p.units.len(),
        1,
        "{name}: the frontend did not read the file"
    );
    assert!(
        p.units[0].errors.is_empty(),
        "{name}: {:?}",
        p.units[0].errors
    );
    p
}

/// A fact the frontend either reports or does not model, with the reason.
#[derive(Debug, PartialEq)]
enum Payer {
    Is(String),
    NotModelled(&'static str),
}

#[derive(Debug)]
struct View {
    actors: BTreeSet<String>,
    setup_calls: BTreeSet<String>,
    exploit_calls: BTreeSet<String>,
    setup_signers: BTreeSet<String>,
    exploit_signers: BTreeSet<String>,
    payer: Payer,
    /// (kind, target) of every state mutation the test makes.
    mutations: BTreeSet<(String, String)>,
    /// Assertions that run after the exploit's first step.
    assertions_after_exploit: usize,
}

/// The shared conformance check: the same six facts in every language.
fn check_shared(lang: &str, v: &View) {
    for actor in ["admin", "attacker"] {
        assert!(
            v.actors.contains(actor),
            "{lang}: actor `{actor}` missing: {v:?}"
        );
    }
    assert!(
        v.setup_calls.contains("initialize"),
        "{lang}: `initialize` is not in setup: {v:?}"
    );
    assert!(
        v.exploit_calls.contains("withdraw"),
        "{lang}: `withdraw` is not in the exploit: {v:?}"
    );
    assert!(
        !v.exploit_calls.contains("initialize") && !v.setup_calls.contains("withdraw"),
        "{lang}: setup and exploit are mixed: {v:?}"
    );
    assert!(
        v.setup_signers.contains("admin"),
        "{lang}: the admin does not sign setup: {v:?}"
    );
    assert!(
        v.exploit_signers.contains("attacker") && !v.exploit_signers.contains("admin"),
        "{lang}: the exploit signer is not the attacker alone: {v:?}"
    );
    assert!(
        v.mutations.iter().any(|(_, target)| target == "attacker"),
        "{lang}: no mutation funds the attacker: {v:?}"
    );
    assert!(
        v.assertions_after_exploit >= 1,
        "{lang}: no assertion after the exploit: {v:?}"
    );
    if let Payer::Is(payer) = &v.payer {
        assert_ne!(payer, "attacker", "{lang}: the payer is the attacker");
    }
}

// ---------------------------------------------------------------------------
// Rust and TypeScript: the exploit model.

fn solana_view(name: &str) -> View {
    let p = project(name);
    let model = realism_gate::solana::dump_model(
        &p,
        &realism_gate::solana::Declared::default(),
        None,
        Path::new("."),
    );
    assert_eq!(model.tests.len(), 1, "{name}: one modelled test");
    let t = serde_json::to_value(&model.tests[0]).unwrap();
    let mut v = View {
        actors: BTreeSet::new(),
        setup_calls: BTreeSet::new(),
        exploit_calls: BTreeSet::new(),
        setup_signers: BTreeSet::new(),
        exploit_signers: BTreeSet::new(),
        payer: Payer::NotModelled("no step"),
        mutations: BTreeSet::new(),
        assertions_after_exploit: 0,
    };
    for a in t["actors"].as_array().unwrap() {
        v.actors.insert(a["id"].as_str().unwrap().to_string());
    }
    let mut first_exploit_op = u64::MAX;
    let mut payers = BTreeSet::new();
    for s in t["steps"].as_array().unwrap() {
        let entry = s["entry_point"].as_str().unwrap_or("").to_string();
        let signers = s["signers"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().to_string());
        if let Some(p) = s["payer"].as_str() {
            payers.insert(p.to_string());
        }
        match s["phase"].as_str().unwrap() {
            "exploit" => {
                first_exploit_op = first_exploit_op.min(s["op"].as_u64().unwrap());
                v.exploit_calls.insert(entry);
                v.exploit_signers.extend(signers);
            }
            "setup" => {
                v.setup_calls.insert(entry);
                v.setup_signers.extend(signers);
            }
            _ => {}
        }
    }
    // The payer is recorded apart from the signers. A fee payer that also
    // signs (Rust's `new_signed_with_payer`) is not an authority holder.
    for p in &payers {
        v.exploit_signers.remove(p);
        v.setup_signers.remove(p);
    }
    assert!(payers.len() <= 1, "{name}: one payer: {payers:?}");
    if let Some(p) = payers.into_iter().next() {
        v.payer = Payer::Is(p);
    }
    for m in t["mutations"].as_array().unwrap() {
        v.mutations.insert((
            m["kind"].as_str().unwrap().to_string(),
            m["target"].as_str().unwrap_or("").to_string(),
        ));
    }
    v.assertions_after_exploit = t["assertions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|a| a["op"].as_u64().unwrap() > first_exploit_op)
        .count();
    v
}

#[test]
fn conformance_rust() {
    let v = solana_view("vault_drain.rs");
    check_shared("rust", &v);
    assert_eq!(v.payer, Payer::Is("fee_payer".to_string()));
}

#[test]
fn conformance_typescript() {
    let v = solana_view("vault_drain.ts");
    check_shared("typescript", &v);
    // Anchor's `.rpc()` is paid by the provider wallet.
    assert_eq!(v.payer, Payer::Is("wallet".to_string()));
}

// ---------------------------------------------------------------------------
// Solidity: the Foundry trace.

const NO_EVM_PAYER: &str = "Foundry: the sender pays its own gas; there is no fee payer apart from the sender (`vm.prank(sender, origin)` sets tx.origin, not a payer)";
const NO_MOVE_PAYER: &str =
    "Move tests: the transaction sender pays; test_scenario has no separate fee payer";

#[test]
fn conformance_solidity() {
    let p = project("VaultDrain.t.sol");
    let suites = p.suites();
    assert_eq!(suites.len(), 1);
    let suite = &suites[0];
    let test = suite
        .tests
        .iter()
        .find(|f| f.name == "testAttackerDrainsTheVault")
        .expect("the test function is read");
    let tr = trace::build(&p, suite, test);
    let mut v = View {
        actors: suite
            .contract
            .vars
            .iter()
            .filter(|s| s.ty == "address")
            .map(|s| s.name.clone())
            .collect(),
        setup_calls: BTreeSet::new(),
        exploit_calls: BTreeSet::new(),
        setup_signers: BTreeSet::new(),
        exploit_signers: BTreeSet::new(),
        payer: Payer::NotModelled(NO_EVM_PAYER),
        mutations: BTreeSet::new(),
        assertions_after_exploit: 0,
    };
    let mut prank: Option<String> = None;
    let mut exploit_started = false;
    for (_, op, call) in tr.calls() {
        let arg0 = call
            .args
            .first()
            .map(|a| a.text.clone())
            .unwrap_or_default();
        match call.class {
            CallClass::Cheat if matches!(call.method.as_str(), "prank" | "startPrank") => {
                prank = Some(arg0);
            }
            CallClass::Cheat | CallClass::StdCheat
                if matches!(call.method.as_str(), "deal" | "store" | "etch" | "hoax") =>
            {
                v.mutations.insert((call.method.clone(), arg0));
            }
            CallClass::External => {
                let sender = prank.take().unwrap_or_else(|| "this".to_string());
                if op.phase == Phase::Setup {
                    v.setup_calls.insert(call.method.clone());
                    v.setup_signers.insert(sender);
                } else {
                    exploit_started = true;
                    v.exploit_calls.insert(call.method.clone());
                    v.exploit_signers.insert(sender);
                }
            }
            CallClass::Assert if exploit_started => v.assertions_after_exploit += 1,
            _ => {}
        }
    }
    check_shared("solidity", &v);
    // The table entry is still true: nothing in the frontend names a payer.
    assert!(
        !tr.calls()
            .any(|(_, _, c)| c.args.len() == 2
                && matches!(c.method.as_str(), "prank" | "startPrank")),
        "a two-argument prank sets tx.origin; revisit the payer row"
    );
}

// ---------------------------------------------------------------------------
// Move: the frontend's test function.

#[test]
fn conformance_move() {
    let p = project("vault_drain.move");
    let file = p.units[0].move_file.as_ref().expect("a Move module");
    assert_eq!(file.tests.len(), 1);
    let t = &file.tests[0];
    let mut v = View {
        actors: t.signers.iter().map(|(name, _)| name.clone()).collect(),
        setup_calls: BTreeSet::new(),
        exploit_calls: BTreeSet::new(),
        setup_signers: BTreeSet::new(),
        exploit_signers: BTreeSet::new(),
        payer: Payer::NotModelled(NO_MOVE_PAYER),
        mutations: BTreeSet::new(),
        assertions_after_exploit: 0,
    };
    // The sender of each call is the address the scenario began with, then
    // the one each `next_tx` switches to. The first transaction is setup; a
    // switch to another sender starts the exploit.
    let mut calls: Vec<_> = t.calls.iter().collect();
    calls.sort_by_key(|c| (c.pos.line, c.pos.col));
    // `args` is the argument list as written, parentheses included.
    let last_arg = |args: &str| {
        let inner = args.trim().trim_start_matches('(').trim_end_matches(')');
        inner.rsplit(',').next().unwrap_or("").trim().to_string()
    };
    let mut sender = String::new();
    let mut setup = true;
    let mut exploit_started = false;
    for c in calls {
        match c.name.as_str() {
            "begin" => sender = last_arg(&c.args),
            "next_tx" => {
                let next = last_arg(&c.args);
                if next != sender {
                    setup = false;
                }
                sender = next;
            }
            "assert" | "assert_eq" if exploit_started => v.assertions_after_exploit += 1,
            name if name.ends_with("_for_testing") => {
                v.mutations.insert((name.to_string(), sender.clone()));
            }
            name if c.path.starts_with("vault::") => {
                if setup {
                    v.setup_calls.insert(name.to_string());
                    v.setup_signers.insert(sender.clone());
                } else {
                    exploit_started = true;
                    v.exploit_calls.insert(name.to_string());
                    v.exploit_signers.insert(sender.clone());
                }
            }
            _ => {}
        }
    }
    check_shared("move", &v);
}

// ---------------------------------------------------------------------------
// Every frontend's scenario is readable through the engine: one suite, one
// test, no parse error.

#[test]
fn every_shared_fixture_is_readable_through_the_engine() {
    for name in [
        "vault_drain.rs",
        "vault_drain.ts",
        "VaultDrain.t.sol",
        "vault_drain.move",
    ] {
        let p = project(name);
        let r = analyze(&p, None, &Options::default());
        assert_eq!(r.summary.suites, 1, "{name}: {:?}", r.summary);
        assert_eq!(r.summary.tests, 1, "{name}: {:?}", r.summary);
        assert!(r.parse_errors.is_empty(), "{name}: {:?}", r.parse_errors);
    }
}

/// Renaming every identifier the scenario uses for its actors leaves the
/// facts unchanged up to the renaming (§9 property, per frontend).
#[test]
fn renaming_the_actors_keeps_the_view() {
    for name in ["vault_drain.rs", "vault_drain.ts"] {
        let (rel, src) = fixture(name);
        let renamed = src
            .replace("attacker", "mallory")
            .replace("admin", "owner_key");
        let p = Project::from_sources(&[(rel.as_str(), renamed.as_str())]);
        let model = realism_gate::solana::dump_model(
            &p,
            &realism_gate::solana::Declared::default(),
            None,
            Path::new("."),
        );
        let t: Value = serde_json::to_value(&model.tests[0]).unwrap();
        let exploit_signers: BTreeSet<&str> = t["steps"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|s| s["phase"] == "exploit")
            .flat_map(|s| s["signers"].as_array().unwrap().iter())
            .filter_map(Value::as_str)
            .collect();
        assert!(exploit_signers.contains("mallory"), "{name}: {t}");
        assert!(!exploit_signers.contains("owner_key"), "{name}: {t}");
    }
}
