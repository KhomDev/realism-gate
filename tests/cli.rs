//! The binary: exit codes, output formats, and subcommands.

use std::path::PathBuf;
use std::process::{Command, Output};

fn bin() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_realism-gate"));
    cmd.current_dir(env!("CARGO_MANIFEST_DIR"));
    cmd.env("NO_COLOR", "1");
    cmd
}

fn run(args: &[&str]) -> Output {
    bin().args(args).output().expect("binary runs")
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).to_string()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).to_string()
}

#[test]
fn clean_input_exits_zero() {
    let o = run(&["check", "tests/fixtures/good", "--no-declaration"]);
    assert_eq!(o.status.code(), Some(0), "{}", stdout(&o));
    assert!(stdout(&o).contains("0 errors"));
    assert!(stdout(&o).contains("PASS"));
}

#[test]
fn an_error_exits_one() {
    let o = run(&[
        "check",
        "tests/fixtures/bad/RG06_balance_injection.t.sol",
        "--no-declaration",
    ]);
    assert_eq!(o.status.code(), Some(1));
    let out = stdout(&o);
    assert!(out.contains("RG06 balance-injection"), "{out}");
    assert!(out.contains("FAIL"));
}

#[test]
fn fail_on_controls_the_threshold() {
    let warn_only = "tests/fixtures/bad/RG11_unpinned_fork.t.sol";
    let o = run(&["check", warn_only, "--no-declaration"]);
    assert_eq!(o.status.code(), Some(0));
    let o = run(&["check", warn_only, "--no-declaration", "--fail-on", "warn"]);
    assert_eq!(o.status.code(), Some(1));
    let o = run(&[
        "check",
        "tests/fixtures/bad",
        "--no-declaration",
        "--fail-on",
        "never",
    ]);
    assert_eq!(o.status.code(), Some(0));
}

#[test]
fn json_output_has_a_stable_envelope() {
    let o = run(&[
        "check",
        "tests/fixtures/bad/RG04_storage_write.t.sol",
        "--no-declaration",
        "--format",
        "json",
    ]);
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["tool"], "realism-gate");
    assert_eq!(v["schema"], 1);
    assert_eq!(v["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(v["summary"]["errors"], 1);
    let f = &v["findings"][0];
    assert_eq!(f["rule"], "RG04");
    assert_eq!(f["slug"], "storage-write");
    assert_eq!(f["severity"], "error");
    assert_eq!(f["file"], "tests/fixtures/bad/RG04_storage_write.t.sol");
    assert_eq!(f["line"], 22);
    assert_eq!(f["suite"], "DebtClearTest");
    assert_eq!(f["tests"][0], "test_debtCanBeCleared");
    assert!(f.get("suppressed").is_none());
}

#[test]
fn sarif_output_is_sarif_2_1_0() {
    let o = run(&[
        "check",
        "tests/fixtures/suppress",
        "--no-declaration",
        "--format",
        "sarif",
    ]);
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["version"], "2.1.0");
    let run = &v["runs"][0];
    assert_eq!(run["tool"]["driver"]["name"], "realism-gate");
    assert_eq!(run["tool"]["driver"]["rules"].as_array().unwrap().len(), 18);
    let results = run["results"].as_array().unwrap();
    assert_eq!(results.len(), 2);
    let suppressed = results.iter().find(|r| r["ruleId"] == "RG11").unwrap();
    assert_eq!(suppressed["suppressions"][0]["kind"], "inSource");
    let active = results.iter().find(|r| r["ruleId"] == "RG04").unwrap();
    assert_eq!(active["level"], "error");
    assert_eq!(
        active["locations"][0]["physicalLocation"]["region"]["startLine"],
        22
    );
}

#[test]
fn output_file_is_written() {
    let dir = std::env::temp_dir().join(format!("rg-out-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path: PathBuf = dir.join("report.json");
    let o = run(&[
        "check",
        "tests/fixtures/good",
        "--no-declaration",
        "--format",
        "json",
        "--output",
        path.to_str().unwrap(),
    ]);
    assert_eq!(o.status.code(), Some(0));
    assert!(stdout(&o).is_empty());
    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(v["summary"]["errors"], 0);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn match_test_and_match_contract_filter() {
    let o = run(&[
        "check",
        "tests/fixtures/bad",
        "--no-declaration",
        "--format",
        "json",
        "--match-test",
        "logsOnly",
    ]);
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["summary"]["tests"], 1);
    assert_eq!(v["findings"].as_array().unwrap().len(), 1);
    let o = run(&[
        "check",
        "tests/fixtures/bad",
        "--no-declaration",
        "--format",
        "json",
        "--match-contract",
        "^SweepTest$",
    ]);
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["summary"]["suites"], 1);
    assert_eq!(v["findings"][0]["rule"], "RG03");
}

#[test]
fn usage_errors_exit_two() {
    let o = run(&["check", "does/not/exist"]);
    assert_eq!(o.status.code(), Some(2));
    assert!(stderr(&o).contains("no such file or directory"));

    let o = run(&["check", "tests/fixtures/good", "--rules", "RG99"]);
    assert_eq!(o.status.code(), Some(2));
    assert!(stderr(&o).contains("unknown rule `RG99`"));

    let o = run(&["check", "tests/fixtures/good", "--match-test", "("]);
    assert_eq!(o.status.code(), Some(2));
}

#[test]
fn finding_no_test_suite_is_an_error_not_a_pass() {
    let dir = std::env::temp_dir().join(format!("rg-empty-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("Vault.sol"),
        "contract Vault { function deposit() external {} }",
    )
    .unwrap();
    let o = run(&["check", dir.to_str().unwrap(), "--no-declaration"]);
    assert_eq!(o.status.code(), Some(2));
    assert!(stderr(&o).contains("no test suite found"), "{}", stderr(&o));

    let o = run(&[
        "check",
        "tests/fixtures/good",
        "--no-declaration",
        "--match-contract",
        "^NoSuchSuite$",
    ]);
    assert_eq!(o.status.code(), Some(2));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn an_invalid_declaration_exits_two() {
    let dir = std::env::temp_dir().join(format!("rg-decl-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("realism.yaml");
    std::fs::write(&path, "version: 1\nattacker:\n  idents: [x]\n").unwrap();
    let o = run(&[
        "check",
        "tests/fixtures/good",
        "--declaration",
        path.to_str().unwrap(),
    ]);
    assert_eq!(o.status.code(), Some(2));
    assert!(stderr(&o).contains("realism.yaml"), "{}", stderr(&o));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn rules_lists_every_rule() {
    let o = run(&["rules"]);
    let out = stdout(&o);
    assert_eq!(out.lines().count(), 18);
    assert!(out.starts_with("RG01"));
    assert!(out.contains("RG18"));
    let o = run(&["rules", "--format", "json"]);
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 18);
    let langs = v
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == "RG01")
        .unwrap();
    assert!(
        langs["languages"]
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l == "move")
    );
}

#[test]
fn explain_accepts_ids_and_slugs() {
    let by_id = stdout(&run(&["explain", "RG06"]));
    let by_slug = stdout(&run(&["explain", "balance-injection"]));
    assert_eq!(by_id, by_slug);
    assert!(by_id.contains("Known false-positive modes"));
    let md = stdout(&run(&["explain", "RG06", "--markdown"]));
    assert!(md.starts_with("# RG06 `balance-injection`"));
    assert_eq!(run(&["explain", "nope"]).status.code(), Some(2));
}

#[test]
fn init_writes_a_template_and_refuses_to_overwrite() {
    let dir = std::env::temp_dir().join(format!("rg-init-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let exe = env!("CARGO_BIN_EXE_realism-gate");
    let o = Command::new(exe)
        .arg("init")
        .current_dir(&dir)
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(0));
    let text = std::fs::read_to_string(dir.join("realism.yaml")).unwrap();
    assert!(realism_gate::declaration::Declaration::parse(&text).is_ok());
    let o = Command::new(exe)
        .arg("init")
        .current_dir(&dir)
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(2));
    let o = Command::new(exe)
        .args(["init", "--force"])
        .current_dir(&dir)
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(0));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn both_binaries_behave_identically() {
    let a = run(&["rules"]);
    let b = Command::new(env!("CARGO_BIN_EXE_rgate"))
        .arg("rules")
        .output()
        .unwrap();
    assert_eq!(a.stdout, b.stdout);
}
