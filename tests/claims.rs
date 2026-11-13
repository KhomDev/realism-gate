//! Claims: parsing, binding, the acceptance fixture, the card and init-claims.
//!
//! `examples/rain-usdr` is the RAINUSDR-1622 case study reduced to four tests.
//! The acceptance test here is the plan's section 1.1 table: every claim
//! SHOWN except the fix row, which `check` cannot show.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn fixture() -> PathBuf {
    root().join("examples/rain-usdr")
}

fn run_in(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_realism-gate"))
        .args(args)
        .current_dir(dir)
        .env("NO_COLOR", "1")
        .output()
        .expect("binary runs")
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).to_string()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).to_string()
}

fn json(o: &Output) -> Value {
    serde_json::from_slice(&o.stdout).unwrap_or_else(|e| {
        panic!("not JSON ({e}): {}{}", stdout(o), stderr(o));
    })
}

fn unique() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rg-{name}-{}-{}", std::process::id(), unique()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let dest = to.join(entry.file_name());
        if path.is_dir() {
            copy_dir(&path, &dest);
        } else {
            std::fs::copy(&path, &dest).unwrap();
        }
    }
}

/// The target tree of the fixture as a throwaway git repo, the same way
/// examples/rain-usdr/make-target-repo.sh builds it: fixed identity, fixed
/// dates. Returns the directory and the commit.
fn target_repo() -> (PathBuf, String) {
    let dir = scratch("target");
    copy_dir(&fixture().join("target"), &dir);
    let git = |args: &[&str]| {
        Command::new("git")
            .args(args)
            .current_dir(&dir)
            .env("GIT_AUTHOR_DATE", "2026-11-12T00:00:00+00:00")
            .env("GIT_COMMITTER_DATE", "2026-11-12T00:00:00+00:00")
            .env("GIT_AUTHOR_NAME", "realism-gate fixture")
            .env("GIT_AUTHOR_EMAIL", "fixture@example.invalid")
            .env("GIT_COMMITTER_NAME", "realism-gate fixture")
            .env("GIT_COMMITTER_EMAIL", "fixture@example.invalid")
            .output()
            .expect("git runs")
    };
    assert!(git(&["init", "-q"]).status.success());
    assert!(git(&["add", "-A"]).status.success());
    let commit = git(&["commit", "-q", "-m", "RAIN-USDR target tree (fixture)"]);
    assert!(
        commit.status.success(),
        "git commit failed: {}",
        String::from_utf8_lossy(&commit.stderr)
    );
    let out = git(&["rev-parse", "HEAD"]);
    let sha = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (dir, sha)
}

/// Everything the fixture proves, in one command. This is the acceptance run.
fn rain_usdr(args: &[&str]) -> Output {
    let (target, commit) = target_repo();
    let mut all: Vec<String> = vec![
        "check".into(),
        "test".into(),
        "--report".into(),
        "finding.md".into(),
        "--target".into(),
        target.display().to_string(),
        "--target-commit".into(),
        commit,
        "--no-config".into(),
        "--no-declaration".into(),
        "--color".into(),
        "never".into(),
    ];
    all.extend(args.iter().map(|s| s.to_string()));
    let refs: Vec<&str> = all.iter().map(String::as_str).collect();
    run_in(&fixture(), &refs)
}

fn rows(v: &Value) -> &Vec<Value> {
    v["claims"]["rows"].as_array().expect("claims rows")
}

fn row<'a>(v: &'a Value, field: &str) -> &'a Value {
    rows(v)
        .iter()
        .find(|r| r["field"] == field)
        .unwrap_or_else(|| panic!("no {field} row in {}", v["claims"]))
}

#[test]
fn rain_usdr_is_all_shown_except_the_fix_row() {
    let o = rain_usdr(&["--format", "json"]);
    assert_eq!(o.status.code(), Some(0), "{}{}", stdout(&o), stderr(&o));
    let v = json(&o);
    assert_eq!(v["claim_binding"], "structured");
    assert_eq!(v["claims"]["schema"], "realism-gate.claims/v1");
    assert_eq!(v["claims"]["binding"], "structured");
    assert_eq!(v["claims"]["outcome"], "shown");
    assert_eq!(v["claims"]["source"], "claims.yaml");
    // One row per claim field, in table order.
    let fields: Vec<&str> = rows(&v)
        .iter()
        .map(|r| r["field"].as_str().unwrap())
        .collect();
    assert_eq!(
        fields,
        vec![
            "attacker",
            "impact",
            "asset",
            "amount",
            "preconditions",
            "cited",
            "fix"
        ]
    );
    // Every row SHOWN except the fix row: check has no patch to run.
    for r in rows(&v) {
        let field = r["field"].as_str().unwrap();
        let want = if field == "fix" { "not_shown" } else { "shown" };
        assert_eq!(r["status"], want, "row {field}: {}", r["evidence"]);
        assert!(
            !r["evidence"].as_str().unwrap().is_empty(),
            "{field} has no evidence"
        );
    }
    assert_eq!(v["claims"]["shown"], 6);
    assert_eq!(v["claims"]["not_shown"], 1);
    assert_eq!(v["claims"]["contradicted"], 0);
    assert_eq!(v["claims"]["not_checkable"], 0);
    // The PoC is clean: no rule at error, and the table is the finding.
    assert_eq!(v["summary"]["errors"], 0);
}

#[test]
fn the_table_prints_claim_evidence_status() {
    let o = rain_usdr(&[]);
    let out = stdout(&o);
    assert_eq!(o.status.code(), Some(0), "{out}");
    let header = out
        .lines()
        .find(|l| l.starts_with("CLAIM ") && l.contains("EVIDENCE"))
        .unwrap_or_else(|| panic!("no CLAIM header in {out}"));
    assert!(header.contains("EVIDENCE"), "{header}");
    assert!(header.contains("STATUS"), "{header}");
    assert!(out.contains("CLAIMS  claims.yaml  ·  structured"), "{out}");
    assert!(out.contains("NOT SHOWN"), "{out}");
    assert!(out.contains("SHOWN"), "{out}");
    assert!(
        out.contains("COMPLETE  every claim the PoC can show is shown; the fix was not run"),
        "{out}"
    );
    // The evidence is what the PoC writes, not a restatement of the claim.
    assert!(out.contains("expectRevert"), "{out}");
    assert!(out.contains("negative control"), "{out}");
    assert!(out.contains("assertEq(drawn, locked)"), "{out}");
}

#[test]
fn without_a_target_the_citation_claim_is_not_checkable() {
    let o = run_in(
        &fixture(),
        &[
            "check",
            "test",
            "--report",
            "finding.md",
            "--no-config",
            "--no-declaration",
            "--format",
            "json",
        ],
    );
    let v = json(&o);
    assert_eq!(row(&v, "cited")["status"], "not_checkable");
    assert_eq!(v["claims"]["outcome"], "incomplete");
    // Nothing is contradicted and the binding is structured, so the run is
    // not a failure on the claim table's account.
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
}

#[test]
fn a_contradicted_claim_exits_non_zero_and_no_fail_on_claims_is_the_escape() {
    let dir = scratch("contradicted");
    std::fs::write(
        dir.join("Poc.t.sol"),
        r#"interface IBalance { function distributeSurplus() external returns (uint256); }
error ReserveBackingShortfall(uint256, uint256);
contract Poc {
    IBalance sheet;
    function test_revertOnly() public {
        vm.expectRevert(ReserveBackingShortfall.selector);
        sheet.distributeSurplus();
    }
}
"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("claims.yaml"),
        "schema: realism-gate.claims/v1\nimpact: drain\n",
    )
    .unwrap();
    std::fs::write(dir.join("finding.md"), "# Drains the vault\n").unwrap();

    let o = run_in(
        &dir,
        &[
            "check",
            "Poc.t.sol",
            "--report",
            "finding.md",
            "--no-config",
            "--no-declaration",
            "--format",
            "json",
        ],
    );
    let v = json(&o);
    assert_eq!(
        row(&v, "impact")["status"],
        "contradicted",
        "{}",
        v["claims"]
    );
    assert_eq!(v["claims"]["outcome"], "contradicted");
    assert_eq!(o.status.code(), Some(1), "a contradiction is not a pass");
    assert!(stderr(&o).is_empty(), "{}", stderr(&o));

    // --fail-on never does not suppress it.
    let o = run_in(
        &dir,
        &[
            "check",
            "Poc.t.sol",
            "--report",
            "finding.md",
            "--no-config",
            "--no-declaration",
            "--fail-on",
            "never",
        ],
    );
    assert_eq!(o.status.code(), Some(1));
    assert!(stdout(&o).contains("CONTRADICTED"), "{}", stdout(&o));

    // The documented escape hatch: the table still says contradicted.
    let o = run_in(
        &dir,
        &[
            "check",
            "Poc.t.sol",
            "--report",
            "finding.md",
            "--no-config",
            "--no-declaration",
            "--format",
            "json",
            "--no-fail-on-claims",
        ],
    );
    assert_eq!(o.status.code(), Some(0));
    assert_eq!(json(&o)["claims"]["outcome"], "contradicted");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn inferred_claims_are_never_a_pass() {
    let dir = scratch("inferred");
    let poc = fixture().join("test/ShadowVault.t.sol");
    std::fs::copy(fixture().join("finding.md"), dir.join("finding.md")).unwrap();
    let o = run_in(
        &dir,
        &[
            "check",
            poc.to_str().unwrap(),
            "--report",
            "finding.md",
            "--no-config",
            "--no-declaration",
            "--format",
            "json",
        ],
    );
    let v = json(&o);
    assert_eq!(v["claim_binding"], "inferred");
    assert_eq!(v["claims"]["binding"], "inferred");
    assert_eq!(
        o.status.code(),
        Some(1),
        "an inferred binding is not a pass"
    );
    // Every field still gets a row, and the fields the prose cannot carry are
    // NOT CHECKABLE rather than guessed.
    assert_eq!(rows(&v).len(), 7);
    assert_eq!(row(&v, "amount")["status"], "not_checkable");
    assert_eq!(row(&v, "fix")["status"], "not_checkable");

    let out = stdout(&run_in(
        &dir,
        &[
            "check",
            poc.to_str().unwrap(),
            "--report",
            "finding.md",
            "--no-config",
            "--no-declaration",
        ],
    ));
    assert!(out.contains("INCOMPLETE  claims inferred"), "{out}");

    // The escape hatch still shows the same table.
    let o = run_in(
        &dir,
        &[
            "check",
            poc.to_str().unwrap(),
            "--report",
            "finding.md",
            "--no-config",
            "--no-declaration",
            "--no-fail-on-claims",
        ],
    );
    assert_eq!(o.status.code(), Some(0));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_front_matter_block_is_read_as_structured_claims() {
    let dir = scratch("frontmatter");
    let poc = fixture().join("test/ShadowVault.t.sol");
    std::fs::write(
        dir.join("finding.md"),
        "---\nschema: realism-gate.claims/v1\nattacker: any-user\nimpact: other:the call reverts\n---\n\n# A finding\n",
    )
    .unwrap();
    let o = run_in(
        &dir,
        &[
            "check",
            poc.to_str().unwrap(),
            "--report",
            "finding.md",
            "--no-config",
            "--no-declaration",
            "--format",
            "json",
        ],
    );
    let v = json(&o);
    assert_eq!(v["claim_binding"], "structured");
    assert!(
        v["claims"]["source"]
            .as_str()
            .unwrap()
            .starts_with("front matter of "),
        "{}",
        v["claims"]["source"]
    );
    assert_eq!(row(&v, "attacker")["status"], "shown");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn init_claims_writes_a_template_that_round_trips() {
    let dir = scratch("initclaims");
    let out = dir.join("claims.yaml");
    let o = run_in(
        &fixture(),
        &["init-claims", "finding.md", "--out", out.to_str().unwrap()],
    );
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let text = std::fs::read_to_string(&out).unwrap();
    assert!(text.contains("schema: realism-gate.claims/v1"), "{text}");
    for field in ["attacker", "impact", "cited", "fix"] {
        assert!(text.contains(field), "{field} missing from the template");
    }
    // The report's own citations are quoted back as a starting list.
    assert!(text.contains("VaultEngine.open"), "{text}");

    // A second run refuses to overwrite, and --force does.
    let again = run_in(
        &fixture(),
        &["init-claims", "finding.md", "--out", out.to_str().unwrap()],
    );
    assert_eq!(again.status.code(), Some(2));
    let forced = run_in(
        &fixture(),
        &[
            "init-claims",
            "finding.md",
            "--out",
            out.to_str().unwrap(),
            "--force",
        ],
    );
    assert_eq!(forced.status.code(), Some(0));

    // Uncomment one claim field: the file is valid YAML and binds structured.
    let filled: String = text
        .lines()
        .map(|line| match line.strip_prefix("# impact:") {
            Some(rest) => format!("impact:{rest}\n"),
            None => format!("{line}\n"),
        })
        .collect();
    std::fs::write(&out, &filled).unwrap();
    let poc = fixture().join("test/ShadowVault.t.sol");
    let o = run_in(
        &dir,
        &[
            "check",
            poc.to_str().unwrap(),
            "--report",
            fixture().join("finding.md").to_str().unwrap(),
            "--claims",
            out.to_str().unwrap(),
            "--no-config",
            "--no-declaration",
            "--format",
            "json",
            "--no-fail-on-claims",
        ],
    );
    let v = json(&o);
    assert_eq!(
        v["claim_binding"],
        "structured",
        "{}{}",
        stdout(&o),
        stderr(&o)
    );
    assert!(
        v["claims"]["source"]
            .as_str()
            .unwrap()
            .ends_with("claims.yaml")
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_report_named_as_the_claims_file_is_an_input_error() {
    // Guards the shape of the bug that shipped a broken Reproduce command: a
    // markdown report is not YAML, and naming it as --claims must fail loudly
    // instead of binding nothing.
    let o = run_in(
        &fixture(),
        &[
            "check",
            "test",
            "--report",
            "finding.md",
            "--claims",
            "finding.md",
            "--no-config",
            "--no-declaration",
        ],
    );
    assert_eq!(o.status.code(), Some(2));
    assert!(stderr(&o).contains("finding.md"), "{}", stderr(&o));
}

#[test]
fn the_card_is_reproducible_and_says_it_is_signed_by_nothing() {
    let dir = scratch("card");
    let card = dir.join("card.md");
    let card_json = dir.join("card.json");
    let (target, commit) = target_repo();
    let o = run_in(
        &fixture(),
        &[
            "check",
            "test",
            "--report",
            "finding.md",
            "--target",
            target.to_str().unwrap(),
            "--target-commit",
            &commit,
            "--card",
            card.to_str().unwrap(),
            "--card-json",
            card_json.to_str().unwrap(),
            "--no-config",
            "--no-declaration",
        ],
    );
    assert_eq!(o.status.code(), Some(0), "{}{}", stdout(&o), stderr(&o));

    let text = std::fs::read_to_string(&card).unwrap();
    assert!(text.contains("Signed by nothing"), "{text}");
    assert!(text.contains("## Claim table"), "{text}");
    assert!(text.contains("## Reproduce"), "{text}");
    // The rerun command must actually reproduce: --report only, no --claims
    // pointing at the report.
    assert!(text.contains("--report finding.md"), "{text}");
    assert!(!text.contains("--claims finding.md"), "{text}");
    assert!(text.contains("--target-commit"), "{text}");
    assert!(text.contains("NOT SHOWN"), "{text}");

    let v: Value = serde_json::from_str(&std::fs::read_to_string(&card_json).unwrap()).unwrap();
    assert_eq!(v["schema"], "realism-gate.card/v1");
    assert_eq!(v["tool"], "realism-gate");
    assert_eq!(v["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(v["poc_sha256"].as_str().unwrap().len(), 64);
    assert_eq!(v["report_sha256"].as_str().unwrap().len(), 64);
    assert_eq!(v["claims_sha256"].as_str().unwrap().len(), 64);
    assert_eq!(v["claim_binding"], "structured");
    assert_eq!(v["target_commit"], commit);
    assert_eq!(v["claims"]["rows"].as_array().unwrap().len(), 7);
    assert!(v["signed_by"].as_str().unwrap().starts_with("nothing"));
    assert!(v["rerun"].as_str().unwrap().contains("--report finding.md"));

    // The hashes are of the files as they are: touching the PoC changes the
    // PoC hash and the table stays.
    let alt = scratch("card-alt");
    let alt_card = alt.join("card.json");
    copy_dir(&fixture(), &alt);
    let poc_path = alt.join("test/ShadowVault.t.sol");
    let mut poc = std::fs::read_to_string(&poc_path).unwrap();
    poc.push_str("\n// a trailing comment changes the hash, not the table\n");
    std::fs::write(&poc_path, poc).unwrap();
    let (target2, commit2) = target_repo();
    let o = run_in(
        &alt,
        &[
            "check",
            "test",
            "--report",
            "finding.md",
            "--target",
            target2.to_str().unwrap(),
            "--target-commit",
            &commit2,
            "--card-json",
            alt_card.to_str().unwrap(),
            "--no-config",
            "--no-declaration",
        ],
    );
    assert_eq!(o.status.code(), Some(0));
    let alt_v: Value = serde_json::from_str(&std::fs::read_to_string(&alt_card).unwrap()).unwrap();
    assert_ne!(v["poc_sha256"], alt_v["poc_sha256"]);
    assert_eq!(v["claims"]["rows"], alt_v["claims"]["rows"]);
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&alt);
}

#[test]
fn a_card_without_claims_is_an_input_error() {
    let o = run_in(
        &fixture(),
        &[
            "check",
            "test",
            "--card",
            "card.md",
            "--no-config",
            "--no-declaration",
        ],
    );
    assert_eq!(o.status.code(), Some(2));
    assert!(
        stderr(&o).contains("--card needs --report"),
        "{}",
        stderr(&o)
    );
}
