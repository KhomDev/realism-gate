//! The model is byte-identical across runs and is valid JSON.

use rg_program::ProgramModel;
use std::path::{Path, PathBuf};

fn fixtures() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut out: Vec<PathBuf> = std::fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap())
        .filter(|entry| entry.file_type().unwrap().is_dir())
        .map(|entry| entry.path())
        .collect();
    out.sort();
    assert!(!out.is_empty());
    out
}

#[test]
fn serialising_the_same_model_twice_is_byte_identical() {
    for fixture in fixtures() {
        let model = ProgramModel::resolve(&fixture).unwrap();
        let first = model.to_json_pretty();
        let second = model.to_json_pretty();
        assert_eq!(
            first,
            second,
            "{}: serialising the same model twice differs",
            fixture.display()
        );
    }
}

#[test]
fn resolving_the_same_target_twice_is_byte_identical() {
    for fixture in fixtures() {
        let first = ProgramModel::resolve(&fixture).unwrap().to_json_pretty();
        let second = ProgramModel::resolve(&fixture).unwrap().to_json_pretty();
        assert_eq!(
            first,
            second,
            "{}: the model is not deterministic",
            fixture.display()
        );
    }
}

#[test]
fn the_serialised_model_is_valid_json_with_the_documented_fields() {
    let model = ProgramModel::resolve(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/anchor-pda"),
    )
    .unwrap();
    let json = model.to_json_pretty();
    let value: serde_json::Value = serde_json::from_str(&json).expect("the model is JSON");
    assert_eq!(value["target"], serde_json::json!("anchor-pda"));
    assert!(value["programs"].is_array());
    let program = &value["programs"][0];
    assert_eq!(program["name"], serde_json::json!("vault"));
    assert_eq!(program["language"], serde_json::json!("anchor"));
    let entry = &program["entry_points"][0];
    assert_eq!(entry["name"], serde_json::json!("initialize"));
    assert_eq!(entry["kind"], serde_json::json!("instruction"));
    assert!(entry["authorities"].is_array());
    assert_eq!(entry["authorities"][0]["kind"], serde_json::json!("signer"));
    assert_eq!(
        entry["authorities"][0]["provenance"],
        serde_json::json!("anchor-struct")
    );
    assert_eq!(
        entry["authorities"][0]["spans"][0]["file"],
        serde_json::json!("src/lib.rs")
    );
    assert!(entry["pdas"].is_array());
    assert!(entry["unknown_reasons"].is_array());
}
