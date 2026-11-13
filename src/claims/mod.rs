//! Claims: what a finding says its PoC shows, bound to the evidence that
//! would show it.
//!
//! A report can carry its claims as structured data (`claims.yaml`, schema
//! `realism-gate.claims/v1`), as a YAML front-matter block at the top of the
//! report, or not at all. When it does not, the reader extracts what parses
//! reliably and labels the result `inferred`; an inferred binding never
//! counts as a pass.
//!
//! The claim fields are fixed, and each one has a predicate in
//! [`crate::bind`]. docs/claims.md is the table.

pub mod extract;
pub mod init;

use crate::project::display_path;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;

/// The schema id a `claims.yaml` names.
pub const SCHEMA: &str = "realism-gate.claims/v1";
/// The default file name looked for next to the report.
pub const FILE_NAME: &str = "claims.yaml";

/// The status of one claim after binding it to the PoC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// The PoC contains the evidence this claim needs.
    Shown,
    /// The PoC does not contain that evidence.
    NotShown,
    /// The PoC contains evidence for the opposite.
    Contradicted,
    /// The claim cannot be bound from source: no target commit, an
    /// uncheckable precondition, an unrecognised impact.
    NotCheckable,
}

impl Status {
    /// The uppercase label the table prints.
    pub fn label(self) -> &'static str {
        match self {
            Status::Shown => "SHOWN",
            Status::NotShown => "NOT SHOWN",
            Status::Contradicted => "CONTRADICTED",
            Status::NotCheckable => "NOT CHECKABLE",
        }
    }

    /// The JSON value.
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Shown => "shown",
            Status::NotShown => "not_shown",
            Status::Contradicted => "contradicted",
            Status::NotCheckable => "not_checkable",
        }
    }
}

/// How the claims were read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Binding {
    /// Parsed from `claims.yaml` or a front-matter block.
    Structured,
    /// Extracted from prose by heuristics.
    Inferred,
}

impl Binding {
    /// The JSON value.
    pub fn as_str(self) -> &'static str {
        match self {
            Binding::Structured => "structured",
            Binding::Inferred => "inferred",
        }
    }
}

/// The top line of the table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// Every claim is shown, or nothing but the fix row is open.
    Shown,
    /// Nothing is contradicted, but a claim is not shown or not checkable.
    Incomplete,
    /// A claim is contradicted by the PoC. Exits non-zero.
    Contradicted,
}

/// Who the PoC says the attacker is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Attacker {
    /// Any account with no role.
    AnyUser,
    /// An account holding a named role.
    Role(String),
    /// A named address.
    Address(String),
}

impl Attacker {
    fn parse(text: &str) -> Result<Attacker, String> {
        let t = text.trim();
        if let Some(rest) = t.strip_prefix("role:") {
            let name = rest.trim();
            if name.is_empty() {
                return Err("attacker: `role:` needs a role name".to_string());
            }
            return Ok(Attacker::Role(name.to_string()));
        }
        if let Some(rest) = t.strip_prefix("address:") {
            let name = rest.trim();
            if name.is_empty() {
                return Err("attacker: `address:` needs an address".to_string());
            }
            return Ok(Attacker::Address(name.to_string()));
        }
        match t.to_ascii_lowercase().as_str() {
            "any-user" | "any_user" | "any user" | "unprivileged" | "anyone" => {
                Ok(Attacker::AnyUser)
            }
            _ if t.starts_with("0x") && t.len() >= 10 => Ok(Attacker::Address(t.to_string())),
            other => Err(format!(
                "attacker: `{other}` is not `any-user`, `role:<NAME>` or `address:<0x...>`"
            )),
        }
    }

    /// The text the table prints.
    pub fn statement(&self) -> String {
        match self {
            Attacker::AnyUser => "any user, no role".to_string(),
            Attacker::Role(name) => format!("role:{name}"),
            Attacker::Address(a) => format!("address:{a}"),
        }
    }

    /// The role this attacker holds, when it holds one.
    pub fn role(&self) -> Option<&str> {
        match self {
            Attacker::Role(name) => Some(name),
            _ => None,
        }
    }
}

/// What the PoC says happens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Impact {
    /// Value leaves the protocol.
    Drain,
    /// Funds or a channel are stuck.
    Freeze,
    /// The protocol stops working.
    Halt,
    /// A call reverts, with no loss shown.
    Dos,
    /// The accounting is wrong.
    MisAccounting,
    /// A free-text impact. The predicate reads keywords out of it.
    Other(String),
}

impl Impact {
    fn parse(text: &str) -> Result<Impact, String> {
        let t = text.trim();
        if let Some(rest) = t.strip_prefix("other:") {
            let free = rest.trim();
            if free.is_empty() {
                return Err("impact: `other:` needs text".to_string());
            }
            return Ok(Impact::Other(free.to_string()));
        }
        match t.to_ascii_lowercase().as_str() {
            "drain" | "theft" | "steal" => Ok(Impact::Drain),
            "freeze" | "locked" => Ok(Impact::Freeze),
            "halt" | "shutdown" => Ok(Impact::Halt),
            "dos" | "denial-of-service" => Ok(Impact::Dos),
            "mis-accounting" | "misaccounting" | "accounting" => Ok(Impact::MisAccounting),
            other => Err(format!(
                "impact: `{other}` is not drain, freeze, halt, dos, mis-accounting or `other:<text>`"
            )),
        }
    }

    /// The text the table prints.
    pub fn statement(&self) -> String {
        match self {
            Impact::Drain => "drain".to_string(),
            Impact::Freeze => "freeze".to_string(),
            Impact::Halt => "halt".to_string(),
            Impact::Dos => "dos".to_string(),
            Impact::MisAccounting => "mis-accounting".to_string(),
            Impact::Other(text) => format!("other:{text}"),
        }
    }
}

/// What a finding claims. Every field is optional: a report that claims less
/// gets fewer rows, not a weaker check.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Claims {
    pub attacker: Option<Attacker>,
    pub impact: Option<Impact>,
    pub asset: Option<String>,
    pub amount: Option<String>,
    pub preconditions: Vec<String>,
    pub cited: Vec<String>,
    pub severity: Option<String>,
    pub fix: Option<String>,
}

impl Claims {
    /// Parses `claims.yaml` or a front-matter block.
    pub fn parse(text: &str) -> Result<Claims, String> {
        let raw: Raw = serde_saphyr::from_str(text).map_err(|e| e.to_string())?;
        let raw = raw.flatten();
        if let Some(schema) = &raw.schema
            && schema.trim() != SCHEMA
        {
            return Err(format!(
                "schema: expected `{SCHEMA}`, found `{}`",
                schema.trim()
            ));
        }
        let mut out = Claims::default();
        if let Some(a) = &raw.attacker {
            out.attacker = Some(Attacker::parse(a)?);
        }
        if let Some(i) = &raw.impact {
            out.impact = Some(Impact::parse(i)?);
        }
        out.asset = non_empty(raw.asset, "asset")?;
        out.amount = non_empty(raw.amount, "amount")?;
        out.severity = non_empty(raw.severity, "severity")?;
        out.fix = non_empty(raw.fix, "fix")?;
        for p in raw.preconditions {
            out.preconditions.push(required(p, "preconditions")?);
        }
        for c in raw.cited {
            out.cited.push(required(c, "cited")?);
        }
        if out.is_empty() {
            return Err(format!(
                "no claims: name at least one of attacker, impact, asset, amount, preconditions, cited, fix (schema {SCHEMA})"
            ));
        }
        Ok(out)
    }

    /// True when the block states nothing.
    pub fn is_empty(&self) -> bool {
        self.attacker.is_none()
            && self.impact.is_none()
            && self.asset.is_none()
            && self.amount.is_none()
            && self.preconditions.is_empty()
            && self.cited.is_empty()
            && self.severity.is_none()
            && self.fix.is_none()
    }

    /// Every claim field this block states, in table order.
    pub fn rows(&self) -> Vec<RowClaim> {
        let mut out = Vec::new();
        if let Some(a) = &self.attacker {
            out.push(RowClaim {
                field: "attacker",
                claim: a.statement(),
            });
        }
        if let Some(i) = &self.impact {
            out.push(RowClaim {
                field: "impact",
                claim: i.statement(),
            });
        }
        if let Some(a) = &self.asset {
            out.push(RowClaim {
                field: "asset",
                claim: a.clone(),
            });
        }
        if let Some(a) = &self.amount {
            out.push(RowClaim {
                field: "amount",
                claim: a.clone(),
            });
        }
        if !self.preconditions.is_empty() {
            out.push(RowClaim {
                field: "preconditions",
                claim: self.preconditions.join("; "),
            });
        }
        if !self.cited.is_empty() {
            out.push(RowClaim {
                field: "cited",
                claim: self.cited.join(", "),
            });
        }
        if let Some(f) = &self.fix {
            out.push(RowClaim {
                field: "fix",
                claim: f.clone(),
            });
        }
        out
    }
}

/// One claim field's statement, before binding.
#[derive(Debug, Clone)]
pub struct RowClaim {
    pub field: &'static str,
    pub claim: String,
}

fn required(value: String, field: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!("{field}: an empty entry"));
    }
    Ok(trimmed.to_string())
}

fn non_empty(value: Option<String>, field: &str) -> Result<Option<String>, String> {
    match value {
        None => Ok(None),
        Some(v) if v.trim().is_empty() => Err(format!("{field}: an empty value")),
        Some(v) => Ok(Some(v.trim().to_string())),
    }
}

/// One row of the claim table.
#[derive(Debug, Clone, Serialize)]
pub struct Row {
    /// attacker, impact, asset, amount, preconditions, cited, fix.
    pub field: &'static str,
    /// What the finding claims.
    pub claim: String,
    /// What in the PoC was looked at.
    pub evidence: String,
    pub status: Status,
}

/// The bound claim table, as printed and as serialized under `claims`.
#[derive(Debug, Clone, Serialize)]
pub struct Table {
    pub schema: &'static str,
    pub binding: Binding,
    /// Where the claims came from: a path, a front-matter block, or prose.
    pub source: String,
    pub outcome: Outcome,
    pub shown: usize,
    pub not_shown: usize,
    pub contradicted: usize,
    pub not_checkable: usize,
    pub rows: Vec<Row>,
}

impl Table {
    /// Builds the table from bound rows. The counts and the outcome follow
    /// from the rows; a fix row that is not shown does not make the PoC
    /// incomplete, because `check` never runs a patch.
    pub fn new(binding: Binding, source: String, rows: Vec<Row>) -> Table {
        let count = |s: Status| rows.iter().filter(|r| r.status == s).count();
        let shown = count(Status::Shown);
        let contradicted = count(Status::Contradicted);
        let not_checkable = count(Status::NotCheckable);
        let not_shown = count(Status::NotShown);
        let open_non_fix = rows
            .iter()
            .any(|r| r.field != "fix" && r.status != Status::Shown);
        let outcome = if contradicted > 0 {
            Outcome::Contradicted
        } else if binding == Binding::Inferred || open_non_fix || not_checkable > 0 {
            Outcome::Incomplete
        } else {
            Outcome::Shown
        };
        Table {
            schema: SCHEMA,
            binding,
            source,
            outcome,
            shown,
            not_shown,
            contradicted,
            not_checkable,
            rows,
        }
    }

    /// The top line, after the rule findings.
    pub fn verdict(&self) -> String {
        match self.outcome {
            Outcome::Contradicted => format!(
                "CONTRADICTED  {} of {} claims are contradicted by the PoC",
                self.contradicted,
                self.rows.len()
            ),
            Outcome::Incomplete if self.binding == Binding::Inferred => {
                "INCOMPLETE  claims inferred".to_string()
            }
            Outcome::Incomplete => format!(
                "INCOMPLETE  {} shown, {} not shown, {} not checkable",
                self.shown, self.not_shown, self.not_checkable
            ),
            Outcome::Shown if self.not_shown > 0 => {
                "COMPLETE  every claim the PoC can show is shown; the fix was not run".to_string()
            }
            Outcome::Shown => "COMPLETE  every claim is shown by the PoC".to_string(),
        }
    }

    /// Non-zero when the table says the report does not hold: a contradiction
    /// is a defect, and an inferred binding is not a pass.
    pub fn fails(&self) -> bool {
        self.outcome == Outcome::Contradicted || self.binding == Binding::Inferred
    }
}

/// Claims plus where they came from.
#[derive(Debug, Clone)]
pub struct Loaded {
    pub claims: Claims,
    pub binding: Binding,
    pub source: String,
    /// The claims text, when it came from a file or a front-matter block.
    /// `None` for an inferred binding: there is nothing to hash.
    pub text: Option<String>,
}

impl fmt::Display for Loaded {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.source)
    }
}

/// Reads claims for a run.
///
/// Order: `--claims FILE`, then `claims.yaml` next to the report, then a
/// front-matter block in the report, then heuristics over the report's prose.
pub fn resolve(
    report: Option<&Path>,
    claims: Option<&Path>,
    base: &Path,
) -> Result<Option<Loaded>, String> {
    if let Some(path) = claims {
        let text = read(path)?;
        let parsed = Claims::parse(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        return Ok(Some(Loaded {
            claims: parsed,
            binding: Binding::Structured,
            source: display_path(path, base),
            text: Some(text),
        }));
    }
    let Some(report) = report else {
        return Ok(None);
    };
    let dir = report.parent().unwrap_or(Path::new("."));
    let sibling = dir.join(FILE_NAME);
    if sibling.is_file() {
        let text = read(&sibling)?;
        let parsed = Claims::parse(&text).map_err(|e| format!("{}: {e}", sibling.display()))?;
        return Ok(Some(Loaded {
            claims: parsed,
            binding: Binding::Structured,
            source: display_path(&sibling, base),
            text: Some(text),
        }));
    }
    let text = read(report)?;
    if let Some(block) = front_matter(&text) {
        let parsed =
            Claims::parse(block).map_err(|e| format!("{} front matter: {e}", report.display()))?;
        return Ok(Some(Loaded {
            claims: parsed,
            binding: Binding::Structured,
            source: format!("front matter of {}", display_path(report, base)),
            text: Some(block.to_string()),
        }));
    }
    Ok(Some(Loaded {
        claims: extract::inferred(&text),
        binding: Binding::Inferred,
        source: format!("{} (inferred)", display_path(report, base)),
        text: None,
    }))
}

fn read(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}

/// The YAML front-matter block of a markdown file: the text between a leading
/// `---` line and the next `---` line.
pub fn front_matter(text: &str) -> Option<&str> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let first_end = line_end(text)?;
    if text[..first_end].trim() != "---" {
        return None;
    }
    let mut offset = first_end + 1;
    let body_start = offset;
    while offset <= text.len() {
        let rest = &text[offset..];
        let Some(end) = line_end(rest) else { break };
        if rest[..end].trim() == "---" {
            return Some(&text[body_start..offset]);
        }
        offset += end + 1;
    }
    None
}

/// The byte offset of the newline that ends the first line of `text`, or the
/// length of `text` when it has none.
fn line_end(text: &str) -> Option<usize> {
    if text.is_empty() {
        return None;
    }
    Some(text.find('\n').unwrap_or(text.len()))
}

/// Serde view of a claims block. Unknown keys are rejected so a typo is an
/// error rather than a silently missing row. A nested `claims:` key is
/// accepted because a report's front matter may hold other keys too.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    #[serde(default)]
    schema: Option<String>,
    #[serde(default)]
    claims: Option<Box<Raw>>,
    #[serde(default)]
    attacker: Option<String>,
    #[serde(default)]
    impact: Option<String>,
    #[serde(default)]
    asset: Option<String>,
    #[serde(default)]
    amount: Option<String>,
    #[serde(default)]
    preconditions: Vec<String>,
    #[serde(default)]
    cited: Vec<String>,
    #[serde(default)]
    severity: Option<String>,
    #[serde(default)]
    fix: Option<String>,
}

impl Raw {
    /// Merges a nested `claims:` block into the top level. The top level wins
    /// for `schema`; a field named in both is an error.
    fn flatten(self) -> Raw {
        let Some(inner) = self.claims else {
            return self;
        };
        let inner = inner.flatten();
        Raw {
            schema: self.schema.or(inner.schema),
            claims: None,
            attacker: self.attacker.or(inner.attacker),
            impact: self.impact.or(inner.impact),
            asset: self.asset.or(inner.asset),
            amount: self.amount.or(inner.amount),
            preconditions: if self.preconditions.is_empty() {
                inner.preconditions
            } else {
                self.preconditions
            },
            cited: if self.cited.is_empty() {
                inner.cited
            } else {
                self.cited
            },
            severity: self.severity.or(inner.severity),
            fix: self.fix.or(inner.fix),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_structured_block() {
        let c = Claims::parse(
            "schema: realism-gate.claims/v1\nattacker: any-user\nimpact: freeze\nasset: USDR\namount: capital-neutral\npreconditions:\n  - no privileged role\ncited:\n  - src/VaultEngine.sol:open\nseverity: high\nfix: restrict the vault\n",
        )
        .unwrap();
        assert_eq!(c.attacker, Some(Attacker::AnyUser));
        assert_eq!(c.impact, Some(Impact::Freeze));
        assert_eq!(c.cited, vec!["src/VaultEngine.sol:open"]);
        assert_eq!(c.preconditions, vec!["no privileged role"]);
        let fields: Vec<&str> = c.rows().iter().map(|r| r.field).collect();
        assert_eq!(
            fields,
            vec![
                "attacker",
                "impact",
                "asset",
                "amount",
                "preconditions",
                "cited",
                "fix"
            ]
        );
    }

    #[test]
    fn rejects_typos_and_unknown_values() {
        assert!(Claims::parse("attacer: any-user\n").is_err());
        assert!(Claims::parse("attacker: admin\n").is_err());
        assert!(Claims::parse("impact: destroy\n").is_err());
        assert!(Claims::parse("schema: realism-gate.claims/v2\nimpact: halt\n").is_err());
        assert!(Claims::parse("asset: \"\"\n").is_err());
        assert!(Claims::parse("# nothing here\n").is_err());
    }

    #[test]
    fn accepts_a_nested_claims_key() {
        let c = Claims::parse(
            "claims:\n  attacker: role:ADMIN\n  impact: other:PSM.sellStable reverts\n",
        )
        .unwrap();
        assert_eq!(c.attacker, Some(Attacker::Role("ADMIN".to_string())));
        assert_eq!(
            c.impact,
            Some(Impact::Other("PSM.sellStable reverts".to_string()))
        );
    }

    #[test]
    fn reads_front_matter() {
        let text = "---\nschema: realism-gate.claims/v1\nimpact: drain\n---\n\n# Finding\n";
        assert_eq!(
            front_matter(text).unwrap(),
            "schema: realism-gate.claims/v1\nimpact: drain\n"
        );
        assert!(front_matter("# no front matter\n").is_none());
        // An unclosed block is not front matter.
        assert!(front_matter("---\nimpact: drain\n").is_none());
        // A horizontal rule later in the body is not a block either.
        assert!(front_matter("text\n---\nmore\n").is_none());
    }

    #[test]
    fn the_outcome_follows_the_rows() {
        let row = |field, status| Row {
            field,
            claim: "c".to_string(),
            evidence: "e".to_string(),
            status,
        };
        let t = Table::new(
            Binding::Structured,
            "claims.yaml".to_string(),
            vec![row("attacker", Status::Shown), row("fix", Status::NotShown)],
        );
        assert_eq!(t.outcome, Outcome::Shown);
        assert!(!t.fails());
        assert!(t.verdict().contains("the fix was not run"));

        let t = Table::new(
            Binding::Structured,
            "claims.yaml".to_string(),
            vec![row("attacker", Status::Contradicted)],
        );
        assert!(t.fails());

        let t = Table::new(
            Binding::Inferred,
            "finding.md (inferred)".to_string(),
            vec![row("attacker", Status::NotCheckable)],
        );
        assert_eq!(t.verdict(), "INCOMPLETE  claims inferred");
        assert!(t.fails());
    }
}
