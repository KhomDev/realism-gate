//! Rust (Solana) tests: what counts as a PoC, where findings are placed,
//! and how declarations, suppressions and filters apply to them.

use realism_gate::Severity::{self, Error, Note, Warn};
use realism_gate::declaration::Declaration;
use realism_gate::engine::{LoadedDeclaration, Options, Report, analyze};
use realism_gate::project::{Project, discover};
use regex::Regex;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn report(paths: &[&str], declaration: Option<&str>, opts: &Options) -> Report {
    let base = root();
    let paths: Vec<PathBuf> = paths.iter().map(|p| base.join(p)).collect();
    let project = Project::load(&discover(&paths).unwrap(), &base).unwrap();
    let text = declaration.map(|d| std::fs::read_to_string(base.join(d)).unwrap());
    let decl = text.as_deref().map(|t| Declaration::parse(t).unwrap());
    let loaded = match (&decl, &text, declaration) {
        (Some(d), Some(t), Some(p)) => Some(LoadedDeclaration {
            decl: d,
            path: p.to_string(),
            text: t,
        }),
        _ => None,
    };
    analyze(&project, loaded.as_ref(), opts)
}

fn active(r: &Report) -> Vec<(&str, Severity, &str, u32)> {
    r.active()
        .map(|f| (f.rule, f.severity, f.file.as_str(), f.line))
        .collect()
}

#[test]
fn only_tests_that_drive_a_solana_harness_are_checked() {
    let r = report(&["tests/fixtures/good"], None, &Options::default());
    let rust_files = r
        .findings
        .iter()
        .filter(|f| f.file.ends_with(".rs"))
        .count();
    assert_eq!(rust_files, 0);
    // The LiteSVM file holds a PoC and a plain unit test; only the PoC counts.
    let only_rust = report(
        &[
            "tests/fixtures/good/SolanaLiteSvmExploit.rs",
            "tests/fixtures/good/SolanaProgramTestExploit.rs",
            "tests/fixtures/good/SolanaMolluskCheck.rs",
        ],
        None,
        &Options::default(),
    );
    assert_eq!((only_rust.summary.suites, only_rust.summary.tests), (3, 3));
}

#[test]
fn a_shared_helper_finding_is_placed_where_the_test_calls_it() {
    let r = report(&["tests/fixtures/rust_shared"], None, &Options::default());
    assert_eq!(
        active(&r),
        vec![("RG02", Warn, "tests/fixtures/rust_shared/exploit.rs", 11)]
    );
    let f = &r.findings[0];
    assert_eq!(f.tests, vec!["test_pause_then_withdraw"]);
    assert_eq!(
        f.evidence,
        "send(&mut w.svm, pause_ix(&w.admin.pubkey()), &[&w.admin])"
    );
    assert_eq!(f.suite, "exploit");
    assert_eq!((r.summary.suites, r.summary.tests), (1, 2));
}

#[test]
fn rust_findings_carry_the_rust_fix() {
    let r = report(
        &["tests/fixtures/bad/RG06_token_balance_injection.rs"],
        None,
        &Options::default(),
    );
    assert_eq!(
        r.findings[0].help,
        "Fund the attacker's own accounts, then transfer from the attacker to the program's account."
    );
}

#[test]
fn a_declaration_names_the_attacker_and_silences_the_declared_role() {
    let without = report(
        &["tests/fixtures/rust_declaration"],
        None,
        &Options::default(),
    );
    assert_eq!(
        active(&without),
        vec![(
            "RG02",
            Warn,
            "tests/fixtures/rust_declaration/keeper_harvest.rs",
            33
        )]
    );
    let with = report(
        &["tests/fixtures/rust_declaration"],
        Some("tests/fixtures/declaration/realism.yaml"),
        &Options::default(),
    );
    let yaml = "tests/fixtures/declaration/realism.yaml";
    assert_eq!(
        active(&with),
        vec![
            ("RG14", Note, yaml, 13),
            ("RG13", Warn, yaml, 16),
            ("RG12", Error, yaml, 19),
        ]
    );
}

#[test]
fn allow_comments_work_in_rust_and_need_a_reason() {
    let r = report(&["tests/fixtures/rust_suppress"], None, &Options::default());
    let rg11 = r.findings.iter().find(|f| f.rule == "RG11").unwrap();
    assert_eq!(
        rg11.suppressed.as_ref().unwrap().reason,
        "RPC_URL points at a local validator started from a pinned snapshot"
    );
    let rg04 = r.findings.iter().find(|f| f.rule == "RG04").unwrap();
    assert!(rg04.is_active());
    assert_eq!(rg04.ignored_suppression, Some(25));
    assert_eq!(
        (r.summary.suppressed, r.summary.suppressions_ignored),
        (1, 1)
    );
}

#[test]
fn match_filters_apply_to_rust_suites_and_tests() {
    let opts = Options {
        match_test: Some(Regex::new("prints").unwrap()),
        ..Options::default()
    };
    let r = report(&["tests/fixtures/bad/RG08_no_assertion.rs"], None, &opts);
    assert_eq!(r.summary.tests, 1);
    assert_eq!(
        active(&r),
        vec![("RG08", Warn, "tests/fixtures/bad/RG08_no_assertion.rs", 27)]
    );
    let opts = Options {
        match_contract: Some(Regex::new("^RG10_").unwrap()),
        ..Options::default()
    };
    let r = report(&["tests/fixtures/bad"], None, &opts);
    assert_eq!((r.summary.suites, r.summary.tests), (1, 1));
}

#[test]
fn a_rust_file_that_does_not_parse_is_reported() {
    let p = Project::from_sources(&[("tests/broken.rs", "#[test] fn t( {")]);
    let r = analyze(&p, None, &Options::default());
    assert_eq!(r.parse_errors.len(), 1);
    assert_eq!(r.parse_errors[0].file, "tests/broken.rs");
    assert_eq!(r.summary.suites, 0);
}

/// The harness every documented example runs in: a LiteSVM and a
/// solana-program-test setup, the usual `send` and `process` wrappers, and the
/// keypairs the examples name.
const EXAMPLE_PRELUDE: &str = r#"
fn send(svm: &mut LiteSVM, ix: &Instruction, signers: &[&Keypair]) {
    let tx = Transaction::new_signed_with_payer(&[ix.clone()], Some(&signers[0].pubkey()), signers, svm.latest_blockhash());
    svm.send_transaction(tx).unwrap();
}

async fn process(banks: &mut BanksClient, ixs: &[Instruction], signers: &[&Keypair]) {
    let tx = Transaction::new_signed_with_payer(ixs, Some(&signers[0].pubkey()), signers, Hash::default());
    banks.process_transaction(tx).await.unwrap();
}

#[tokio::test]
async fn test_example() {
    let mut program_test = ProgramTest::new("vault", vault::ID, None);
    let mut svm = LiteSVM::new();
    let admin = Keypair::new();
    let attacker = Keypair::new();
"#;

fn example_rules(snippet: &str) -> Vec<&'static str> {
    let src = format!("{EXAMPLE_PRELUDE}{snippet}\n}}\n");
    let p = Project::from_sources(&[("tests/example.rs", &src)]);
    let r = analyze(&p, None, &Options::default());
    assert!(r.parse_errors.is_empty(), "{snippet}: {:?}", r.parse_errors);
    assert_eq!(r.summary.tests, 1, "{snippet}");
    r.active().map(|f| f.rule).collect()
}

/// Every Rust example in the rule pages does what the page says: the "fires
/// on" one raises the rule, the "does not fire on" one does not.
#[test]
fn rust_examples_in_the_rule_pages_are_true() {
    use realism_gate::rules::{RULES, RustNotes};
    for r in RULES {
        let RustNotes::Checked { bad, good, .. } = r.rust else {
            continue;
        };
        let fired = example_rules(bad);
        assert!(
            fired.contains(&r.id),
            "{}: bad example raised {fired:?}",
            r.id
        );
        let fired = example_rules(good);
        assert!(
            !fired.contains(&r.id),
            "{}: good example raised {fired:?}",
            r.id
        );
    }
}

// Each case below is a false positive found by running the checker over
// public Solana test suites, reduced to a few lines.

#[test]
fn an_attacker_granting_itself_a_role_is_the_attack_not_the_harness() {
    // The attacker tries the access-control bug itself.
    let own = "send(&mut svm, &set_admin_ix(&attacker.pubkey()), &[&attacker]);";
    assert!(!example_rules(own).contains(&"RG01"));
    // A role holder grants it: the harness made the attacker privileged.
    let granted = "send(&mut svm, &set_admin_ix(&attacker.pubkey()), &[&admin]);";
    assert!(example_rules(granted).contains(&"RG01"));
    // The attacker initializes the program as its own admin.
    let init =
        "send(&mut svm, &instruction::Initialize { admin: attacker.pubkey() }, &[&attacker]);";
    assert!(example_rules(init).contains(&"RG01"));
    // No transaction the tool can see: who grants it is unknown.
    let unseen = "let ix = set_admin_ix(&attacker.pubkey()); bench.run(&[ix], None);";
    assert!(!example_rules(unseen).contains(&"RG01"));
}

#[test]
fn a_users_own_account_field_is_not_a_role() {
    let src = "send(&mut svm, &instruction::InitObligation { owner: alice.pubkey() }, &[&admin]);\n\
               send(&mut svm, &borrow_ix(&alice.pubkey(), 1), &[&alice]);";
    assert!(!example_rules(src).contains(&"RG02"));
}

#[test]
fn a_method_with_another_arity_is_not_inlined() {
    // `liquidity.borrow(amount)` is a library call, not the fixture's
    // four-parameter `borrow`, which sends an admin-signed transaction.
    let p = Project::from_sources(&[(
        "tests/t.rs",
        r#"
        struct Fixture;
        impl Fixture {
            fn borrow(&self, svm: &mut LiteSVM, admin: &Keypair, amount: u64) {
                let tx = Transaction::new_signed_with_payer(&[ix], None, &[admin], h);
            }
        }
        #[test]
        fn test_a() {
            let mut svm = LiteSVM::new();
            let x = reserve.liquidity.borrow(5);
            assert_eq!(x, y);
        }
        "#,
    )]);
    let r = analyze(&p, None, &Options::default());
    assert!(r.active().next().is_none(), "{:?}", r.findings);
}

#[test]
fn a_keypairs_pubkey_is_not_a_fixed_address() {
    let src = "let payer = Keypair::new();\n\
               svm.set_account(payer.pubkey(), Account { lamports: 5, ..Default::default() }).unwrap();\n\
               svm.set_account(pubkey!(\"Va111111111111111111111111111111111111111111\"), Account { lamports: 5, ..Default::default() }).unwrap();";
    let p =
        Project::from_sources(&[("tests/example.rs", &format!("{EXAMPLE_PRELUDE}{src}\n}}\n"))]);
    let r = analyze(&p, None, &Options::default());
    let rg06: Vec<_> = r.active().filter(|f| f.rule == "RG06").collect();
    // Only the second write, to a fixed address, is a balance injection.
    assert_eq!(rg06.len(), 1, "{:?}", r.findings);
    assert!(rg06[0].evidence.contains("pubkey!"), "{:?}", rg06[0]);
    assert!(rg06[0].message.contains("a fixed address"), "{:?}", rg06[0]);
}

#[test]
fn an_account_owned_by_a_loaded_program_names_that_program() {
    let src = "let counter = Pubkey::new_unique();\n\
               svm.set_account(counter, Account { lamports: 5, data: vec![0; 4], owner: program_id, ..Default::default() }).unwrap();";
    let p =
        Project::from_sources(&[("tests/example.rs", &format!("{EXAMPLE_PRELUDE}{src}\n}}\n"))]);
    let r = analyze(&p, None, &Options::default());
    let f = r.active().find(|f| f.rule == "RG04").expect("RG04");
    assert!(
        f.message.contains("owned by the program `program_id`"),
        "{}",
        f.message
    );
}

/// A scratch Cargo-style project: tests live in `tests/`.
fn cargo_project(name: &str, files: &[(&str, &str)]) -> PathBuf {
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
fn without_paths_a_cargo_tests_directory_is_checked() {
    let poc =
        std::fs::read_to_string(root().join("tests/fixtures/bad/RG10_no_baseline.rs")).unwrap();
    let dir = cargo_project("rust-default-path", &[("tests/exploit.rs", &poc)]);
    let o = run(&dir, &["check", "--no-config", "--no-declaration"]);
    let out = String::from_utf8_lossy(&o.stdout);
    assert_eq!(o.status.code(), Some(0), "{out}");
    assert!(out.contains("tests/exploit.rs:20:5"), "{out}");
    assert!(out.contains("RG10 no-baseline"), "{out}");
}

#[test]
fn plain_rust_tests_alone_are_no_test_suite() {
    let dir = cargo_project(
        "rust-no-poc",
        &[(
            "tests/math.rs",
            "#[test] fn adds() { assert_eq!(1 + 1, 2); }",
        )],
    );
    let o = run(&dir, &["check", "--no-config", "--no-declaration"]);
    assert_eq!(o.status.code(), Some(2));
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(err.contains("no test suite found"), "{err}");
}

#[test]
fn explain_and_rules_describe_the_rust_checks() {
    let o = run(&root(), &["explain", "RG04"]);
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(out.contains("In Rust (Solana) tests"), "{out}");
    assert!(out.contains("svm.set_account(pool_pda, pool)"), "{out}");
    let o = run(&root(), &["explain", "RG07"]);
    assert!(String::from_utf8_lossy(&o.stdout).contains("Not checked."));
    let o = run(&root(), &["rules", "--format", "json"]);
    let rules: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    let langs = |id: &str| {
        rules
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == id)
            .unwrap()["languages"]
            .clone()
    };
    assert_eq!(langs("RG04"), serde_json::json!(["solidity", "rust"]));
    assert_eq!(langs("RG07"), serde_json::json!(["solidity"]));
}
