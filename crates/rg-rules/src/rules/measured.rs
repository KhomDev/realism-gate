//! Measured precision, and the severity each rule ships at, from the committed
//! `data/measured.json`.
//!
//! The data file holds one cell per rule and language. A cell lifts its rule's
//! severity only when it is measured **and** marked `"fresh": true`: labels
//! from a sealed slice opened once by the frozen binary under test. An
//! in-sample or DEV cell, or a cell without the `fresh` field, is kept as
//! history and printed by `rules` and `explain`, but it never lifts a rule.
//!
//! The shipped severity is derived at **build time**: `build.rs` applies the
//! floor in [`floor`] to every cell and writes the table this module includes.
//! A finding ships at the lower of the severity its rule's code raised and the
//! level the floor allows for the finding's language. Replacing the cells is a
//! data change; the next build re-derives the table.
//!
//! Each finding also carries its rule's **fresh** record for its language
//! ([`record`]): fresh k/n with a Wilson 95% interval, a count only when
//! n < 5, or `unmeasured`.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

use crate::finding::Severity;
use crate::rules::{Lang, RuleInfo};

#[path = "measured/floor.rs"]
pub mod floor;

/// The committed data file, embedded so a packaged `rg-rules` builds without
/// the evaluation tree. The evidence card hashes these bytes.
pub const DATA: &str = include_str!("../../data/measured.json");

mod derived {
    include!(concat!(env!("OUT_DIR"), "/derived_severity.rs"));
}

/// Every language a cell can be keyed by, in output order.
pub const LANGUAGES: [&str; 5] = ["solidity", "rust", "typescript", "javascript", "move"];

#[derive(Debug, Deserialize, Serialize)]
pub struct Doc {
    pub rules: BTreeMap<String, BTreeMap<String, Cell>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Cell {
    pub status: String,
    #[serde(default)]
    pub labelled: u32,
    #[serde(default)]
    pub tp: u32,
    #[serde(default)]
    pub fp: u32,
    #[serde(default)]
    pub unclear: u32,
    #[serde(default)]
    pub precision: Option<f64>,
    #[serde(default)]
    pub wilson95: Option<[f64; 2]>,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub split: Option<String>,
    #[serde(default)]
    pub population: Option<String>,
    /// Fewer than five labels: print the count, never a percentage.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub count_only: bool,
    /// Out-of-sample for the binary under test. Absent means not fresh, so a
    /// data file written before this field existed lifts nothing.
    #[serde(default)]
    pub fresh: bool,
    /// Why the cell is or is not fresh.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fresh_reason: Option<String>,
}

impl Cell {
    /// Measured and fresh: the only kind of cell the floor reads.
    pub fn is_fresh_measurement(&self) -> bool {
        self.status == "measured" && self.fresh
    }
}

static DOC: LazyLock<Doc> =
    LazyLock::new(|| serde_json::from_str(DATA).expect("data/measured.json is valid"));

pub fn doc() -> &'static Doc {
    &DOC
}

pub fn cell(rule: &str, lang: &str) -> Option<&'static Cell> {
    DOC.rules.get(rule).and_then(|m| m.get(lang))
}

fn severity_of(level: floor::Level) -> Severity {
    match level {
        floor::ERROR => Severity::Error,
        floor::WARN => Severity::Warn,
        _ => Severity::Note,
    }
}

/// The severity the floor allows for `rule` in `lang`, from the table
/// `build.rs` derived. A rule or language with no cell derives `note`.
pub fn derived(rule: &str, lang: &str) -> Severity {
    derived::DERIVED
        .iter()
        .find(|(r, l, _)| *r == rule && *l == lang)
        .map_or(Severity::Note, |(_, _, level)| severity_of(*level))
}

/// Every row of the build-time table: (rule, language, severity).
pub fn derived_table() -> Vec<(&'static str, &'static str, Severity)> {
    derived::DERIVED
        .iter()
        .map(|(r, l, level)| (*r, *l, severity_of(*level)))
        .collect()
}

/// A severity table derived from a data file at run time by the same floor,
/// for tests that need cells other than the committed ones.
pub type Table = BTreeMap<(String, String), Severity>;

/// Derives a [`Table`] from the text of a measurement file.
pub fn derive(json: &str) -> Result<Table, String> {
    let doc: Doc = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let mut table = Table::new();
    for (rule, cells) in doc.rules {
        for (lang, c) in cells {
            let level = floor::level(c.status == "measured", c.fresh, c.tp, c.fp);
            table.insert((rule.clone(), lang), severity_of(level));
        }
    }
    Ok(table)
}

/// The shipped severity: the lower of what the rule raised and the cap.
pub fn capped(emitted: Severity, cap: Severity) -> Severity {
    emitted.min(cap)
}

/// The severity a finding of `rule` in `lang` ships at, by the build-time
/// table.
pub fn shipped(rule: &str, lang: &str, emitted: Severity) -> Severity {
    capped(emitted, derived(rule, lang))
}

/// [`shipped`] against a run-time [`Table`].
pub fn shipped_in(table: &Table, rule: &str, lang: &str, emitted: Severity) -> Severity {
    let cap = table
        .get(&(rule.to_string(), lang.to_string()))
        .copied()
        .unwrap_or(Severity::Note);
    capped(emitted, cap)
}

fn extension(file: &str) -> &str {
    let name = file.rsplit(['/', '\\']).next().unwrap_or(file);
    name.rsplit_once('.').map_or("", |(_, ext)| ext)
}

fn is_javascript(file: &str) -> bool {
    matches!(
        extension(file).to_ascii_lowercase().as_str(),
        "js" | "jsx" | "mjs" | "cjs"
    )
}

/// The measurement key of a finding the engine raised in `lang` on `file`.
/// The engine reads JavaScript and TypeScript into one model, so a
/// TypeScript-model finding on a `.js`/`.jsx`/`.mjs`/`.cjs` file is keyed
/// `javascript`.
pub fn lang_key(lang: Lang, file: &str) -> &'static str {
    match lang {
        Lang::Solidity => "solidity",
        Lang::Rust => "rust",
        Lang::Move => "move",
        Lang::TypeScript if is_javascript(file) => "javascript",
        Lang::TypeScript => "typescript",
    }
}

/// The measurement key of a finding from its file alone, for renderers that
/// see only the report. It agrees with [`lang_key`] for every file the
/// engine reads by extension; a finding on the declaration or on any other
/// file is a Solidity-model finding, as the engine files it.
pub fn lang_of_path(file: &str) -> &'static str {
    match extension(file).to_ascii_lowercase().as_str() {
        "rs" => "rust",
        "ts" | "tsx" | "mts" | "cts" => "typescript",
        "js" | "jsx" | "mjs" | "cjs" => "javascript",
        "move" => "move",
        _ => "solidity",
    }
}

/// A rule's fresh record for one language, as a finding prints it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Record {
    pub language: &'static str,
    /// `fresh` when a fresh measured cell exists, else `unmeasured`.
    pub status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tp: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n: Option<u32>,
    /// Wilson 95% interval, four decimals; absent for a count-only cell.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wilson95: Option<[f64; 2]>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub count_only: bool,
    /// The severity the floor allows this rule in this language.
    pub floor: Severity,
    /// On a finding: the level the rule's code raised, before the floor and
    /// any `--severity` override. Shown next to `shipped` so a lowered level
    /// stays visible.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raised: Option<Severity>,
    /// On a finding: the level it ships at.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shipped: Option<Severity>,
}

fn round4(x: f64) -> f64 {
    (x * 10_000.0).round() / 10_000.0
}

/// The record a cell gives, with `floor` the allowed severity.
pub fn record_of(lang: &'static str, c: Option<&Cell>, floor: Severity) -> Record {
    match c {
        Some(c) if c.is_fresh_measurement() => {
            let n = c.tp + c.fp;
            let count_only = n < floor::COUNT_ONLY_BELOW;
            let (lo, hi) = floor::wilson(c.tp, n);
            Record {
                language: lang,
                status: "fresh",
                tp: Some(c.tp),
                n: Some(n),
                wilson95: (!count_only).then_some([round4(lo), round4(hi)]),
                count_only,
                floor,
                raised: None,
                shipped: None,
            }
        }
        _ => Record {
            language: lang,
            status: "unmeasured",
            tp: None,
            n: None,
            wilson95: None,
            count_only: false,
            floor,
            raised: None,
            shipped: None,
        },
    }
}

/// The committed fresh record of `rule` in `lang`.
pub fn record(rule: &str, lang: &'static str) -> Record {
    record_of(lang, cell(rule, lang), derived(rule, lang))
}

impl Record {
    /// One line: `fresh 14/16 [64.0-96.5]`, `fresh 3/4 (n<5, count only)` or
    /// `not yet measured on fresh data`, then the floor, then (on a finding)
    /// the raised and shipped levels.
    pub fn text(&self) -> String {
        let body = match (self.status, self.tp, self.n, self.wilson95) {
            ("fresh", Some(tp), Some(n), Some([lo, hi])) => {
                format!("fresh {tp}/{n} [{:.1}-{:.1}]", lo * 100.0, hi * 100.0)
            }
            ("fresh", Some(tp), Some(n), None) => format!("fresh {tp}/{n} (n<5, count only)"),
            _ => "not yet measured on fresh data".to_string(),
        };
        let mut line = format!(
            "{} {body}; floor allows {}",
            self.language,
            self.floor.as_str()
        );
        if let (Some(raised), Some(shipped)) = (self.raised, self.shipped) {
            line.push_str(&format!(
                "; raised {}, shipped {}",
                raised.as_str(),
                shipped.as_str()
            ));
        }
        line
    }
}

/// The languages a rule applies to, in output order.
pub fn languages(r: &RuleInfo) -> Vec<&'static str> {
    if matches!(r.solana, super::SolanaNotes::Declaration) {
        return LANGUAGES.to_vec();
    }
    let mut langs = vec!["solidity"];
    if r.checks_solana() {
        langs.extend(["rust", "typescript", "javascript"]);
    }
    if r.checks_move() {
        langs.push("move");
    }
    langs
}

/// A compact one-line cell, used by `rules` and `explain`.
pub fn format_cell(c: &Cell, with_reason: bool) -> String {
    if c.status != "measured" {
        if with_reason {
            return format!(
                "unmeasured ({})",
                c.reason.as_deref().unwrap_or("no measurement")
            );
        }
        return "unmeasured".to_string();
    }
    let freshness = if c.fresh { "fresh" } else { "in-sample" };
    let n = c.tp + c.fp;
    if c.count_only {
        let split = c
            .split
            .as_deref()
            .map(|s| format!(", {s}"))
            .unwrap_or_default();
        return format!("{}/{n} (n<5, count only{split}, {freshness})", c.tp);
    }
    let precision = c.precision.unwrap_or(0.0) * 100.0;
    let interval = c
        .wilson95
        .map(|[lo, hi]| format!("[{:.1}-{:.1}]", lo * 100.0, hi * 100.0))
        .unwrap_or_else(|| "[-]".to_string());
    let unclear = if c.unclear > 0 {
        format!(", {} unclear", c.unclear)
    } else {
        String::new()
    };
    let split = c
        .split
        .as_deref()
        .map(|s| format!(" ({s}, {freshness})"))
        .unwrap_or_else(|| format!(" ({freshness})"));
    format!(
        "{}/{} = {precision:.1}% {interval}{unclear}{split}",
        c.tp, n
    )
}

/// One line listing every applicable language.
pub fn line(r: &RuleInfo, with_reason: bool) -> String {
    languages(r)
        .into_iter()
        .map(|lang| match cell(r.id, lang) {
            Some(c) => format!("{lang}: {}", format_cell(c, with_reason)),
            None => format!("{lang}: unmeasured (no measurement)"),
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// The shipped severity per applicable language, in one line:
/// `solidity note · rust note · …`.
pub fn shipped_line(r: &RuleInfo) -> String {
    languages(r)
        .into_iter()
        .map(|lang| {
            format!(
                "{lang} {}",
                capped(r.severity, derived(r.id, lang)).as_str()
            )
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The crate embeds its own copy so a packaged `rg-rules` builds without
    /// the evaluation tree. Every number in it must be the committed
    /// evaluation table's; only the additive freshness fields may differ.
    #[test]
    fn the_embedded_numbers_are_the_committed_eval_table() {
        let strip = |text: &str| -> serde_json::Value {
            let mut v: serde_json::Value = serde_json::from_str(text).unwrap();
            v.as_object_mut().unwrap().remove("fresh_policy");
            for cells in v["rules"].as_object_mut().unwrap().values_mut() {
                for c in cells.as_object_mut().unwrap().values_mut() {
                    let c = c.as_object_mut().unwrap();
                    c.remove("fresh");
                    c.remove("fresh_reason");
                }
            }
            v
        };
        assert_eq!(
            strip(DATA),
            strip(include_str!("../../../../eval/solana/measured.json")),
            "crates/rg-rules/data/measured.json numbers differ from eval/solana/measured.json"
        );
    }

    #[test]
    fn every_rule_is_present_and_internally_consistent() {
        for i in 1..=20 {
            let id = format!("RG{i:02}");
            let cells = DOC.rules.get(&id).unwrap_or_else(|| panic!("{id} missing"));
            for (lang, cell) in cells {
                assert!(
                    LANGUAGES.contains(&lang.as_str()),
                    "{id}: bad language {lang}"
                );
                assert!(
                    ["measured", "unmeasured"].contains(&cell.status.as_str()),
                    "{id} {lang}: bad status {}",
                    cell.status
                );
                if cell.status == "measured" {
                    assert!(cell.tp + cell.fp > 0, "{id} {lang}: measured with n=0");
                    assert_eq!(
                        cell.count_only,
                        cell.tp + cell.fp < floor::COUNT_ONLY_BELOW,
                        "{id} {lang}: count_only must mark exactly the n<5 cells"
                    );
                    let [lo, hi] = cell
                        .wilson95
                        .unwrap_or_else(|| panic!("{id} {lang}: no interval"));
                    let (want_lo, want_hi) = floor::wilson(cell.tp, cell.tp + cell.fp);
                    assert!(
                        (lo - want_lo).abs() < 0.001 && (hi - want_hi).abs() < 0.001,
                        "{id} {lang}: stored interval [{lo}, {hi}] is not Wilson [{want_lo}, {want_hi}]"
                    );
                    assert!(cell.precision.is_some(), "{id} {lang}: no precision");
                    assert!(
                        cell.fresh_reason.is_some(),
                        "{id} {lang}: a measured cell says why it is or is not fresh"
                    );
                } else {
                    assert!(
                        cell.reason.is_some(),
                        "{id} {lang}: unmeasured without a reason"
                    );
                }
            }
        }
    }

    #[test]
    fn every_rule_explains_itself_in_every_applicable_language() {
        for r in crate::rules::RULES {
            for lang in languages(r) {
                let text = match cell(r.id, lang) {
                    Some(c) => format_cell(c, true),
                    None => "unmeasured (no measurement)".to_string(),
                };
                assert!(!text.is_empty(), "{} {lang}", r.id);
            }
        }
    }

    #[test]
    fn solana_cells_come_from_the_poc_study() {
        // The embedded numbers must be the PoC held-out split, cell for cell.
        let poc: serde_json::Value =
            serde_json::from_str(include_str!("../../../../eval/solana-poc/results.json"))
                .expect("eval/solana-poc/results.json");
        let heldout = &poc["rq2_precision"]["heldout"];
        let mut measured = 0;
        for (rule, langs) in heldout.as_object().expect("heldout object") {
            for (lang, want) in langs.as_object().expect("language object") {
                let got = cell(rule, lang).unwrap_or_else(|| panic!("{rule} {lang} missing"));
                if want["status"] == "measured" {
                    measured += 1;
                    assert_eq!(got.status, "measured", "{rule} {lang}");
                    assert_eq!(
                        u64::from(got.tp),
                        want["tp"].as_u64().unwrap(),
                        "{rule} {lang} tp"
                    );
                    assert_eq!(
                        u64::from(got.fp),
                        want["fp"].as_u64().unwrap(),
                        "{rule} {lang} fp"
                    );
                    assert!(
                        got.population.as_deref().unwrap_or("").contains("PoC"),
                        "{rule} {lang}: population must name PoCs"
                    );
                    // The P1 PoC set is in-sample for every later binary.
                    assert!(!got.fresh, "{rule} {lang}: the P1 set cannot be fresh");
                } else {
                    assert_eq!(got.status, "unmeasured", "{rule} {lang}");
                }
            }
        }
        assert!(measured > 0, "no measured PoC cell");
    }

    fn sample(tp: u32, fp: u32, fresh: bool) -> Cell {
        let n = tp + fp;
        let (lo, hi) = floor::wilson(tp, n);
        Cell {
            status: "measured".into(),
            labelled: n,
            tp,
            fp,
            unclear: 0,
            precision: Some(f64::from(tp) / f64::from(n.max(1))),
            wilson95: Some([lo, hi]),
            reason: None,
            split: Some("heldout".into()),
            population: None,
            count_only: n < 5,
            fresh,
            fresh_reason: None,
        }
    }

    #[test]
    fn count_only_cells_print_no_percentage() {
        let text = format_cell(&sample(0, 2, false), true);
        assert_eq!(text, "0/2 (n<5, count only, heldout, in-sample)");
        assert!(!text.contains('%'));
        let r = record_of("rust", Some(&sample(3, 1, true)), Severity::Note);
        assert_eq!(
            r.text(),
            "rust fresh 3/4 (n<5, count only); floor allows note"
        );
        assert!(r.wilson95.is_none());
    }

    #[test]
    fn a_record_prints_only_a_fresh_cell() {
        let in_sample = record_of("solidity", Some(&sample(30, 0, false)), Severity::Note);
        assert_eq!(in_sample.status, "unmeasured");
        assert_eq!(
            in_sample.text(),
            "solidity not yet measured on fresh data; floor allows note"
        );
        let on_finding = Record {
            raised: Some(Severity::Warn),
            shipped: Some(Severity::Note),
            ..record_of("solidity", None, Severity::Note)
        };
        assert_eq!(
            on_finding.text(),
            "solidity not yet measured on fresh data; floor allows note; raised warn, shipped note"
        );
        let fresh = record_of("solidity", Some(&sample(16, 0, true)), Severity::Error);
        assert_eq!(fresh.status, "fresh");
        assert_eq!(fresh.n, Some(16));
        assert_eq!(fresh.wilson95, Some([0.8064, 1.0]));
        assert_eq!(
            fresh.text(),
            "solidity fresh 16/16 [80.6-100.0]; floor allows error"
        );
        assert_eq!(record_of("move", None, Severity::Note).status, "unmeasured");
    }

    #[test]
    fn the_floor_boundaries() {
        // error needs n >= 16 and a lower bound >= 0.80: 16/16 is 0.8064.
        assert_eq!(floor::level(true, true, 16, 0), floor::ERROR);
        // 15/15 has a lower bound of 0.796 and n < 16: warn.
        assert_eq!(floor::level(true, true, 15, 0), floor::WARN);
        // warn needs n >= 10: 10/10 (0.7225) is warn, 9/9 is note.
        assert_eq!(floor::level(true, true, 10, 0), floor::WARN);
        assert_eq!(floor::level(true, true, 9, 0), floor::NOTE);
        // A lower bound under 0.50 is note at any n.
        assert_eq!(floor::level(true, true, 12, 8), floor::NOTE);
        // In-sample or unmeasured never lifts.
        assert_eq!(floor::level(true, false, 100, 0), floor::NOTE);
        assert_eq!(floor::level(false, true, 100, 0), floor::NOTE);
    }

    #[test]
    fn the_shipped_severity_is_never_above_the_emitted_one() {
        for cap in [Severity::Note, Severity::Warn, Severity::Error] {
            assert_eq!(capped(Severity::Note, cap), Severity::Note);
        }
        assert_eq!(capped(Severity::Error, Severity::Warn), Severity::Warn);
        assert_eq!(capped(Severity::Warn, Severity::Error), Severity::Warn);
    }

    #[test]
    fn language_keys() {
        assert_eq!(lang_key(Lang::TypeScript, "tests/poc.js"), "javascript");
        assert_eq!(lang_key(Lang::TypeScript, "tests/poc.MJS"), "javascript");
        assert_eq!(lang_key(Lang::TypeScript, "tests/poc.ts"), "typescript");
        assert_eq!(lang_key(Lang::Rust, "tests/poc.rs"), "rust");
        assert_eq!(lang_key(Lang::Solidity, "realism.yaml"), "solidity");
        assert_eq!(lang_key(Lang::Move, "sources/a.move"), "move");
        for (file, lang) in [
            ("test/A.t.sol", Lang::Solidity),
            ("tests/poc.rs", Lang::Rust),
            ("tests/poc.ts", Lang::TypeScript),
            ("tests/poc.cjs", Lang::TypeScript),
            ("sources/a.move", Lang::Move),
            ("realism.yaml", Lang::Solidity),
        ] {
            assert_eq!(lang_of_path(file), lang_key(lang, file), "{file}");
        }
    }
}
