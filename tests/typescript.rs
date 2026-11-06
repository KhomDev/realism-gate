//! Anchor TypeScript tests: which files are read, how mocha's structure is
//! traced, where findings are placed, and how suppressions, the CLI and the
//! rule pages apply to them.

use realism_gate::Severity::Warn;
use realism_gate::engine::{Options, Report, analyze};
use realism_gate::project::{Project, discover};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn report(paths: &[&str]) -> Report {
    let base = root();
    let paths: Vec<PathBuf> = paths.iter().map(|p| base.join(p)).collect();
    let project = Project::load(&discover(&paths).unwrap(), &base).unwrap();
    analyze(&project, None, &Options::default())
}

fn source_report(src: &str) -> Report {
    let p = Project::from_sources(&[("tests/example.ts", src)]);
    analyze(&p, None, &Options::default())
}

#[test]
fn scripts_that_import_no_solana_package_are_not_read() {
    let base = root();
    let files = discover(&[base.join("tests/fixtures/good")]).unwrap();
    assert!(files.iter().any(|f| f.ends_with("HardhatSuite.ts")));
    let project = Project::load(&files, &base).unwrap();
    assert!(
        project
            .units
            .iter()
            .all(|u| !u.path.ends_with("HardhatSuite.ts")),
        "a Hardhat suite is not a Solana test"
    );
}

#[test]
fn a_setup_test_that_later_tests_build_on_is_their_setup() {
    let r = report(&["tests/fixtures/good/AnchorVaultExploit.ts"]);
    // "is initialized" and "is funded by the victim" run first and are not
    // checked on their own; the exploit is.
    assert_eq!(r.summary.tests, 1);
    assert!(r.findings.is_empty(), "{:?}", r.findings);
}

#[test]
fn a_helper_in_a_shared_file_is_reported_at_the_call() {
    let r = report(&["tests/fixtures/ts_shared"]);
    let got: Vec<_> = r
        .active()
        .map(|f| (f.rule, f.severity, f.file.as_str(), f.line))
        .collect();
    assert_eq!(
        got,
        vec![("RG02", Warn, "tests/fixtures/ts_shared/exploit.ts", 15)]
    );
    assert!(
        r.findings[0]
            .evidence
            .contains("withdrawAs(program, admin, 100)")
    );
}

#[test]
fn a_suppression_needs_a_reason_in_typescript_too() {
    let r = report(&["tests/fixtures/ts_suppress"]);
    let rg11 = r.findings.iter().find(|f| f.rule == "RG11").unwrap();
    assert!(!rg11.is_active());
    let rg09 = r.findings.iter().find(|f| f.rule == "RG09").unwrap();
    assert!(rg09.is_active());
    assert_eq!(rg09.ignored_suppression, Some(18));
    assert_eq!((r.summary.suppressed, r.summary.errors), (1, 1));
}

#[test]
fn findings_carry_the_typescript_fix_and_the_written_names() {
    let r = report(&["tests/fixtures/bad/RG04_account_write.ts"]);
    let f = &r.findings[0];
    assert!(f.message.starts_with("setAccount writes"), "{}", f.message);
    assert_eq!(f.suite, "lending");
    assert_eq!(f.tests, vec!["withdraws with its debt cleared"]);
    let rg08 = report(&["tests/fixtures/bad/RG08_no_assertion.ts"]);
    assert_eq!(rg08.findings[0].evidence, "it(\"drains the vault\")");
    assert!(
        rg08.findings[0]
            .message
            .contains("awaiting a transaction only checks"),
        "{}",
        rg08.findings[0].message
    );
}

#[test]
fn jest_node_and_legacy_anchor_forms_are_read() {
    let src = r#"
import * as anchor from "@coral-xyz/anchor";
import { Keypair } from "@solana/web3.js";
import assert from "node:assert";
const program = anchor.workspace.Vault;
const admin = Keypair.generate();
const attacker = Keypair.generate();
test("legacy namespace", async () => {
  await program.rpc.setFee(10_000, { accounts: { admin: admin.publicKey }, signers: [admin] });
  await program.rpc.withdraw(1, { accounts: { recipient: attacker.publicKey }, signers: [attacker] });
  expect(await program.provider.connection.getBalance(attacker.publicKey)).toBeGreaterThan(0);
  assert.strictEqual(1, 1);
});
"#;
    let r = source_report(src);
    let rules: Vec<(&str, u32)> = r.active().map(|f| (f.rule, f.line)).collect();
    assert_eq!(rules, vec![("RG02", 9), ("RG09", 12)], "{:?}", r.findings);
}

#[test]
fn transactions_sent_through_the_provider_carry_the_wallet_and_signers() {
    let src = r#"
import * as anchor from "@coral-xyz/anchor";
import { Keypair, Transaction, sendAndConfirmTransaction } from "@solana/web3.js";
import { assert } from "chai";
const provider = anchor.AnchorProvider.env();
const program = anchor.workspace.Vault;
const admin = Keypair.generate();
it("drains", async () => {
  const ix = await program.methods.setFee(10_000).accounts({ admin: admin.publicKey }).instruction();
  await provider.sendAndConfirm(new Transaction().add(ix), [admin]);
  const tx = new Transaction().add(await program.methods.sweep().instruction());
  await sendAndConfirmTransaction(provider.connection, tx, [provider.wallet.payer]);
  assert.equal((await program.account.vault.fetch(vault)).fees.toNumber(), 0);
});
"#;
    let r = source_report(src);
    let rules: Vec<(&str, u32)> = r.active().map(|f| (f.rule, f.line)).collect();
    // `sendAndConfirm` adds the admin to the wallet's signature, so the
    // instruction it sends is an admin step. Nothing set the protocol up
    // before it, so the wallet-only `sweep` after it is not RG03.
    assert_eq!(rules, vec![("RG02", 10)], "{:?}", r.findings);
}

/// The scene every TypeScript rule-page example runs in.
const EXAMPLE_PRELUDE: &str = r#"
import * as anchor from "@coral-xyz/anchor";
import { BN, Program } from "@coral-xyz/anchor";
import { Connection, Keypair, LAMPORTS_PER_SOL, PublicKey, clusterApiUrl } from "@solana/web3.js";
import { getAccount } from "@solana/spl-token";
import { LiteSVM } from "litesvm";
import { assert, expect } from "chai";

const provider = anchor.AnchorProvider.env();
const program = anchor.workspace.Vault as Program<any>;
const svm = new LiteSVM();
const admin = Keypair.generate();
const attacker = Keypair.generate();

it("example", async () => {
"#;

fn example_rules(snippet: &str) -> Vec<&'static str> {
    let r = source_report(&format!("{EXAMPLE_PRELUDE}{snippet}\n}});\n"));
    assert!(r.parse_errors.is_empty(), "{snippet}: {:?}", r.parse_errors);
    assert_eq!(r.summary.tests, 1, "{snippet}");
    r.active().map(|f| f.rule).collect()
}

/// Every TypeScript example in the rule pages does what the page says.
#[test]
fn typescript_examples_in_the_rule_pages_are_true() {
    use realism_gate::rules::{RULES, SolanaNotes};
    for r in RULES {
        let SolanaNotes::Checked {
            ts_bad, ts_good, ..
        } = r.solana
        else {
            continue;
        };
        let fired = example_rules(ts_bad);
        assert!(
            fired.contains(&r.id),
            "{}: bad example raised {fired:?}",
            r.id
        );
        let fired = example_rules(ts_good);
        assert!(
            !fired.contains(&r.id),
            "{}: good example raised {fired:?}",
            r.id
        );
    }
}

/// A scratch Anchor workspace: tests live in `tests/`.
fn anchor_workspace(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("tests")).unwrap();
    for (path, text) in files {
        std::fs::write(dir.join(path), text).unwrap();
    }
    dir
}

fn run(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_realism-gate"))
        .args(args)
        .current_dir(dir)
        .env("NO_COLOR", "1")
        .output()
        .unwrap()
}

#[test]
fn without_paths_an_anchor_tests_directory_is_checked() {
    let poc =
        std::fs::read_to_string(root().join("tests/fixtures/bad/RG01_attacker_holds_privilege.ts"))
            .unwrap();
    let dir = anchor_workspace(
        "ts-default-path",
        &[
            ("tests/vault.ts", &poc),
            ("Anchor.toml", "[programs.localnet]\n"),
        ],
    );
    let o = run(&dir, &["check", "--no-config", "--no-declaration"]);
    let out = String::from_utf8_lossy(&o.stdout);
    assert_eq!(o.status.code(), Some(1), "{out}");
    assert!(out.contains("tests/vault.ts:15:11"), "{out}");
    assert!(out.contains("RG01 attacker-holds-privilege"), "{out}");
}

#[test]
fn a_solana_script_that_does_not_parse_is_reported() {
    let dir = anchor_workspace(
        "ts-parse-error",
        &[(
            "tests/broken.ts",
            "import * as anchor from \"@coral-xyz/anchor\";\nit(\"x\", async () => { await ( });\n",
        )],
    );
    let o = run(
        &dir,
        &[
            "check",
            "--no-config",
            "--no-declaration",
            "--format",
            "json",
        ],
    );
    let report: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    let errors = report["parse_errors"].as_array().unwrap();
    assert_eq!(errors.len(), 1, "{report}");
    assert_eq!(errors[0]["file"], "tests/broken.ts");
}

#[test]
fn explain_shows_the_typescript_examples() {
    let o = run(&root(), &["explain", "RG03"]);
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(out.contains("Fires on (TypeScript)"), "{out}");
    assert!(out.contains("provider.wallet"), "{out}");
}

// Each case below is a false positive found by running the checker over
// public Anchor test suites, reduced to a few lines.

#[test]
fn a_deliberate_failure_after_a_call_that_should_throw_is_not_a_tautology() {
    let src = "try {\n  await program.methods.withdraw(new BN(1)).accounts({ recipient: attacker.publicKey }).signers([attacker]).rpc();\n  assert.ok(false);\n} catch (e) {\n  assert.include(e.message, \"Unauthorized\");\n}";
    assert!(!example_rules(src).contains(&"RG09"));
}

#[test]
fn a_flag_set_by_a_callback_is_not_a_constant() {
    let src = "let called = false;\nprogram.addEventListener(\"Drained\", () => { called = true; });\nawait program.methods.drain().accounts({ recipient: attacker.publicKey }).signers([attacker]).rpc();\nexpect(called).is.true;";
    assert!(!example_rules(src).contains(&"RG09"));
}

#[test]
fn the_provider_wallet_paying_the_fee_is_not_the_signer_that_authorizes() {
    // The wallet is the admin, and also pays for the attacker's transaction.
    let src = "await program.methods.initialize().accounts({ authority: provider.wallet.publicKey }).rpc();\nawait program.methods.withdraw(new BN(1)).accounts({ recipient: attacker.publicKey }).signers([attacker]).rpc();\nassert.equal((await program.account.vault.fetch(vault)).total.toNumber(), 0);";
    assert!(!example_rules(src).contains(&"RG02"));
    // Naming the wallet in the accounts makes its signature the one that
    // authorizes the instruction.
    let named = "await program.methods.initialize().accounts({ authority: provider.wallet.publicKey }).rpc();\nawait program.methods.withdraw(new BN(1)).accounts({ authority: provider.wallet.publicKey, recipient: attacker.publicKey }).signers([attacker]).rpc();\nassert.equal((await program.account.vault.fetch(vault)).total.toNumber(), 0);";
    assert!(example_rules(named).contains(&"RG02"));
}

#[test]
fn a_local_url_held_in_a_constant_is_not_live() {
    let src = "const ENDPOINT = \"http://localhost:8899\";\nconst conn = new Connection(ENDPOINT, { commitment: \"confirmed\" });\nassert.equal(await conn.getBalance(attacker.publicKey), 0);";
    assert!(!example_rules(src).contains(&"RG11"));
}

#[test]
fn a_helper_read_labelled_before_and_after_is_a_baseline() {
    let src = "const [before] = getBalances(attacker.publicKey, \"Beginning\");\nawait program.methods.withdraw(new BN(1)).accounts({ recipient: attacker.publicKey }).signers([attacker]).rpc();\nconst [after] = getBalances(attacker.publicKey, \"Resulting\");\nassert(after > before, \"gained\");";
    assert!(!example_rules(src).contains(&"RG10"));
}
