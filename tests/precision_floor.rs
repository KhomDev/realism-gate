//! The severity floor (P2R preregistration §5, Amendment 2), for every
//! language: Solidity, Rust, TypeScript, JavaScript and Move.
//!
//! A rule ships above `note` in a language only when a **fresh** measured
//! cell for that rule and language clears the floor:
//!
//! * `error`: Wilson 95% lower bound >= 0.80 with n >= 16;
//! * `warn`: lower bound >= 0.50 with n >= 10;
//! * `note` otherwise.
//!
//! "Fresh" is the explicit `fresh` field of a cell in
//! `crates/rg-rules/data/measured.json`, never a match on the population text.
//! In-sample and DEV cells, and cells without the field, never lift a rule.
//!
//! The shipped table is derived at build time by `crates/rg-rules/build.rs`.
//! This file re-implements the floor from the preregistered numbers, without
//! the crate's own floor code, and checks the build-time table against it, so
//! a derivation that skips the floor (for example one that ships the catalog
//! severity) fails here.

use realism_gate::Severity;
use realism_gate::rules::measured::{self, LANGUAGES};
use realism_gate::rules::{Lang, RULES};
use serde_json::{Value, json};

/// The preregistered floor, written out independently of `floor.rs`.
const Z: f64 = 1.959964;

fn wilson_lower(k: u64, n: u64) -> f64 {
    if n == 0 {
        return 0.0;
    }
    let (k, n) = (k as f64, n as f64);
    let p = k / n;
    let z2 = Z * Z;
    let centre = p + z2 / (2.0 * n);
    let margin = Z * (p * (1.0 - p) / n + z2 / (4.0 * n * n)).sqrt();
    ((centre - margin) / (1.0 + z2 / n)).max(0.0)
}

fn spec_level(cell: Option<&Value>) -> Severity {
    let Some(cell) = cell else {
        return Severity::Note;
    };
    if cell.get("status").and_then(Value::as_str) != Some("measured")
        || cell.get("fresh").and_then(Value::as_bool) != Some(true)
    {
        return Severity::Note;
    }
    let tp = cell.get("tp").and_then(Value::as_u64).unwrap_or(0);
    let fp = cell.get("fp").and_then(Value::as_u64).unwrap_or(0);
    let n = tp + fp;
    let lower = wilson_lower(tp, n);
    if n >= 16 && lower >= 0.80 {
        Severity::Error
    } else if n >= 10 && lower >= 0.50 {
        Severity::Warn
    } else {
        Severity::Note
    }
}

fn committed() -> Value {
    serde_json::from_str(measured::DATA).expect("measured.json")
}

fn cell_of<'a>(doc: &'a Value, rule: &str, lang: &str) -> Option<&'a Value> {
    doc.get("rules")?.get(rule)?.get(lang)
}

fn fresh_cell(tp: u32, fp: u32) -> Value {
    json!({"status": "measured", "labelled": tp + fp, "tp": tp, "fp": fp,
           "unclear": 0, "precision": 0.0, "wilson95": [0.0, 1.0],
           "split": "final", "population": "synthetic", "fresh": true})
}

/// The committed data with one cell replaced, derived by the crate's floor.
fn derive_with(rule: &str, lang: &str, cell: Value) -> measured::Table {
    let mut doc = committed();
    doc["rules"][rule][lang] = cell;
    measured::derive(&doc.to_string()).expect("derives")
}

/// Every (rule, language) the table holds above `note`.
fn lifted(table: &measured::Table) -> Vec<(String, String, Severity)> {
    table
        .iter()
        .filter(|(_, s)| **s != Severity::Note)
        .map(|((r, l), s)| (r.clone(), l.clone(), *s))
        .collect()
}

#[test]
fn the_build_time_table_is_what_the_floor_says_for_the_committed_data() {
    let doc = committed();
    let mut checked = 0;
    for rule in RULES {
        for lang in LANGUAGES {
            let want = spec_level(cell_of(&doc, rule.id, lang));
            assert_eq!(
                measured::derived(rule.id, lang),
                want,
                "{} {lang}: build-time severity is not the floor's",
                rule.id
            );
            checked += 1;
        }
    }
    assert_eq!(checked, RULES.len() * 5);
    // Every row the build script wrote, too, including any cell for a
    // language the catalog does not list for that rule.
    let rows = measured::derived_table();
    let cells: usize = doc["rules"]
        .as_object()
        .unwrap()
        .values()
        .map(|c| c.as_object().unwrap().len())
        .sum();
    assert_eq!(rows.len(), cells, "one derived row per committed cell");
    for (rule, lang, got) in rows {
        assert_eq!(got, spec_level(cell_of(&doc, rule, lang)), "{rule} {lang}");
    }
}

/// Until P2R-2's FINAL cells land, no committed cell is fresh, so every rule
/// ships at `note` in every language.
#[test]
fn every_rule_is_note_in_every_language_until_fresh_cells_land() {
    let doc = committed();
    for (rule, cells) in doc["rules"].as_object().unwrap() {
        for (lang, cell) in cells.as_object().unwrap() {
            assert_eq!(
                cell.get("fresh").and_then(Value::as_bool),
                Some(false),
                "{rule} {lang}: every current cell is marked not fresh"
            );
        }
    }
    for rule in RULES {
        for lang in LANGUAGES {
            assert_eq!(
                measured::derived(rule.id, lang),
                Severity::Note,
                "{} {lang}",
                rule.id
            );
            assert_eq!(
                measured::shipped(rule.id, lang, Severity::Error),
                Severity::Note,
                "{} {lang}: an error-level finding ships as a note",
                rule.id
            );
        }
        assert_eq!(rule.solana_severity(), Severity::Note, "{}", rule.id);
    }
}

/// No rule ships above `note` in any language without a fresh cell for that
/// rule and language that clears the level it ships at.
#[test]
fn no_rule_ships_above_note_without_a_fresh_cell_that_passes_the_floor() {
    let doc = committed();
    for rule in RULES {
        for lang in LANGUAGES {
            let shipped = measured::derived(rule.id, lang);
            if shipped == Severity::Note {
                continue;
            }
            let cell = cell_of(&doc, rule.id, lang)
                .unwrap_or_else(|| panic!("{} {lang}: above note with no cell", rule.id));
            assert_eq!(cell["fresh"], true, "{} {lang}: not fresh", rule.id);
            let (tp, fp) = (cell["tp"].as_u64().unwrap(), cell["fp"].as_u64().unwrap());
            let (min_n, min_lower) = match shipped {
                Severity::Error => (16, 0.80),
                _ => (10, 0.50),
            };
            assert!(
                tp + fp >= min_n && wilson_lower(tp, tp + fp) >= min_lower,
                "{} {lang}: {tp}/{} does not clear {}",
                rule.id,
                tp + fp,
                shipped.as_str()
            );
        }
    }
}

/// A synthetic fresh cell that meets the floor lifts exactly its own rule and
/// language, in each of the five languages.
#[test]
fn a_fresh_cell_meeting_the_floor_lifts_exactly_that_rule_and_language() {
    for lang in LANGUAGES {
        // warn: 10/10 has a lower bound of 0.7225 with n = 10.
        let table = derive_with("RG08", lang, fresh_cell(10, 0));
        assert_eq!(
            lifted(&table),
            vec![("RG08".to_string(), lang.to_string(), Severity::Warn)],
            "{lang}"
        );
        // error: 16/16 has a lower bound of 0.8064 with n = 16.
        let table = derive_with("RG08", lang, fresh_cell(16, 0));
        assert_eq!(
            lifted(&table),
            vec![("RG08".to_string(), lang.to_string(), Severity::Error)],
            "{lang}"
        );
    }
}

#[test]
fn the_floor_boundaries_agree_with_the_preregistered_numbers() {
    let mut in_sample = fresh_cell(30, 0);
    in_sample["fresh"] = json!(false);
    let mut unmarked = fresh_cell(30, 0);
    unmarked.as_object_mut().unwrap().remove("fresh");
    let cases: [(Value, Severity); 9] = [
        (fresh_cell(16, 0), Severity::Error),
        // n = 15: never error, whatever the rate (lower bound 0.796).
        (fresh_cell(15, 0), Severity::Warn),
        // n = 16 but the lower bound is 0.717: warn.
        (fresh_cell(15, 1), Severity::Warn),
        (fresh_cell(10, 0), Severity::Warn),
        // n = 9: never warn.
        (fresh_cell(9, 0), Severity::Note),
        // n = 20, lower bound under 0.50.
        (fresh_cell(12, 8), Severity::Note),
        // The same strong numbers, in-sample: note.
        (in_sample, Severity::Note),
        // A cell written before the `fresh` field existed: not fresh.
        (unmarked, Severity::Note),
        // Marked fresh but unmeasured.
        (
            json!({"status": "unmeasured", "reason": "x", "fresh": true}),
            Severity::Note,
        ),
    ];
    for (cell, want) in cases {
        assert_eq!(spec_level(Some(&cell)), want, "spec: {cell}");
        let table = derive_with("RG02", "solidity", cell.clone());
        assert_eq!(
            table[&("RG02".to_string(), "solidity".to_string())],
            want,
            "crate floor: {cell}"
        );
    }
}

/// The cap is per language: a Rust and a TypeScript finding of one rule ship
/// at different severities when their cells differ, and a `.js` file is
/// keyed `javascript`, not `typescript`.
#[test]
fn rust_typescript_and_javascript_findings_of_one_rule_use_their_own_cells() {
    let mut doc = committed();
    doc["rules"]["RG08"]["rust"] = fresh_cell(10, 0);
    doc["rules"]["RG08"]["typescript"] = fresh_cell(2, 8);
    doc["rules"]["RG08"]["javascript"] = fresh_cell(20, 0);
    let table = measured::derive(&doc.to_string()).unwrap();
    let ship = |lang: Lang, file: &str| {
        measured::shipped_in(
            &table,
            "RG08",
            measured::lang_key(lang, file),
            Severity::Error,
        )
    };
    assert_eq!(ship(Lang::Rust, "tests/poc.rs"), Severity::Warn);
    assert_eq!(ship(Lang::TypeScript, "tests/poc.ts"), Severity::Note);
    assert_eq!(ship(Lang::TypeScript, "tests/poc.js"), Severity::Error);
    assert_eq!(
        measured::lang_key(Lang::TypeScript, "tests/poc.js"),
        "javascript"
    );
    assert_eq!(
        measured::lang_key(Lang::TypeScript, "tests/poc.ts"),
        "typescript"
    );
    // The cap only lowers: a note-level finding stays a note under error.
    assert_eq!(
        measured::shipped_in(&table, "RG08", "javascript", Severity::Note),
        Severity::Note
    );
}

/// End to end: the binary ships every finding at the floor's cap, and each
/// finding's JSON carries its fresh record for its language.
#[test]
fn the_binary_ships_findings_at_the_floor_and_prints_their_record() {
    let root = env!("CARGO_MANIFEST_DIR");
    let mut langs = std::collections::BTreeSet::new();
    for dir in [
        "tests/fixtures/bad",
        "tests/fixtures/rust_shared",
        "tests/fixtures/ts_shared",
        "tests/fixtures/move",
    ] {
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_realism-gate"))
            .args(["check", dir, "--no-declaration", "--format", "json"])
            .current_dir(root)
            .output()
            .expect("binary runs");
        let v: Value = serde_json::from_slice(&out.stdout).expect("json");
        let findings = v["findings"].as_array().unwrap();
        assert!(!findings.is_empty(), "{dir} raises findings");
        for f in findings {
            let rule = f["rule"].as_str().unwrap();
            let record = &f["measured"];
            let lang = record["language"].as_str().unwrap();
            langs.insert(lang.to_string());
            assert_eq!(f["severity"], "note", "{dir} {rule} {lang}");
            assert_eq!(record["status"], "unmeasured", "{dir} {rule} {lang}");
            assert_eq!(record["floor"], "note", "{dir} {rule} {lang}");
        }
        // Nothing ships at error, so the default `--fail-on error` passes.
        assert_eq!(out.status.code(), Some(0), "{dir}");
    }
    for lang in ["solidity", "rust", "typescript", "move"] {
        assert!(langs.contains(lang), "{lang} not exercised: {langs:?}");
    }
}
