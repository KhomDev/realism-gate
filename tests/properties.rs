//! Properties that must hold for any input the generator picks, not only for
//! the fixtures a person thought of. Each property runs the real binary, a few
//! cases each, so the whole file stays under about 30 seconds.
//!
//! 1. The JSON report does not depend on the order the files are given.
//! 2. Adding an unrelated test function to a file does not change the findings
//!    of the tests that were already there.
//! 3. Every JSON report validates against docs/output-schema.json, and every
//!    SARIF report has the keys SARIF 2.1.0 requires.

#[path = "support/mod.rs"]
mod support;

use proptest::prelude::*;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use support::validate;

const CASES: u32 = 30;

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// Runs `realism-gate` with `args` in the repository root and returns stdout.
fn run(args: &[String]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_realism-gate"))
        .args(args)
        .current_dir(root())
        .env("NO_COLOR", "1")
        .output()
        .expect("binary runs");
    assert!(
        !String::from_utf8_lossy(&out.stderr).contains("panicked"),
        "panic: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn check_json(files: &[String]) -> Value {
    let mut args = vec!["check".to_string()];
    args.extend(files.iter().cloned());
    args.extend(["--no-declaration".into(), "--format".into(), "json".into()]);
    let text = run(&args);
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("not JSON ({e}): {text}"))
}

/// Every `.t.sol` fixture under `tests/fixtures/bad`, relative to the root.
fn bad_fixtures() -> Vec<String> {
    let mut found: Vec<String> = std::fs::read_dir(root().join("tests/fixtures/bad"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.to_string_lossy().ends_with(".t.sol"))
        .map(|p: PathBuf| {
            p.strip_prefix(root())
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect();
    found.sort();
    assert!(found.len() >= 6, "fixture set too small");
    found
}

/// A non-empty subset of `files`, in a random order.
fn subset_shuffled(files: Vec<String>) -> impl Strategy<Value = Vec<String>> {
    proptest::sample::subsequence(files.clone(), 1..=files.len()).prop_shuffle()
}

// ---------------------------------------------------------------- property 1

proptest! {
    #![proptest_config(ProptestConfig::with_cases(CASES))]

    /// The same set of files, in any order, gives the same JSON report.
    #[test]
    fn report_does_not_depend_on_file_order(order in subset_shuffled(bad_fixtures())) {
        let mut sorted = order.clone();
        sorted.sort();
        prop_assert_eq!(check_json(&order), check_json(&sorted));
    }
}

// ---------------------------------------------------------------- property 2

/// Unrelated test bodies. None of them shares a name with the original file.
const EXTRA_BODIES: &[&str] = &[
    "        assertTrue(true);",
    "        Vault v = new Vault();\n        v.setFee(1);\n        assertEq(v.fee(), 1);",
    "        vm.prank(address(0xBEEF));\n        assertTrue(true);",
    "",
];

/// The findings of the tests named in `keep`, without line numbers, so a
/// change that only moves a line is not a change of finding.
fn findings_of(report: &Value, keep: &BTreeSet<String>) -> Vec<Value> {
    let mut out: Vec<Value> = report["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| {
            f["tests"]
                .as_array()
                .is_some_and(|ts| ts.iter().any(|t| keep.contains(t.as_str().unwrap_or(""))))
        })
        .map(|f| {
            // A finding can name tests the other way round; compare on the
            // kept tests only.
            let tests: Vec<&Value> = f["tests"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|t| keep.contains(t.as_str().unwrap_or("")))
                .collect();
            json!({
                "rule": f["rule"],
                "severity": f["severity"],
                "suite": f["suite"],
                "message": f["message"],
                "tests": tests,
            })
        })
        .collect();
    out.sort_by_key(|v| v.to_string());
    out
}

/// The names of the test functions in a Solidity file, in order.
fn test_names(source: &str) -> BTreeSet<String> {
    source
        .lines()
        .filter_map(|l| {
            let l = l.trim_start();
            l.strip_prefix("function ")
                .and_then(|r| r.split('(').next())
                .filter(|n| n.starts_with("test"))
                .map(str::to_string)
        })
        .collect()
}

/// Inserts `function` just before the final `}` of the file, which closes the
/// last contract in it.
fn append_to_last_contract(source: &str, function: &str) -> String {
    let close = source.rfind('}').expect("a closing brace");
    format!("{}\n{function}\n{}", &source[..close], &source[close..])
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(CASES))]

    /// Adding an unrelated test function does not change the findings of the
    /// tests that were already in the file.
    #[test]
    fn an_unrelated_test_does_not_change_other_findings(
        fixture_index in 0usize..1000,
        body_index in 0usize..EXTRA_BODIES.len(),
        suffix in "[a-z]{1,8}",
    ) {
        let fixtures = bad_fixtures();
        let rel = &fixtures[fixture_index % fixtures.len()];
        let original = std::fs::read_to_string(root().join(rel)).unwrap();
        let names = test_names(&original);
        prop_assume!(!names.is_empty());

        let dir = std::env::temp_dir().join(format!(
            "rg-prop-{}-{}",
            std::process::id(),
            fixture_index
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let before_path = dir.join("Before.t.sol");
        let after_path = dir.join("After.t.sol");
        let extra = format!(
            "    function test_unrelated_{suffix}() public {{\n{}\n    }}",
            EXTRA_BODIES[body_index]
        );
        let extended = append_to_last_contract(&original, &extra);
        std::fs::write(&before_path, &original).unwrap();
        std::fs::write(&after_path, &extended).unwrap();

        let before = check_json(&[before_path.to_string_lossy().into_owned()]);
        let after = check_json(&[after_path.to_string_lossy().into_owned()]);
        let _ = std::fs::remove_dir_all(&dir);

        // Only the tests that existed before are compared. The new function is
        // not in `names`, so its own findings are excluded by construction.
        prop_assert_eq!(findings_of(&before, &names), findings_of(&after, &names));
    }
}

// ---------------------------------------------------------------- property 3

/// The SARIF 2.1.0 keys this file checks: a log has `version` ("2.1.0") and
/// `runs`; a run has `tool.driver.name`; each result has `message.text`; each
/// location's region has a 1-based `startLine`. These are the required parts
/// of the spec that realism-gate's output must always contain.
fn sarif_violations(log: &Value) -> Vec<String> {
    let mut bad = Vec::new();
    if log["version"] != "2.1.0" {
        bad.push(format!("version is {}", log["version"]));
    }
    let Some(runs) = log["runs"].as_array() else {
        bad.push("runs is missing or not an array".into());
        return bad;
    };
    for (i, run) in runs.iter().enumerate() {
        if !run["tool"]["driver"]["name"].is_string() {
            bad.push(format!("runs[{i}].tool.driver.name is missing"));
        }
        let results = run["results"].as_array().map(Vec::as_slice).unwrap_or(&[]);
        for (j, r) in results.iter().enumerate() {
            if !r["message"]["text"].is_string() {
                bad.push(format!("runs[{i}].results[{j}].message.text is missing"));
            }
            if let Some(locs) = r["locations"].as_array() {
                for loc in locs {
                    let line = &loc["physicalLocation"]["region"]["startLine"];
                    if line.as_u64().is_none_or(|n| n < 1) {
                        bad.push(format!("runs[{i}].results[{j}] startLine is not >= 1"));
                    }
                }
            }
        }
    }
    bad
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(CASES))]

    /// Any subset of the fixtures gives a JSON report that matches the
    /// published schema and a SARIF log with the required keys.
    #[test]
    fn every_report_validates(order in subset_shuffled(bad_fixtures())) {
        let mut args = vec!["check".to_string()];
        args.extend(order.iter().cloned());
        args.extend(["--no-declaration".into(), "--format".into(), "json".into()]);
        let json_text = run(&args);
        let report: Value = serde_json::from_str(&json_text).unwrap();
        let schema: Value = serde_json::from_str(
            &std::fs::read_to_string(root().join("docs/output-schema.json")).unwrap(),
        )
        .unwrap();
        let mut errors = Vec::new();
        validate(&schema, &report, "$", &mut errors);
        prop_assert!(errors.is_empty(), "schema: {errors:?}");

        let mut sarif_args = args.clone();
        let fmt = sarif_args.iter().position(|a| a == "json").unwrap();
        sarif_args[fmt] = "sarif".into();
        let log: Value = serde_json::from_str(&run(&sarif_args)).unwrap();
        prop_assert!(sarif_violations(&log).is_empty(), "sarif: {:?}", sarif_violations(&log));
    }
}
