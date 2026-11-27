//! The report the engine assembles and the renderers print.
//!
//! The field names and their JSON shape are the stable `report/v1` interface.
//! They live here, below the binary crate, because the renderers and the claim
//! binder consume them and must not depend on the binary that builds them.

use crate::claims::{Binding, Table};
use rg_ir::finding::Finding;
use serde::Serialize;

/// `--changed-since`: the revision and how many files differ from it.
#[derive(Debug, Clone, Serialize)]
pub struct ChangedSince {
    #[serde(rename = "ref")]
    pub git_ref: String,
    pub files: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ParseError {
    pub file: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Summary {
    pub files: usize,
    pub suites: usize,
    pub tests: usize,
    pub errors: usize,
    pub warnings: usize,
    pub notes: usize,
    pub suppressed: usize,
    /// Allow comments ignored because they give no reason. The findings they
    /// name stay active.
    pub suppressions_ignored: usize,
}

fn is_false(b: &bool) -> bool {
    !*b
}

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub summary: Summary,
    pub declaration: Option<String>,
    /// The `realism-gate.toml` that was applied, if any.
    pub config: Option<String>,
    /// The input was read as a fragment: a test body pasted into an issue,
    /// wrapped and parsed as one test. Added in 1.2.0.
    #[serde(default, skip_serializing_if = "is_false")]
    pub fragment: bool,
    pub changed_since: Option<ChangedSince>,
    pub findings: Vec<Finding>,
    pub parse_errors: Vec<ParseError>,
    /// Policy files found inside the repo under review and not applied.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ignored_policy: Vec<String>,
    /// The claim table, when `--report` or `--claims` was given. Added in 1.0.0.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claims: Option<Table>,
    /// How the claims were read: `structured` or `inferred`. Added in 1.0.0.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claim_binding: Option<Binding>,
}

impl Report {
    pub fn active(&self) -> impl Iterator<Item = &Finding> {
        self.findings.iter().filter(|f| f.is_active())
    }

    /// Whether any active finding is at or above `level`.
    pub fn fails(&self, level: rg_ir::finding::Severity) -> bool {
        self.active().any(|f| f.severity >= level)
    }
}
