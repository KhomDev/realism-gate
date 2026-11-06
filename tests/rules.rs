//! Every rule against its fixtures: each `bad/` file fires exactly its own
//! rule, at the expected line and severity, and nothing else; every `good/`
//! file fires nothing.

use realism_gate::Severity::{self, Error, Note, Warn};
use realism_gate::declaration::Declaration;
use realism_gate::engine::{LoadedDeclaration, Options, analyze};
use realism_gate::project::{Project, discover};
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn run(paths: &[&str], declaration: Option<&str>) -> Vec<(String, Severity, String, u32)> {
    let base = root();
    let paths: Vec<PathBuf> = paths.iter().map(|p| base.join(p)).collect();
    let files = discover(&paths).unwrap();
    let project = Project::load(&files, &base).unwrap();
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
    let report = analyze(&project, loaded.as_ref(), &Options::default());
    assert!(report.parse_errors.is_empty(), "{:?}", report.parse_errors);
    report
        .active()
        .map(|f| {
            let file = Path::new(&f.file)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string();
            (f.rule.to_string(), f.severity, file, f.line)
        })
        .collect()
}

fn expect(path: &str, want: &[(&str, Severity, u32)]) {
    let got: Vec<(String, Severity, u32)> = run(&[path], None)
        .into_iter()
        .map(|(r, s, _, l)| (r, s, l))
        .collect();
    let want: Vec<(String, Severity, u32)> = want
        .iter()
        .map(|(r, s, l)| (r.to_string(), *s, *l))
        .collect();
    assert_eq!(got, want, "{path}");
}

#[test]
fn rg01_attacker_holds_privilege() {
    expect(
        "tests/fixtures/bad/RG01_attacker_holds_privilege.t.sol",
        &[("RG01", Error, 17)],
    );
}

#[test]
fn rg02_privileged_step_in_exploit() {
    expect(
        "tests/fixtures/bad/RG02_privileged_step.t.sol",
        &[("RG02", Warn, 22)],
    );
}

#[test]
fn rg03_deployer_is_attacker() {
    expect(
        "tests/fixtures/bad/RG03_deployer_is_attacker.t.sol",
        &[("RG03", Warn, 18)],
    );
}

#[test]
fn rg04_storage_write() {
    expect(
        "tests/fixtures/bad/RG04_storage_write.t.sol",
        &[("RG04", Error, 22)],
    );
}

#[test]
fn rg05_code_replacement() {
    expect(
        "tests/fixtures/bad/RG05_code_replacement.t.sol",
        &[("RG05", Warn, 26)],
    );
}

#[test]
fn rg06_balance_injection() {
    expect(
        "tests/fixtures/bad/RG06_balance_injection.t.sol",
        &[("RG06", Error, 23)],
    );
}

#[test]
fn rg07_mocked_call() {
    expect(
        "tests/fixtures/bad/RG07_mocked_call.t.sol",
        &[("RG07", Warn, 21)],
    );
}

#[test]
fn rg08_no_assertion_error_and_log_only_warning() {
    expect(
        "tests/fixtures/bad/RG08_no_assertion.t.sol",
        &[("RG08", Error, 16), ("RG08", Warn, 20)],
    );
}

#[test]
fn rg09_tautological_assertion() {
    expect(
        "tests/fixtures/bad/RG09_tautological_assertion.t.sol",
        &[("RG09", Error, 20), ("RG09", Error, 21)],
    );
}

#[test]
fn rg10_no_baseline() {
    expect(
        "tests/fixtures/bad/RG10_no_baseline.t.sol",
        &[("RG10", Warn, 22)],
    );
}

#[test]
fn rg10_test_contract_ether_has_no_baseline() {
    expect(
        "tests/fixtures/bad/RG10_test_contract_ether.t.sol",
        &[("RG10", Warn, 19)],
    );
}

#[test]
fn rg11_unpinned_fork() {
    expect(
        "tests/fixtures/bad/RG11_unpinned_fork.t.sol",
        &[("RG11", Warn, 14)],
    );
}

#[test]
fn every_good_fixture_is_clean() {
    let got = run(&["tests/fixtures/good"], None);
    assert!(got.is_empty(), "{got:?}");
}

#[test]
fn every_code_rule_has_a_bad_fixture() {
    let files: Vec<String> = std::fs::read_dir(root().join("tests/fixtures/bad"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    for i in 1..=11 {
        let id = format!("RG{i:02}");
        assert!(
            files
                .iter()
                .any(|f| f.starts_with(&id) && f.ends_with(".sol")),
            "{id} has no Solidity fixture"
        );
        let checks_solana = realism_gate::rules::rule(&id).unwrap().checks_solana();
        for ext in [".rs", ".ts"] {
            let has = files.iter().any(|f| f.starts_with(&id) && f.ends_with(ext));
            assert_eq!(
                has, checks_solana,
                "{id}: a {ext} fixture exists exactly when the rule checks Solana tests"
            );
        }
    }
}

// Rust (Solana) fixtures: each fires exactly its own rule, as the Solidity
// ones do.

#[test]
fn rust_rg01_attacker_holds_privilege() {
    expect(
        "tests/fixtures/bad/RG01_attacker_holds_privilege.rs",
        &[("RG01", Error, 19)],
    );
}

#[test]
fn rust_rg02_privileged_signer() {
    expect(
        "tests/fixtures/bad/RG02_privileged_signer.rs",
        &[("RG02", Warn, 28)],
    );
}

#[test]
fn rust_rg03_payer_is_deployer() {
    expect(
        "tests/fixtures/bad/RG03_payer_is_deployer.rs",
        &[("RG03", Warn, 19)],
    );
}

#[test]
fn rust_rg04_account_write() {
    expect(
        "tests/fixtures/bad/RG04_account_write.rs",
        &[("RG04", Error, 26)],
    );
}

#[test]
fn rust_rg05_mock_program() {
    expect(
        "tests/fixtures/bad/RG05_mock_program.rs",
        &[("RG05", Warn, 17)],
    );
}

#[test]
fn rust_rg06_token_balance_injection() {
    expect(
        "tests/fixtures/bad/RG06_token_balance_injection.rs",
        &[("RG06", Error, 42)],
    );
}

#[test]
fn rust_rg08_no_assertion_error_and_print_only_warning() {
    expect(
        "tests/fixtures/bad/RG08_no_assertion.rs",
        &[("RG08", Error, 21), ("RG08", Warn, 27)],
    );
}

#[test]
fn rust_rg09_tautological_assertion() {
    expect(
        "tests/fixtures/bad/RG09_tautological_assertion.rs",
        &[("RG09", Error, 22), ("RG09", Error, 23)],
    );
}

#[test]
fn rust_rg10_no_baseline() {
    expect(
        "tests/fixtures/bad/RG10_no_baseline.rs",
        &[("RG10", Warn, 20)],
    );
}

#[test]
fn rust_rg11_live_rpc() {
    expect("tests/fixtures/bad/RG11_live_rpc.rs", &[("RG11", Warn, 20)]);
}

// Anchor TypeScript fixtures: the same rules, the same one-rule-per-file
// discipline.

#[test]
fn ts_rg01_attacker_holds_privilege() {
    expect(
        "tests/fixtures/bad/RG01_attacker_holds_privilege.ts",
        &[("RG01", Error, 15)],
    );
}

#[test]
fn ts_rg02_privileged_signer() {
    expect(
        "tests/fixtures/bad/RG02_privileged_signer.ts",
        &[("RG02", Warn, 23)],
    );
}

#[test]
fn ts_rg03_payer_is_deployer() {
    expect(
        "tests/fixtures/bad/RG03_payer_is_deployer.ts",
        &[("RG03", Warn, 14)],
    );
}

#[test]
fn ts_rg04_account_write() {
    expect(
        "tests/fixtures/bad/RG04_account_write.ts",
        &[("RG04", Error, 22)],
    );
}

#[test]
fn ts_rg05_mock_program() {
    expect(
        "tests/fixtures/bad/RG05_mock_program.ts",
        &[("RG05", Warn, 15)],
    );
}

#[test]
fn ts_rg06_token_balance_injection() {
    expect(
        "tests/fixtures/bad/RG06_token_balance_injection.ts",
        &[("RG06", Warn, 21)],
    );
}

#[test]
fn ts_rg08_no_assertion_error_and_print_only_warning() {
    expect(
        "tests/fixtures/bad/RG08_no_assertion.ts",
        &[("RG08", Error, 11), ("RG08", Warn, 15)],
    );
}

#[test]
fn ts_rg09_tautological_assertion() {
    expect(
        "tests/fixtures/bad/RG09_tautological_assertion.ts",
        &[("RG09", Error, 17), ("RG09", Error, 18)],
    );
}

#[test]
fn ts_rg10_no_baseline() {
    expect(
        "tests/fixtures/bad/RG10_no_baseline.ts",
        &[("RG10", Warn, 19)],
    );
}

#[test]
fn ts_rg11_live_rpc() {
    expect("tests/fixtures/bad/RG11_live_rpc.ts", &[("RG11", Warn, 15)]);
}

#[test]
fn a_finding_in_an_inherited_setup_is_reported_once_for_all_tests() {
    let base = root();
    let files = discover(&[base.join("tests/fixtures/inherit")]).unwrap();
    let project = Project::load(&files, &base).unwrap();
    let report = analyze(&project, None, &Options::default());
    assert_eq!(report.findings.len(), 1);
    let f = &report.findings[0];
    assert_eq!((f.rule, f.severity, f.line), ("RG06", Warn, 17));
    assert_eq!(f.file, "tests/fixtures/inherit/BaseFixture.sol");
    assert_eq!(f.tests, vec!["test_first", "test_second"]);
}

#[test]
fn suppressions_need_a_reason() {
    let base = root();
    let files = discover(&[base.join("tests/fixtures/suppress")]).unwrap();
    let project = Project::load(&files, &base).unwrap();
    let report = analyze(&project, None, &Options::default());
    let rg11 = report.findings.iter().find(|f| f.rule == "RG11").unwrap();
    assert_eq!(
        rg11.suppressed.as_ref().unwrap().reason,
        "the block is pinned in foundry.toml (fork_block_number)"
    );
    let rg04 = report.findings.iter().find(|f| f.rule == "RG04").unwrap();
    assert!(rg04.is_active());
    assert_eq!(rg04.ignored_suppression, Some(22));
    assert_eq!(report.summary.suppressed, 1);
    assert_eq!(report.summary.errors, 1);
}

#[test]
fn without_a_declaration_role_holders_and_keepers_are_flagged() {
    let got = run(&["tests/fixtures/declaration"], None);
    let rules: Vec<(&str, Severity, u32)> = got
        .iter()
        .map(|(r, s, _, l)| (r.as_str(), *s, *l))
        .collect();
    // mallet holds a granted role; keeper reads as one.
    assert_eq!(rules, vec![("RG02", Warn, 26), ("RG02", Warn, 28)]);
}

#[test]
fn a_declaration_replaces_the_heuristics_and_adds_boundary_rules() {
    let got = run(
        &["tests/fixtures/declaration"],
        Some("tests/fixtures/declaration/realism.yaml"),
    );
    let rules: Vec<(&str, Severity, &str, u32)> = got
        .iter()
        .map(|(r, s, f, l)| (r.as_str(), *s, f.as_str(), *l))
        .collect();
    assert_eq!(
        rules,
        vec![
            // mallet is now the declared attacker, and the owner grants it a role.
            ("RG01", Error, "KeeperSandwich.t.sol", 21),
            // The keeper's step is declared, so RG02 is silent.
            ("RG14", Note, "realism.yaml", 13),
            ("RG13", Warn, "realism.yaml", 16),
            ("RG12", Error, "realism.yaml", 19),
        ]
    );
}

#[test]
fn rule_selection_filters_findings() {
    let base = root();
    let files = discover(&[base.join("tests/fixtures/bad")]).unwrap();
    let project = Project::load(&files, &base).unwrap();
    let opts = Options {
        only: ["RG04".to_string(), "RG06".to_string()].into(),
        ..Options::default()
    };
    let report = analyze(&project, None, &opts);
    let mut rules: Vec<&str> = report.findings.iter().map(|f| f.rule).collect();
    rules.dedup();
    assert_eq!(rules, vec!["RG04", "RG06"]);

    let opts = Options {
        skip: ["RG08".to_string()].into(),
        ..Options::default()
    };
    let report = analyze(&project, None, &opts);
    assert!(report.findings.iter().all(|f| f.rule != "RG08"));
}

#[test]
fn output_paths_are_never_absolute() {
    let got = analyze(
        &Project::load(
            &discover(&[root().join("tests/fixtures/bad")]).unwrap(),
            &root(),
        )
        .unwrap(),
        None,
        &Options::default(),
    );
    for f in &got.findings {
        assert!(f.file.starts_with("tests/fixtures/"), "{}", f.file);
        assert!(!f.file.contains('\\'), "{}", f.file);
    }
}
