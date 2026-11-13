//! The JSON output matches docs/output-schema.json.
//!
//! The validator below covers the subset of JSON Schema the file uses (type,
//! enum, required, properties, additionalProperties, items, oneOf), so the
//! check needs no extra dependency.

use serde_json::Value;
use std::process::Command;

fn validate(schema: &Value, v: &Value, path: &str, errors: &mut Vec<String>) {
    if let Some(alts) = schema.get("oneOf").and_then(Value::as_array) {
        let ok = alts.iter().filter(|alt| {
            let mut e = Vec::new();
            validate(alt, v, path, &mut e);
            e.is_empty()
        });
        if ok.count() != 1 {
            errors.push(format!("{path}: matches no oneOf branch"));
        }
        return;
    }
    if let Some(t) = schema.get("type") {
        let types: Vec<&str> = match t {
            Value::String(s) => vec![s.as_str()],
            Value::Array(a) => a.iter().filter_map(Value::as_str).collect(),
            _ => vec![],
        };
        let fits = types.iter().any(|t| match *t {
            "object" => v.is_object(),
            "array" => v.is_array(),
            "string" => v.is_string(),
            "integer" => v.is_u64() || v.is_i64(),
            "boolean" => v.is_boolean(),
            "null" => v.is_null(),
            _ => false,
        });
        if !fits {
            errors.push(format!("{path}: expected {types:?}, got {v}"));
            return;
        }
    }
    if let Some(allowed) = schema.get("enum").and_then(Value::as_array)
        && !allowed.contains(v)
    {
        errors.push(format!("{path}: {v} is not one of {allowed:?}"));
    }
    if let Some(obj) = v.as_object() {
        let props = schema.get("properties").and_then(Value::as_object);
        for key in schema
            .get("required")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            if !obj.contains_key(key) {
                errors.push(format!("{path}: missing `{key}`"));
            }
        }
        for (k, child) in obj {
            match props.and_then(|p| p.get(k)) {
                Some(s) => validate(s, child, &format!("{path}.{k}"), errors),
                None if schema.get("additionalProperties") == Some(&Value::Bool(false)) => {
                    errors.push(format!("{path}: undeclared field `{k}`"))
                }
                None => {}
            }
        }
    }
    if let (Some(items), Some(arr)) = (schema.get("items"), v.as_array()) {
        for (i, child) in arr.iter().enumerate() {
            validate(items, child, &format!("{path}[{i}]"), errors);
        }
    }
}

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
