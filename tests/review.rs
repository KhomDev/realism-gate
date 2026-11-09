//! The PoCs from the external review, with the findings this version raises.
//!
//! `expected.json` is the contract: RG03 on these files is a note (the
//! deployer calls an ordinary function), and the profit and the unevidenced
//! maker setup are warnings.

use realism_gate::engine::{Options, analyze};
use realism_gate::project::{Project, discover};
use serde_json::Value;
use std::path::PathBuf;

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
    for (g, w) in got.iter().zip(want) {
        assert_eq!(g, w);
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
