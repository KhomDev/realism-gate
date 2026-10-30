//! SARIF 2.1.0, for GitHub code scanning and any other SARIF consumer.

use super::{REPOSITORY, TOOL, VERSION, rule_url};
use crate::engine::Report;
use crate::rules::RULES;
use serde_json::{Value, json};

pub fn render(report: &Report) -> String {
    let rules: Vec<Value> = RULES
        .iter()
        .map(|r| {
            json!({
                "id": r.id,
                "name": r.slug,
                "shortDescription": { "text": r.title },
                "fullDescription": { "text": r.summary },
                "help": { "text": r.fix },
                "helpUri": rule_url(r.id),
                "defaultConfiguration": { "level": r.severity.sarif_level() },
            })
        })
        .collect();
    let results: Vec<Value> = report
        .findings
        .iter()
        .map(|f| {
            let mut result = json!({
                "ruleId": f.rule,
                "ruleIndex": RULES.iter().position(|r| r.id == f.rule).unwrap_or(0),
                "level": f.severity.sarif_level(),
                "message": { "text": f.message },
                "locations": [{
                    "physicalLocation": {
                        "artifactLocation": { "uri": f.file },
                        "region": { "startLine": f.line.max(1), "startColumn": f.column.max(1) }
                    }
                }],
                "partialFingerprints": {
                    "realismGate/v1": format!("{}:{}:{}", f.rule, f.file, f.evidence)
                },
                "properties": { "suite": f.suite, "tests": f.tests, "evidence": f.evidence },
            });
            if let Some(s) = &f.suppressed {
                result["suppressions"] = json!([{
                    "kind": "inSource",
                    "justification": s.reason,
                }]);
            }
            result
        })
        .collect();
    let doc = json!({
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": {
                "driver": {
                    "name": TOOL,
                    "version": VERSION,
                    "semanticVersion": VERSION,
                    "informationUri": REPOSITORY,
                    "rules": rules,
                }
            },
            "results": results,
        }]
    });
    let mut out = serde_json::to_string_pretty(&doc).expect("sarif serializes");
    out.push('\n');
    out
}
