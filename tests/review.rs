//! The PoCs from the external review, with the findings this version raises.
//!
//! `expected.json` is the contract: RG03 on these files is a note (the
//! deployer calls an ordinary function), and the profit and the unevidenced
//! maker setup are warnings.

use realism_gate::Severity;
use realism_gate::engine::{Options, analyze};
use realism_gate::project::{Project, discover};
use serde_json::Value;
use std::path::PathBuf;

/// The severity a finding ships at: the rule's own level `raw`, capped by the
/// RG-A5 severity floor for the rule in the file's language
/// (`rules::measured`, derived from `data/measured.json`). While every
/// committed cell is in-sample the floor is `note`, so this is `note`; when a
/// fresh FINAL cell lifts a rule, the raw level written in the test is what
/// the assertion checks again.
fn floored(rule: &str, file: &str, raw: Severity) -> Severity {
    realism_gate::rules::measured::shipped(
        rule,
        realism_gate::rules::measured::lang_of_path(file),
        raw,
    )
}

fn parse_level(s: &str) -> Severity {
    match s {
        "error" => Severity::Error,
        "warn" => Severity::Warn,
        "note" => Severity::Note,
        other => panic!("expected.json level {other:?}"),
    }
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn review_pocs_match_the_expected_findings() {
    let base = root();
    let files = discover(&[base.join("tests/fixtures/review")]).unwrap();
    let sol: Vec<_> = files
        .into_iter()
        .filter(|p| p.extension().is_some_and(|e| e == "sol"))
        .collect();
    let project = Project::load(&sol, &base).unwrap();
    let report = analyze(&project, None, &Options::default());
    assert!(report.parse_errors.is_empty(), "{:?}", report.parse_errors);
    let mut got: Vec<Value> = report
        .active()
        .map(|f| {
            serde_json::json!({
                "file": PathBuf::from(&f.file).file_name().unwrap().to_string_lossy(),
                "rule": f.rule,
                "severity": f.severity.as_str(),
                "line": f.line,
            })
        })
        .collect();
    got.sort_by(|a, b| {
        (a["file"].as_str(), a["line"].as_u64(), a["rule"].as_str()).cmp(&(
            b["file"].as_str(),
            b["line"].as_u64(),
            b["rule"].as_str(),
        ))
    });
    let expected: Value = serde_json::from_str(
        &std::fs::read_to_string(base.join("tests/fixtures/review/expected.json")).unwrap(),
    )
    .unwrap();
    let want = expected["findings"].as_array().unwrap();
    assert_eq!(got.len(), want.len(), "finding count");
    // expected.json records each rule's own level; it ships capped by the floor.
    for (g, w) in got.iter().zip(want) {
        let mut w = w.clone();
        let raw = parse_level(w["severity"].as_str().unwrap());
        let level = floored(
            w["rule"].as_str().unwrap(),
            w["file"].as_str().unwrap(),
            raw,
        );
        w["severity"] = Value::from(level.as_str());
        assert_eq!(g, &w);
    }
    let rg03: Vec<_> = got.iter().filter(|f| f["rule"] == "RG03").collect();
    assert_eq!(rg03.len(), 9, "the review's nine RG03 hits");
    assert!(rg03.iter().all(|f| f["severity"] == "note"));
    assert!(
        got.iter()
            .any(|f| f["rule"] == "RG15" && f["file"] == "PocReplay.t.sol")
    );
    assert!(got.iter().any(|f| f["rule"] == "RG17"));
}
