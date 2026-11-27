//! The workspace split must not change a byte of machine-readable output.
//!
//! `tests/golden/manifest.json` was produced by `tests/golden/generate.py`
//! from the pre-split binary at commit 45ca25a, over every committed fixture:
//! each fixture directory with `--format json` and `--format sarif`, each
//! source file with `--format json`, the declaration and inline-allow
//! variants, and the catalog commands. This test re-runs every case with the
//! binary under test and compares stdout bytes (by SHA-256), the byte count,
//! and the exit code.
//!
//! stderr is compared only when the recorded case says so: a refused in-repo
//! `--declaration` names the checkout directory, which is machine-specific.
//! JSON stdout never embeds an absolute path; the generator refuses to write a
//! manifest if one does.

use serde::Deserialize;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Deserialize)]
struct Manifest {
    case_count: usize,
    cases: Vec<Case>,
}

#[derive(Debug, Deserialize)]
struct Case {
    argv: Vec<String>,
    exit: i32,
    stdout_sha256: String,
    stdout_bytes: usize,
    stderr_sha256: String,
    stderr_bytes: usize,
    compare_stderr: bool,
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn committed_fixtures_reproduce_their_pre_split_output() {
    let text = std::fs::read_to_string(root().join("tests/golden/manifest.json"))
        .expect("tests/golden/manifest.json; run tests/golden/generate.py");
    let manifest: Manifest = serde_json::from_str(&text).expect("golden manifest is JSON");
    assert_eq!(
        manifest.case_count,
        manifest.cases.len(),
        "manifest case_count does not match its cases"
    );
    assert!(
        manifest.cases.len() >= 100,
        "the golden manifest must cover the whole fixture tree, not {} cases",
        manifest.cases.len()
    );

    let mut failures = Vec::new();
    for (i, case) in manifest.cases.iter().enumerate() {
        let out = Command::new(env!("CARGO_BIN_EXE_realism-gate"))
            .args(&case.argv)
            .current_dir(root())
            .env("NO_COLOR", "1")
            .output()
            .unwrap_or_else(|e| panic!("case {i} {:?}: binary did not run: {e}", case.argv));

        let stdout_sha = realism_gate::render::sha256::hex(&out.stdout);
        let stderr_sha = realism_gate::render::sha256::hex(&out.stderr);
        let code = out.status.code();
        if code != Some(case.exit) {
            failures.push(format!(
                "case {i} {:?}: exit {code:?}, want {}",
                case.argv, case.exit
            ));
        }
        if out.stdout.len() != case.stdout_bytes || stdout_sha != case.stdout_sha256 {
            failures.push(format!(
                "case {i} {:?}: stdout {} bytes sha {stdout_sha}, want {} bytes sha {}\n--- stdout ---\n{}",
                case.argv,
                out.stdout.len(),
                case.stdout_bytes,
                case.stdout_sha256,
                String::from_utf8_lossy(&out.stdout)
            ));
        }
        if case.compare_stderr
            && (out.stderr.len() != case.stderr_bytes || stderr_sha != case.stderr_sha256)
        {
            failures.push(format!(
                "case {i} {:?}: stderr {} bytes sha {stderr_sha}, want {} bytes sha {}\n--- stderr ---\n{}",
                case.argv,
                out.stderr.len(),
                case.stderr_bytes,
                case.stderr_sha256,
                String::from_utf8_lossy(&out.stderr)
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} golden case(s) changed:\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}
