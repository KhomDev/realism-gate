//! The evidence card: a small, reproducible summary a hunter attaches to a
//! submission.
//!
//! The card names what was checked and hashes it. It is signed by nothing: the
//! point is that any reader can re-run one command and get the same table, and
//! can hash the same files to see they did not change. See
//! docs/evidence-card.md.

use super::sha256;
use super::{TOOL, VERSION};
use crate::claims::{Loaded, Table};
use crate::project::Project;
use crate::rules::RULES;
use serde::Serialize;

/// The card's JSON schema id.
pub const SCHEMA: &str = "realism-gate.card/v1";

/// Everything a card states, gathered by the caller.
pub struct Input<'a> {
    pub project: &'a Project,
    pub table: &'a Table,
    pub claims: &'a Loaded,
    /// The report's own text, when `--report` was given.
    pub report_text: Option<&'a str>,
    pub report_path: Option<&'a str>,
    pub target_commit: Option<&'a str>,
    /// The command a reader can run to reproduce the table.
    pub rerun: String,
}

/// The hash of the rule set: every rule id, slug and severity in one line, in
/// catalog order. Changing a rule's meaning, its slug or its severity changes
/// the hash; a purely internal refactor does not.
pub fn rule_set_hash() -> String {
    let mut text = String::new();
    for rule in RULES {
        text.push_str(&format!(
            "{} {} {}\n",
            rule.id,
            rule.slug,
            rule.severity.as_str()
        ));
    }
    sha256::hex(text.as_bytes())
}

/// The hash of the PoC the run read: every file's display path and source, in
/// load order. A moved file or a changed byte changes the hash.
pub fn poc_hash(project: &Project) -> String {
    let mut bytes = Vec::new();
    for unit in &project.units {
        bytes.extend_from_slice(unit.path.as_bytes());
        bytes.push(0);
        bytes.extend_from_slice(unit.source.as_bytes());
        bytes.push(0);
    }
    sha256::hex(&bytes)
}

/// The fields the JSON twin publishes.
#[derive(Debug, Serialize)]
struct Json<'a> {
    schema: &'static str,
    tool: &'static str,
    version: &'static str,
    rule_set_sha256: String,
    poc_sha256: String,
    report_sha256: Option<String>,
    claims_sha256: Option<String>,
    claims_source: &'a str,
    claim_binding: &'static str,
    target_commit: Option<&'a str>,
    rerun: &'a str,
    signed_by: &'static str,
    claims: &'a Table,
}

const SIGNED_BY: &str =
    "nothing: every field is a hash of a file named here, or a value the rerun command reproduces";

/// The JSON twin of the card.
pub fn json(input: &Input<'_>) -> String {
    let doc = Json {
        schema: SCHEMA,
        tool: TOOL,
        version: VERSION,
        rule_set_sha256: rule_set_hash(),
        poc_sha256: poc_hash(input.project),
        report_sha256: input
            .report_text
            .map(|t| sha256::hex(t.replace("\r\n", "\n").as_bytes())),
        claims_sha256: input
            .claims
            .text
            .as_ref()
            .map(|t| sha256::hex(t.replace("\r\n", "\n").as_bytes())),
        claims_source: &input.claims.source,
        claim_binding: input.claims.binding.as_str(),
        target_commit: input.target_commit,
        rerun: &input.rerun,
        signed_by: SIGNED_BY,
        claims: input.table,
    };
    let mut out = serde_json::to_string_pretty(&doc).expect("card serializes");
    out.push('\n');
    out
}

/// The Markdown card.
pub fn markdown(input: &Input<'_>) -> String {
    let poc = poc_hash(input.project);
    let rules = rule_set_hash();
    let mut out = String::new();
    out.push_str(&format!("# Evidence card: {TOOL} {VERSION}\n\n"));
    out.push_str(
        "What this PoC shows, claim by claim, with the hashes a reader needs to check that\n\
         nothing changed. **Signed by nothing.** Every field is a hash of a file, or a value\n\
         the rerun command reproduces.\n\n",
    );
    out.push_str("| Field | Value |\n|---|---|\n");
    out.push_str(&format!("| tool | `{TOOL} {VERSION}` |\n"));
    out.push_str(&format!(
        "| rule set | RG01-RG20, sha256 `{}` |\n",
        short(&rules)
    ));
    let files = input
        .project
        .units
        .iter()
        .map(|u| format!("`{}`", u.path))
        .collect::<Vec<_>>()
        .join(", ");
    out.push_str(&format!("| PoC | sha256 `{}` ({files}) |\n", short(&poc)));
    match (&input.report_path, input.report_text) {
        (Some(path), Some(text)) => out.push_str(&format!(
            "| report | sha256 `{}` (`{path}`) |\n",
            short(&sha256::hex(text.replace("\r\n", "\n").as_bytes()))
        )),
        _ => out.push_str("| report | not given |\n"),
    }
    if let Some(text) = &input.claims.text {
        out.push_str(&format!(
            "| claims | sha256 `{}` ({}) |\n",
            short(&sha256::hex(text.replace("\r\n", "\n").as_bytes())),
            input.claims.source
        ));
    }
    out.push_str(&format!(
        "| claim binding | {} |\n",
        input.claims.binding.as_str()
    ));
    let commit = input
        .target_commit
        .map(|c| format!("`{c}`"))
        .unwrap_or_else(|| "not pinned".to_string());
    out.push_str(&format!("| target commit | {commit} |\n"));
    out.push_str(&format!("| outcome | {} |\n\n", input.table.verdict()));

    out.push_str("## Claim table\n\n");
    out.push_str(&super::text::claims_table(input.table, false));
    out.push('\n');

    out.push_str("## Reproduce\n\n```bash\n");
    out.push_str(&input.rerun);
    out.push_str("\n```\n\n");
    out.push_str(
        "A reader who runs that command on this tree gets this table. The JSON twin carries\n\
         the full digests, and docs/evidence-card.md gives the recipe for each one\n\
         (`poc_sha256` hashes every checked file's path and source, not one file).\n\
         Nothing here is signed: there is no key, no timestamp and no authority behind\n\
         it, only a command.\n",
    );
    out
}

/// The first 16 hex characters of a digest, for a table cell. The full hash is
/// in the JSON twin.
fn short(hash: &str) -> &str {
    &hash[..16.min(hash.len())]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::claims::{Binding, Claims, Row, Status};

    fn table() -> Table {
        Table::new(
            Binding::Structured,
            "claims.yaml".to_string(),
            vec![Row {
                field: "impact",
                claim: "freeze".to_string(),
                evidence: "vm.expectRevert()".to_string(),
                status: Status::Shown,
            }],
        )
    }

    #[test]
    fn the_rule_set_hash_is_stable_and_hashes_the_catalog() {
        let a = rule_set_hash();
        assert_eq!(a, rule_set_hash());
        assert_eq!(a.len(), 64);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn the_poc_hash_follows_the_source() {
        let one = Project::from_sources(&[("t.sol", "contract T {}")]);
        let two = Project::from_sources(&[("t.sol", "contract T { }")]);
        assert_eq!(poc_hash(&one), poc_hash(&one));
        assert_ne!(poc_hash(&one), poc_hash(&two));
    }

    #[test]
    fn the_card_says_it_is_signed_by_nothing() {
        let project = Project::from_sources(&[("t.sol", "contract T {}")]);
        let loaded = Loaded {
            claims: Claims::default(),
            binding: Binding::Structured,
            source: "claims.yaml".to_string(),
            text: None,
        };
        let input = Input {
            project: &project,
            table: &table(),
            claims: &loaded,
            report_text: Some("# r\n"),
            report_path: Some("finding.md"),
            target_commit: Some("abc"),
            rerun: "realism-gate check t.sol".to_string(),
        };
        let md = markdown(&input);
        assert!(md.contains("Signed by nothing"), "{md}");
        assert!(md.contains("## Reproduce"), "{md}");
        assert!(md.contains("realism-gate check t.sol"), "{md}");
        let json: serde_json::Value = serde_json::from_str(&json(&input)).unwrap();
        assert_eq!(json["schema"], SCHEMA);
        assert_eq!(json["claim_binding"], "structured");
        assert!(json["signed_by"].as_str().unwrap().starts_with("nothing"));
        assert_eq!(json["claims"]["rows"][0]["status"], "shown");
    }
}
