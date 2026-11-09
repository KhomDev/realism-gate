//! Findings and their severity.

use serde::Serialize;

/// How serious a finding is. Ordered: `Note < Warn < Error`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Context a reviewer should know. Never fails a run on its own.
    Note,
    /// The PoC may still be valid, but it needs a justification it does not give.
    Warn,
    /// The PoC, as written, does not show what an unprivileged attacker can do.
    Error,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Note => "note",
            Severity::Warn => "warn",
            Severity::Error => "error",
        }
    }

    /// The SARIF 2.1.0 `level` for this severity.
    pub fn sarif_level(self) -> &'static str {
        match self {
            Severity::Note => "note",
            Severity::Warn => "warning",
            Severity::Error => "error",
        }
    }
}

/// An inline `realism-gate: allow(...)` comment that silenced a finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Suppression {
    pub line: u32,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    pub rule: &'static str,
    pub slug: &'static str,
    pub severity: Severity,
    pub file: String,
    pub line: u32,
    pub column: u32,
    /// The test contract the finding was raised in.
    pub suite: String,
    /// Every test the finding applies to. A line in `setUp()` applies to all
    /// of a suite's tests and is reported once.
    pub tests: Vec<String>,
    pub message: String,
    /// The source the finding points at.
    pub evidence: String,
    /// The one-line fix from the rule's documentation.
    pub help: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suppressed: Option<Suppression>,
    /// Set when a suppression comment was found but ignored.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignored_suppression: Option<u32>,
    /// Why that comment was ignored: no reason, or the comment is inside the
    /// repo under review and is not operator policy.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignored_reason: Option<String>,
}

impl Finding {
    pub fn is_active(&self) -> bool {
        self.suppressed.is_none()
    }
}
