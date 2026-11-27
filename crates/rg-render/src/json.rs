//! JSON output, schema version 1. Fields are only ever added within a schema
//! version; a removal or a change of meaning bumps `schema`.
//!
//! Each finding carries one added field, `measured`: its rule's fresh record
//! for the finding's language (`rules::measured::Record`). Everything else is
//! the `report/v1` shape of `engine::Report`, field for field and in order.

use super::{TOOL, VERSION};
use crate::engine::{ChangedSince, ParseError, Report, Summary};
use crate::finding::Finding;
use crate::rules::measured::{self, Record};
use serde::Serialize;

pub const SCHEMA: u32 = 1;

#[derive(Serialize)]
struct Document<'a> {
    tool: &'static str,
    version: &'static str,
    schema: u32,
    #[serde(flatten)]
    report: Body<'a>,
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// `engine::Report` with the measured record on each finding. The field list
/// mirrors `Report`; `a_finding_gains_only_its_measured_record` fails if the
/// two drift apart.
#[derive(Serialize)]
struct Body<'a> {
    summary: &'a Summary,
    declaration: &'a Option<String>,
    config: &'a Option<String>,
    #[serde(skip_serializing_if = "is_false")]
    fragment: bool,
    changed_since: &'a Option<ChangedSince>,
    findings: Vec<Measured<'a>>,
    parse_errors: &'a [ParseError],
    #[serde(skip_serializing_if = "<[String]>::is_empty")]
    ignored_policy: &'a [String],
    #[serde(skip_serializing_if = "Option::is_none")]
    claims: Option<&'a crate::claims::Table>,
    #[serde(skip_serializing_if = "Option::is_none")]
    claim_binding: Option<&'a crate::claims::Binding>,
}

#[derive(Serialize)]
struct Measured<'a> {
    #[serde(flatten)]
    finding: &'a Finding,
    measured: Record,
}

/// The fresh record a finding prints: its rule, in its file's language.
pub fn record(f: &Finding) -> Record {
    measured::record(f.rule, measured::lang_of_path(&f.file))
}

fn body(report: &Report) -> Body<'_> {
    Body {
        summary: &report.summary,
        declaration: &report.declaration,
        config: &report.config,
        fragment: report.fragment,
        changed_since: &report.changed_since,
        findings: report
            .findings
            .iter()
            .map(|f| Measured {
                finding: f,
                measured: record(f),
            })
            .collect(),
        parse_errors: &report.parse_errors,
        ignored_policy: &report.ignored_policy,
        claims: report.claims.as_ref(),
        claim_binding: report.claim_binding.as_ref(),
    }
}

pub fn render(report: &Report) -> String {
    let doc = Document {
        tool: TOOL,
        version: VERSION,
        schema: SCHEMA,
        report: body(report),
    };
    let mut out = serde_json::to_string_pretty(&doc).expect("report serializes");
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::finding::{Severity, Suppression};

    fn report() -> Report {
        Report {
            summary: Summary::default(),
            declaration: Some("realism.yaml".into()),
            config: None,
            fragment: true,
            changed_since: Some(ChangedSince {
                git_ref: "main".into(),
                files: 1,
            }),
            findings: vec![Finding {
                rule: "RG08",
                slug: "no-assertion",
                severity: Severity::Note,
                file: "tests/poc.js".into(),
                line: 3,
                column: 1,
                suite: "s".into(),
                tests: vec!["t".into()],
                message: "m".into(),
                evidence: "e".into(),
                help: "h",
                suppressed: Some(Suppression {
                    line: 2,
                    reason: "r".into(),
                }),
                ignored_suppression: None,
                ignored_reason: None,
            }],
            parse_errors: vec![ParseError {
                file: "x.sol".into(),
                message: "bad".into(),
            }],
            ignored_policy: vec!["realism-gate.toml".into()],
            claims: None,
            claim_binding: None,
        }
    }

    /// `report/v1` is additive: removing `measured` from every finding gives
    /// exactly what serializing `Report` itself gives.
    #[test]
    fn a_finding_gains_only_its_measured_record() {
        let r = report();
        let mut ours: serde_json::Value = serde_json::from_str(&render(&r)).unwrap();
        let record = ours["findings"][0]
            .as_object_mut()
            .unwrap()
            .remove("measured")
            .expect("each finding has a measured record");
        assert_eq!(record["language"], "javascript");
        assert_eq!(record["status"], "unmeasured");
        assert_eq!(record["floor"], "note");
        let obj = ours.as_object_mut().unwrap();
        for key in ["tool", "version", "schema"] {
            obj.remove(key);
        }
        let plain = serde_json::to_value(&r).unwrap();
        assert_eq!(ours, plain);
    }
}
