//! The JSON output matches docs/output-schema.json.
//!
//! The validator in tests/support covers the subset of JSON Schema the file uses (type,
//! enum, required, properties, additionalProperties, items, oneOf), so the
//! check needs no extra dependency.

mod support;

use serde_json::Value;
use std::process::Command;

use support::validate;

fn report(args: &[&str]) -> Value {
    let out = Command::new(env!("CARGO_BIN_EXE_realism-gate"))
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    serde_json::from_slice(&out.stdout).unwrap()
}

#[test]
fn json_output_matches_the_published_schema() {
    let text = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/output-schema.json"),
    )
    .unwrap();
    let schema: Value = serde_json::from_str(&text).unwrap();
    // Every fixture set: findings of each rule, ignored allow comments, a
    // declaration that lives outside the repo, and a clean run.
    let decl = std::env::temp_dir().join(format!("rg-schema-decl-{}.yaml", std::process::id()));
    std::fs::copy(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/declaration/realism.yaml"),
        &decl,
    )
    .unwrap();
    let decl = decl.to_str().unwrap().to_string();
    let mut runs: Vec<Vec<&str>> = vec![
        vec!["check", "tests/fixtures/bad", "--no-declaration"],
        vec!["check", "tests/fixtures/suppress", "--no-declaration"],
        vec![
            "check",
            "tests/fixtures/declaration",
            "--declaration",
            &decl,
        ],
        vec!["check", "tests/fixtures/good", "--no-declaration"],
        // The claim table: structured, from the acceptance fixture.
        vec![
            "check",
            "examples/rain-usdr/test",
            "--report",
            "examples/rain-usdr/finding.md",
            "--no-declaration",
            "--no-fail-on-claims",
        ],
    ];
    // Inferred claims: the same report, with no claims.yaml beside it.
    let dir = std::env::temp_dir().join(format!("rg-schema-claims-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::copy(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/rain-usdr/finding.md"),
        dir.join("finding.md"),
    )
    .unwrap();
    let inferred_report = dir.join("finding.md").to_str().unwrap().to_string();
    runs.push(vec![
        "check",
        "examples/rain-usdr/test",
        "--report",
        &inferred_report,
        "--no-declaration",
        "--no-fail-on-claims",
    ]);
    for args in &runs {
        let mut args = args.clone();
        args.extend(["--format", "json", "--fail-on", "never", "--no-config"]);
        let v = report(&args);
        let mut errors = Vec::new();
        validate(&schema, &v, "$", &mut errors);
        assert!(errors.is_empty(), "{args:?}: {errors:#?}");
    }
    std::fs::remove_dir_all(&dir).ok();
}
