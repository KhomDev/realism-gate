//! realism-gate.toml, --severity, --exclude, --changed-since, the Markdown
//! format and the allow comment without parentheses.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// A scratch project with two test files: one error (RG06), one warning (RG11).
fn project(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("test")).unwrap();
    for f in ["RG06_balance_injection.t.sol", "RG11_unpinned_fork.t.sol"] {
        std::fs::copy(
            root().join("tests/fixtures/bad").join(f),
            dir.join("test").join(f),
        )
        .unwrap();
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

fn json(dir: &Path, args: &[&str]) -> Value {
    let mut a = vec!["check", "--format", "json", "--fail-on", "never"];
    a.extend(args);
    let o = run(dir, &a);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    serde_json::from_slice(&o.stdout).unwrap()
}

fn rules(v: &Value) -> Vec<(String, String)> {
    v["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            (
                f["rule"].as_str().unwrap().to_string(),
                f["severity"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

fn pair(r: &str, s: &str) -> (String, String) {
    (r.to_string(), s.to_string())
}

#[test]
fn config_sets_severity_and_turns_rules_off() {
    let dir = project("config");
    std::fs::write(
        dir.join("realism-gate.toml"),
        "[rules]\nRG11 = \"off\"\nbalance-injection = \"warn\"\n",
    )
    .unwrap();
    let v = json(&dir, &[]);
    assert_eq!(rules(&v), vec![pair("RG06", "warn")]);
    assert_eq!(v["config"], "realism-gate.toml");
    // Warn only, so the default threshold passes.
    assert_eq!(run(&dir, &["check"]).status.code(), Some(0));

    // Flags win: --severity replaces the level, --rules runs an "off" rule.
    let v = json(&dir, &["--severity", "RG06=error", "--rules", "RG06,RG11"]);
    assert_eq!(rules(&v), vec![pair("RG06", "error"), pair("RG11", "warn")]);
    assert_eq!(
        run(&dir, &["check", "--severity", "RG06=error"])
            .status
            .code(),
        Some(1)
    );

    // --no-config ignores the file.
    let v = json(&dir, &["--no-config"]);
    assert_eq!(rules(&v), vec![pair("RG06", "error"), pair("RG11", "warn")]);
    assert!(v["config"].is_null());
}

#[test]
fn a_bad_config_is_a_usage_error() {
    let dir = project("bad-config");
    std::fs::write(dir.join("realism-gate.toml"), "[rules]\nRG99 = \"off\"\n").unwrap();
    let o = run(&dir, &["check"]);
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).contains("RG99"));
    let o = run(&dir, &["check", "--no-config", "--severity", "RG01=loud"]);
    assert_eq!(o.status.code(), Some(2));
}

#[test]
fn exclude_drops_files_from_config_and_flags() {
    let dir = project("exclude");
    let v = json(&dir, &["--exclude", "test/RG06*"]);
    assert_eq!(rules(&v), vec![pair("RG11", "warn")]);
    assert_eq!(v["summary"]["suites"], 1);
    std::fs::write(
        dir.join("realism-gate.toml"),
        "exclude = [\"test/RG11_unpinned_fork.t.sol\"]\n",
    )
    .unwrap();
    let v = json(&dir, &[]);
    assert_eq!(rules(&v), vec![pair("RG06", "error")]);
    // The flag adds to the file. Excluding every suite leaves nothing
    // checked, which is an error, not a pass.
    let o = run(&dir, &["check", "--exclude", "test/RG06*"]);
    assert_eq!(o.status.code(), Some(2));
}

fn git(dir: &Path, args: &[&str]) {
    let o = Command::new("git")
        .args([
            "-c",
            "user.name=test",
            "-c",
            "user.email=test@example.invalid",
        ])
        .args(["-c", "commit.gpgsign=false", "-c", "core.autocrlf=false"])
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git is installed");
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
}

#[test]
fn changed_since_reports_only_changed_files() {
    let dir = project("changed");
    git(&dir, &["init", "-q"]);
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    let rg11 = dir.join("test/RG11_unpinned_fork.t.sol");
    let text = std::fs::read_to_string(&rg11).unwrap();
    std::fs::write(&rg11, format!("{text}\n// touched\n")).unwrap();

    let v = json(&dir, &["--changed-since", "HEAD"]);
    assert_eq!(rules(&v), vec![pair("RG11", "warn")]);
    assert_eq!(v["changed_since"]["ref"], "HEAD");
    assert_eq!(v["changed_since"]["files"], 1);
    // Everything was still read.
    assert_eq!(v["summary"]["suites"], 2);

    // Nothing changed: nothing reported, and that is a pass.
    git(&dir, &["commit", "-q", "-am", "touch"]);
    let v = json(&dir, &["--changed-since", "HEAD"]);
    assert!(rules(&v).is_empty());
    assert_eq!(
        run(&dir, &["check", "--changed-since", "HEAD"])
            .status
            .code(),
        Some(0)
    );

    // An unknown revision is a usage error, not a clean result.
    let o = run(&dir, &["check", "--changed-since", "no-such-ref"]);
    assert_eq!(o.status.code(), Some(2));
}

#[test]
fn markdown_summarises_for_a_pull_request() {
    let dir = project("markdown");
    let o = run(&dir, &["check", "--format", "markdown", "--no-config"]);
    assert_eq!(o.status.code(), Some(1));
    let md = String::from_utf8_lossy(&o.stdout);
    assert!(md.contains(": FAIL"), "{md}");
    assert!(
        md.contains("**1 error · 1 warning · 0 notes · 0 suppressed**"),
        "{md}"
    );
    assert!(md.contains("| error | [RG06]("), "{md}");
    assert!(md.contains("`test/RG06_balance_injection.t.sol:"), "{md}");
    let o = run(
        &dir,
        &["check", "--format", "markdown", "--fail-on", "never"],
    );
    assert!(String::from_utf8_lossy(&o.stdout).contains(": PASS"));
}

#[test]
fn allow_without_parentheses_and_ignored_allows_are_counted() {
    let dir = project("allow");
    let rg11 = dir.join("test/RG11_unpinned_fork.t.sol");
    let text = std::fs::read_to_string(&rg11).unwrap();
    let marked = text.replacen(
        "vm.createSelectFork(",
        "// realism-gate: allow RG11 the block is set in foundry.toml\n        vm.createSelectFork(",
        1,
    );
    assert_ne!(marked, text);
    std::fs::write(&rg11, marked).unwrap();
    let v = json(&dir, &[]);
    assert_eq!(v["summary"]["suppressed"], 1);
    assert_eq!(v["summary"]["suppressions_ignored"], 0);

    let v = json(
        &root(),
        &["tests/fixtures/suppress", "--no-declaration", "--no-config"],
    );
    assert_eq!(v["summary"]["suppressed"], 1);
    assert_eq!(v["summary"]["suppressions_ignored"], 1);
}
