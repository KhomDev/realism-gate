//! The severity floor (docs/precision.md): a rule ships at `error` on Solana
//! only when its **fresh** held-out Wilson lower bound is at least 0.80. The
//! test reads the committed measurements; cells from the P1 population (the
//! same PoCs the fixes were designed from) are in-sample and cannot lift a
//! rule over the floor. A Solana rule that ships at `error` with no fresh
//! measured cell is a violation too: an unmeasured error rule is a claim.
//!
//! Fresh means the sealed v2 corpus (`eval/solana-poc-v2/measured.json`),
//! whose population string carries the marker below. `measure.py` in that
//! directory regenerates the file from the committed held-out labels.

use realism_gate::Severity;
use realism_gate::rules::{RULES, RuleInfo};
use serde_json::Value;
use std::path::Path;

/// The floor written into docs/precision.md.
const FLOOR: f64 = 0.80;
/// A cell counts only when its population is the sealed v2 corpus (or a later
/// fresh corpus whose population string contains this marker).
const FRESH_MARKER: &str = "eval/solana-poc-v2";
/// The rule ids `src/solana/rules.rs` emits. A rule outside this list never
/// fires on Solana, so its catalog severity is a Solidity severity and the
/// Solana floor does not apply.
const SOLANA_RULES: [&str; 10] = [
    "RG01", "RG02", "RG03", "RG04", "RG05", "RG06", "RG08", "RG09", "RG10", "RG11",
];

fn is_solana_rule(id: &str) -> bool {
    SOLANA_RULES.contains(&id)
}

fn violations_with(doc: &Value, severity: impl Fn(&RuleInfo) -> Severity) -> Vec<String> {
    let mut out = Vec::new();
    for rule in RULES {
        if !is_solana_rule(rule.id) || severity(rule) != Severity::Error {
            continue;
        }
        let cells = doc
            .get("rules")
            .and_then(|m| m.get(rule.id))
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let mut measured_fresh = 0;
        for (lang, cell) in &cells {
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
            measured_fresh += 1;
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
        if measured_fresh == 0 {
            out.push(format!(
                "{} ships at error with no fresh held-out cell of n>=5",
                rule.id
            ));
        }
    }
    out
}

fn violations(doc: &Value) -> Vec<String> {
    violations_with(doc, |rule| rule.solana_severity())
}

fn load(path: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).expect("measurement file")).unwrap()
}

/// RG08 as an `error`, so the floor's branches can be tested even though the
/// shipped Solana severity is `warn`.
fn rg08_at_error(rule: &RuleInfo) -> Severity {
    if rule.id == "RG08" {
        Severity::Error
    } else {
        rule.solana_severity()
    }
}

#[test]
fn a_low_fresh_cell_breaks_the_floor() {
    let doc: Value = serde_json::from_str(
        r#"{"rules":{"RG08":{"rust":{"status":"measured","labelled":30,
             "wilson95":[0.5,0.9],
             "population":"public Solana PoCs (held-out split; eval/solana-poc-v2)"}}}}"#,
    )
    .unwrap();
    let got = violations_with(&doc, rg08_at_error);
    assert_eq!(got, vec!["RG08 rust lower bound 0.500 < 0.80".to_string()]);
}

#[test]
fn an_in_sample_cell_cannot_lift_a_rule_over_the_floor() {
    let doc: Value = serde_json::from_str(
        r#"{"rules":{"RG08":{"rust":{"status":"measured","labelled":30,
             "wilson95":[0.5,0.9],
             "population":"public Solana PoCs (held-out split; eval/solana-poc)"}}}}"#,
    )
    .unwrap();
    let got = violations_with(&doc, rg08_at_error);
    assert_eq!(
        got,
        vec!["RG08 ships at error with no fresh held-out cell of n>=5".to_string()]
    );
}

#[test]
fn an_error_rule_with_no_fresh_cell_breaks_the_floor() {
    let doc: Value = serde_json::from_str(r#"{"rules":{}}"#).unwrap();
    let got = violations_with(&doc, |_| Severity::Error);
    assert!(
        got.iter()
            .any(|v| v == "RG08 ships at error with no fresh held-out cell of n>=5"),
        "{got:?}"
    );
}

#[test]
fn no_solana_rule_ships_at_error_without_clearing_the_floor() {
    let base = Path::new(env!("CARGO_MANIFEST_DIR"));
    let in_sample = load(&base.join("eval/solana/measured.json"));
    let fresh = load(&base.join("eval/solana-poc-v2/measured.json"));
    assert!(
        violations(&in_sample).is_empty(),
        "{:?}",
        violations(&in_sample)
    );
    assert!(violations(&fresh).is_empty(), "{:?}", violations(&fresh));
}

/// The fresh file must actually contain the held-out cells the severity
/// decision cites; a missing or empty file would make the floor vacuous.
#[test]
fn the_fresh_measurement_is_present_and_measured() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("eval/solana-poc-v2/measured.json");
    let doc = load(&path);
    let population = doc.get("population").and_then(Value::as_str).unwrap_or("");
    assert!(
        population.contains(FRESH_MARKER),
        "wrong population: {population}"
    );
    let mut measured = 0;
    for id in ["RG02", "RG03", "RG08"] {
        let cells = doc["rules"][id].as_object().expect("rule cells");
        measured += cells
            .values()
            .filter(|c| c.get("status").and_then(Value::as_str) == Some("measured"))
            .count();
    }
    assert!(
        measured >= 4,
        "expected the held-out cells, found {measured}"
    );
}
