//! The severity floor (docs/precision.md): a rule ships at `error` on Solana
//! only when its **fresh** held-out Wilson lower bound is at least 0.80. The
//! test reads the committed measurement; cells from the P1 population (the
//! same PoCs the fixes were designed from) are in-sample and cannot lift a
//! rule over the floor.

use realism_gate::Severity;
use realism_gate::rules::RULES;
use serde_json::Value;
use std::path::Path;

/// The floor written into docs/precision.md.
const FLOOR: f64 = 0.80;
/// A cell counts only when its population is the sealed v2 corpus (or a later
/// fresh corpus whose population string contains this marker).
const FRESH_MARKER: &str = "eval/solana-poc-v2";

fn violations(doc: &Value) -> Vec<String> {
    let mut out = Vec::new();
    for rule in RULES {
        if rule.solana_severity() != Severity::Error {
            continue;
        }
        let Some(cells) = doc.get("rules").and_then(|m| m.get(rule.id)) else {
            continue;
        };
        let Some(cells) = cells.as_object() else {
            continue;
        };
        for (lang, cell) in cells {
            if cell.get("status").and_then(Value::as_str) != Some("measured") {
                continue;
            }
            // The minimum-n rule: fewer than five labels is a count.
            let n = cell.get("labelled").and_then(Value::as_u64).unwrap_or(0);
            if n < 5 {
                continue;
            }
            let population = cell.get("population").and_then(Value::as_str).unwrap_or("");
            if !population.contains(FRESH_MARKER) {
                continue;
            }
            if let Some(lower) = cell
                .get("wilson95")
                .and_then(|w| w.get(0))
                .and_then(Value::as_f64)
                && lower < FLOOR
            {
                out.push(format!(
                    "{} {} lower bound {lower:.3} < {FLOOR:.2}",
                    rule.id, lang
                ));
            }
        }
    }
    out
}

#[test]
fn a_low_fresh_cell_breaks_the_floor() {
    let doc: Value = serde_json::from_str(
        r#"{"rules":{"RG08":{"rust":{"status":"measured","labelled":30,
             "wilson95":[0.5,0.9],
             "population":"public Solana PoCs (held-out split; eval/solana-poc-v2)"}}}}"#,
    )
    .unwrap();
    assert_eq!(
        violations(&doc),
        vec!["RG08 rust lower bound 0.500 < 0.80".to_string()]
    );
}

#[test]
fn an_in_sample_cell_cannot_lift_a_rule_over_the_floor() {
    let doc: Value = serde_json::from_str(
        r#"{"rules":{"RG08":{"rust":{"status":"measured","labelled":30,
             "wilson95":[0.5,0.9],
             "population":"public Solana PoCs (held-out split; eval/solana-poc)"}}}}"#,
    )
    .unwrap();
    assert!(violations(&doc).is_empty());
}

#[test]
fn the_committed_measurement_meets_the_floor() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("eval/solana/measured.json");
    let doc: Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("measured.json")).unwrap();
    let violations = violations(&doc);
    assert!(violations.is_empty(), "{violations:?}");
}
