//! P1-C regression tests: the readers added after the PoC study, the eight
//! false-positive classes it found, the true positives that must survive, and
//! the properties a rename, a reorder or an unrelated test must not change.
//!
//! Every fixture here is synthetic: a few lines written for this file, never
//! third-party PoC text.

use realism_gate::Severity;
use realism_gate::engine::{Options, Report, analyze};
use realism_gate::project::Project;

const RS_PRELUDE: &str = r#"
use litesvm::LiteSVM;
use solana_sdk::account::Account;
use solana_sdk::instruction::Instruction;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk::transaction::Transaction;

fn send(svm: &mut LiteSVM, ix: Instruction, signers: &[&Keypair]) {
    let payer = signers[0].pubkey();
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer), signers, svm.latest_blockhash());
    svm.send_transaction(tx).unwrap();
}
"#;

fn rs_report(name: &str, snippet: &str) -> Report {
    let src = format!(
        "{RS_PRELUDE}\n#[test]\nfn {name}() {{\n    let mut svm = LiteSVM::new();\n    let admin = Keypair::new();\n    let attacker = Keypair::new();\n    let victim = Keypair::new();\n{snippet}\n}}\n"
    );
    let p = Project::from_sources(&[("tests/p1c.rs", &src)]);
    let r = analyze(&p, None, &Options::default());
    assert!(r.parse_errors.is_empty(), "{name}: {:?}", r.parse_errors);
    r
}

fn rs_rules(name: &str, snippet: &str) -> Vec<&'static str> {
    rs_report(name, snippet).active().map(|f| f.rule).collect()
}

const TS_PRELUDE: &str = r#"
import * as anchor from "@coral-xyz/anchor";
import { BN, Program } from "@coral-xyz/anchor";
import { Keypair, PublicKey, Transaction } from "@solana/web3.js";
import { assert } from "chai";
const provider = anchor.AnchorProvider.env();
const program = anchor.workspace.Vault as Program<any>;
"#;

fn ts_report(body: &str) -> Report {
    let src = format!("{TS_PRELUDE}{body}");
    let p = Project::from_sources(&[("tests/p1c.ts", &src)]);
    let r = analyze(&p, None, &Options::default());
    assert!(r.parse_errors.is_empty(), "{:?}", r.parse_errors);
    r
}

fn ts_rules(body: &str) -> Vec<&'static str> {
    ts_report(body).active().map(|f| f.rule).collect()
}

// ---------------------------------------------------------------- readers

#[test]
fn reader_reads_a_rust_unit_test_that_calls_program_functions() {
    // Class 1 of the study's unreadable list: a #[test] that calls program
    // code directly, with no harness. The direct call is modelled as the
    // exploit transaction.
    let r = rs_report(
        "test_update_sets_value",
        "    let mut global = Global::default();\n    let before = global.total;\n    global.update_settings(5);\n    assert!(global.total > before);",
    );
    assert_eq!(r.summary.tests, 1, "{:?}", r.findings);
}

#[test]
fn reader_reads_a_rust_fn_main_poc_framework_binary() {
    let src = r#"
use poc_framework_osec::solana_sdk::signer::Signer;
fn main() {
    let env = LocalEnvironment::builder().build();
    let attacker = Keypair::new();
    let ix = withdraw_ix(&attacker.pubkey());
    env.execute_as_transaction(&[ix], &[&attacker]).print();
}
"#;
    let p = Project::from_sources(&[("pocs/poc.rs", src)]);
    let r = analyze(&p, None, &Options::default());
    assert_eq!(r.summary.tests, 1, "{:?}", r.parse_errors);
    assert!(r.active().any(|f| f.rule == "RG08"), "{:?}", r.findings);
}

#[test]
fn reader_reads_a_typescript_script_as_one_test() {
    let r = ts_report(
        r#"
async function main() {
  const attacker = Keypair.generate();
  await program.methods.adminWithdraw(new BN(1)).accounts({ recipient: attacker.publicKey }).signers([attacker]).rpc();
  console.log("done");
}
main().catch(console.error);
"#,
    );
    assert_eq!(r.summary.tests, 1, "{:?}", r.findings);
    assert!(r.active().any(|f| f.rule == "RG08"), "{:?}", r.findings);
}

// ------------------------------------------------- class 1: the attacker's own key

#[test]
fn class1_a_fresh_keypair_with_no_grant_is_the_attacker() {
    let rules = rs_rules(
        "test_owner_overflows_its_own_vault",
        "    let owner = Keypair::new();\n    send(&mut svm, &overflow_ix(&owner.pubkey()), &[&owner]);\n    assert!(svm.get_balance(&owner.pubkey()).unwrap() > 0);",
    );
    assert!(!rules.contains(&"RG01"), "{rules:?}");
    assert!(!rules.contains(&"RG02"), "{rules:?}");
}

#[test]
fn class1_a_role_granted_by_another_signer_still_fires() {
    let rules = rs_rules(
        "test_admin_grants_the_attacker",
        "    send(&mut svm, &instruction::Initialize { admin: attacker.pubkey() }, &[&admin]);\n    send(&mut svm, &instruction::SetFee { bps: 10_000 }, &[&attacker]);\n    assert!(svm.get_balance(&attacker.pubkey()).unwrap() > 0);",
    );
    assert!(rules.contains(&"RG01"), "{rules:?}");
}

// ------------------------------------------- class 2: secure-path / control tests

#[test]
fn class2_a_control_test_does_not_fire_identity_or_state_rules() {
    let rules = rs_rules(
        "test_secure_path_rejects_the_attack",
        "    send(&mut svm, &set_admin_ix(&admin.pubkey()), &[&admin]);\n    let result = try_withdraw(&attacker.pubkey());\n    assert!(result.is_err());",
    );
    for rule in ["RG01", "RG02", "RG03", "RG04", "RG06"] {
        assert!(
            !rules.contains(&rule),
            "{rule} fired on a control test: {rules:?}"
        );
    }
}

#[test]
fn class2_the_exploit_twin_still_reports_the_privileged_step() {
    let rules = rs_rules(
        "test_exploit_path",
        "    send(&mut svm, &set_oracle_ix(&fake_oracle), &[&admin]);\n    send(&mut svm, &borrow_ix(&attacker.pubkey(), 1_000), &[&attacker]);\n    assert!(svm.get_balance(&attacker.pubkey()).unwrap() > 0);",
    );
    assert!(rules.contains(&"RG02"), "{rules:?}");
}

// ------------------------------- class 3: fixture accounts the program initialises

#[test]
fn class3_a_zeroed_account_the_program_initialises_is_a_fixture() {
    let rules = rs_rules(
        "test_initialise_my_own_vault",
        "    let vault = Pubkey::new_unique();\n    svm.set_account(vault, Account { lamports: 1_000_000, data: vec![], owner: vault_program::ID, executable: false, rent_epoch: 0 }).unwrap();\n    send(&mut svm, &init_vault_ix(&vault), &[&admin]);\n    assert!(svm.get_account(&vault).is_ok());",
    );
    assert!(!rules.contains(&"RG04"), "{rules:?}");
    assert!(!rules.contains(&"RG06"), "{rules:?}");
}

#[test]
fn class3_the_same_write_without_the_init_is_still_flagged() {
    let rules = rs_rules(
        "test_write_a_vault_with_no_init",
        "    let vault = Pubkey::new_unique();\n    svm.set_account(vault, Account { lamports: 1_000_000, data: vec![], owner: vault_program::ID, executable: false, rent_epoch: 0 }).unwrap();\n    send(&mut svm, &withdraw_ix(&attacker.pubkey()), &[&attacker]);\n    assert!(svm.get_account(&vault).is_ok());",
    );
    assert!(
        rules.contains(&"RG06") || rules.contains(&"RG04"),
        "{rules:?}"
    );
}

// ------------------------------- class 4: the victim's precondition fabricated

#[test]
fn class4_a_precondition_written_in_setup_is_a_note() {
    let r = rs_report(
        "test_drain_a_funded_vault",
        "    let vault = Pubkey::find_program_address(&[b\"vault\"], &program_id).0;\n    svm.set_account(vault, Account { lamports: 1, data: vec![1u8; 32], owner: vault_program::ID, executable: false, rent_epoch: 0 }).unwrap();\n    send(&mut svm, &withdraw_ix(&attacker.pubkey()), &[&attacker]);\n    assert!(svm.get_account(&vault).is_ok());",
    );
    let f = r
        .active()
        .find(|f| f.rule == "RG04" || f.rule == "RG06")
        .expect("a state write finding");
    assert_eq!(f.severity, Severity::Note);
    assert!(
        f.message.contains("Precondition fabricated"),
        "{}",
        f.message
    );
    assert!(f.message.contains("never passed through"), "{}", f.message);
}

#[test]
fn class4_a_write_in_the_exploit_is_not_called_a_precondition() {
    let r = rs_report(
        "test_write_inside_the_exploit",
        "    send(&mut svm, &withdraw_ix(&attacker.pubkey()), &[&attacker]);\n    let vault = Pubkey::find_program_address(&[b\"vault\"], &program_id).0;\n    svm.set_account(vault, Account { lamports: 1, data: vec![1u8; 32], owner: vault_program::ID, executable: false, rent_epoch: 0 }).unwrap();\n    assert!(svm.get_account(&vault).is_ok());",
    );
    let f = r
        .active()
        .find(|f| f.rule == "RG04" || f.rule == "RG06")
        .expect("a state write finding");
    assert!(
        !f.message.contains("Precondition fabricated"),
        "{}",
        f.message
    );
}

// ------------------------------------ class 5: setup tests inherited from a suite

#[test]
fn class5_a_configuration_test_is_setup_for_the_tests_after_it() {
    let r = ts_report(
        r#"
describe("vault", () => {
  it("updates the oracle price", async () => {
    await program.methods.setOracle(new BN(1)).rpc();
    const config = await program.account.config.fetch(configKey);
    assert.equal(config.price.toNumber(), 1);
  });
  it("a later test", async () => {
    const attacker = Keypair.generate();
    await program.methods.adminWithdraw(new BN(1)).accounts({ recipient: attacker.publicKey }).signers([attacker]).rpc();
    assert.ok(true);
  });
});
"#,
    );
    assert_eq!(r.summary.tests, 1, "{:?}", r.findings);
}

// ----------------------------- class 6: the attacker takes the role through the bug

#[test]
fn class6_the_attacker_initialising_itself_as_admin_is_the_exploit() {
    let rules = rs_rules(
        "test_take_the_role_through_the_bug",
        "    send(&mut svm, &instruction::Initialize { admin: attacker.pubkey() }, &[&attacker]);\n    send(&mut svm, &instruction::SetFee { bps: 10_000 }, &[&attacker]);\n    assert!(svm.get_balance(&attacker.pubkey()).unwrap() > 0);",
    );
    assert!(!rules.contains(&"RG01"), "{rules:?}");
}

// ------------------------------------------- class 7: RG10 and a before-value

#[test]
fn class7_a_value_read_before_the_attack_is_a_baseline() {
    let rules = rs_rules(
        "test_measure_a_gain",
        "    let pool = Pubkey::new_from_array([9u8; 32]);\n    let before = svm.get_balance(&pool).unwrap();\n    send(&mut svm, &drain_ix(&attacker.pubkey()), &[&attacker]);\n    let after = svm.get_balance(&pool).unwrap();\n    assert!(after > before);",
    );
    assert!(!rules.contains(&"RG10"), "{rules:?}");
}

#[test]
fn class7_a_value_read_only_after_the_attack_is_still_flagged() {
    let rules = rs_rules(
        "test_measure_after_only",
        "    let pool = Pubkey::new_from_array([9u8; 32]);\n    send(&mut svm, &drain_ix(&attacker.pubkey()), &[&attacker]);\n    let after = svm.get_balance(&pool).unwrap();\n    assert!(after > 0);",
    );
    assert!(rules.contains(&"RG10"), "{rules:?}");
}

// ------------------------------------------------- class 8: RG08's scope

#[test]
fn class8_rg08_still_applies_to_a_control_test_with_no_assertion() {
    let rules = rs_rules(
        "test_control_path_without_an_assertion",
        "    send(&mut svm, &set_admin_ix(&admin.pubkey()), &[&admin]);\n    let _result = try_withdraw(&attacker.pubkey());",
    );
    assert!(rules.contains(&"RG08"), "{rules:?}");
    assert!(!rules.contains(&"RG02"), "{rules:?}");
}

#[test]
fn class8_rg08_does_not_fire_when_the_exploit_asserts_its_outcome() {
    let rules = rs_rules(
        "test_assert_the_outcome",
        "    let before = svm.get_balance(&attacker.pubkey()).unwrap();\n    send(&mut svm, &drain_ix(&attacker.pubkey()), &[&attacker]);\n    let after = svm.get_balance(&attacker.pubkey()).unwrap();\n    assert!(after > before);",
    );
    assert!(!rules.contains(&"RG08"), "{rules:?}");
}

// ------------------------------------------------------ true-positive survival

#[test]
fn true_positive_a_tautological_ts_assertion_is_still_rg09() {
    let rules = ts_rules(
        r#"
it("drains", async () => {
  const attacker = Keypair.generate();
  await program.methods.adminWithdraw(new BN(1)).accounts({ recipient: attacker.publicKey }).signers([attacker]).rpc();
  assert.ok(true);
});
"#,
    );
    assert!(rules.contains(&"RG09"), "{rules:?}");
}

#[test]
fn true_positive_a_console_only_outcome_is_still_rg08() {
    let rules = ts_rules(
        r#"
it("drains", async () => {
  const attacker = Keypair.generate();
  await program.methods.adminWithdraw(new BN(1)).accounts({ recipient: attacker.publicKey }).signers([attacker]).rpc();
  console.log("drained");
});
"#,
    );
    assert!(rules.contains(&"RG08"), "{rules:?}");
}

// ------------------------------------------------------------- properties

#[test]
fn property_renaming_the_attacker_key_does_not_change_the_flags() {
    let a = rs_rules(
        "test_a",
        "    let owner = Keypair::new();\n    send(&mut svm, &overflow_ix(&owner.pubkey()), &[&owner]);\n    assert!(svm.get_balance(&owner.pubkey()).unwrap() > 0);",
    );
    let b = rs_rules(
        "test_b",
        "    let holding = Keypair::new();\n    send(&mut svm, &overflow_ix(&holding.pubkey()), &[&holding]);\n    assert!(svm.get_balance(&holding.pubkey()).unwrap() > 0);",
    );
    assert_eq!(a, b, "a rename changed the flags");
}

#[test]
fn property_reordering_independent_setup_does_not_change_the_flags() {
    let a = rs_rules(
        "test_a",
        "    let owner = Keypair::new();\n    let mint = Pubkey::new_unique();\n    send(&mut svm, &overflow_ix(&owner.pubkey()), &[&owner]);\n    assert!(svm.get_balance(&owner.pubkey()).unwrap() > 0);\n    assert!(mint != Pubkey::default());",
    );
    let b = rs_rules(
        "test_b",
        "    let mint = Pubkey::new_unique();\n    let owner = Keypair::new();\n    send(&mut svm, &overflow_ix(&owner.pubkey()), &[&owner]);\n    assert!(svm.get_balance(&owner.pubkey()).unwrap() > 0);\n    assert!(mint != Pubkey::default());",
    );
    assert_eq!(a, b, "a reorder changed the flags");
}

#[test]
fn property_adding_an_unrelated_control_test_keeps_the_true_positive() {
    let src = format!(
        "{TS_PRELUDE}\nit(\"drains\", async () => {{\n  const attacker = Keypair.generate();\n  await program.methods.adminWithdraw(new BN(1)).accounts({{ recipient: attacker.publicKey }}).signers([attacker]).rpc();\n  assert.ok(true);\n}});\nit(\"rejects the attack\", async () => {{\n  const attacker = Keypair.generate();\n  try {{ await program.methods.adminWithdraw(new BN(1)).accounts({{ recipient: attacker.publicKey }}).signers([attacker]).rpc(); assert.fail(\"expected a rejection\"); }} catch (e) {{}}\n}});\n"
    );
    let p = Project::from_sources(&[("tests/p1c.ts", &src)]);
    let r = analyze(&p, None, &Options::default());
    let rules: Vec<&str> = r.active().map(|f| f.rule).collect();
    assert!(rules.contains(&"RG09"), "{rules:?}");
}
