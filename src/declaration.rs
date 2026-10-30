//! `realism.yaml`: the attacker model a PoC claims, written down.
//!
//! The code rules infer who is who from names and data flow. A declaration
//! replaces the inference with facts the author states, and adds facts no code
//! can show: which steps depend on someone else acting, and in which
//! transaction each step runs. RG12-RG14 read only the declaration.

use crate::finding::Severity;
use crate::identity::compact;
use crate::rules::Raw;
use serde::Deserialize;
use std::fmt;

pub const FILE_NAME: &str = "realism.yaml";

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Declaration {
    pub version: u32,
    #[serde(default)]
    pub chain: Chain,
    #[serde(default)]
    pub attacker: Identities,
    #[serde(default)]
    pub victim: Identities,
    #[serde(default)]
    pub privileged: Vec<PrivilegedStep>,
    #[serde(default)]
    pub preconditions: Vec<Precondition>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Chain {
    /// One sender per transaction; nobody can append a call to a transaction
    /// someone else signs.
    #[default]
    Evm,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Identities {
    #[serde(default)]
    pub identities: Vec<String>,
}

/// A privileged address the exploit is allowed to act as, and why.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrivilegedStep {
    pub identity: String,
    pub role: String,
    pub reason: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Precondition {
    pub step: String,
    pub performed_by: Actor,
    pub boundary: Boundary,
    #[serde(default)]
    pub ordering_guarantee: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Actor {
    Attacker,
    Victim,
    Privileged,
    ThirdParty,
    /// Pre-existing chain state rather than an action ("the pool is imbalanced").
    State,
}

impl Actor {
    fn as_str(self) -> &'static str {
        match self {
            Actor::Attacker => "attacker",
            Actor::Victim => "victim",
            Actor::Privileged => "privileged",
            Actor::ThirdParty => "third_party",
            Actor::State => "state",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Boundary {
    /// The actor's own transaction.
    OwnTransaction,
    /// Inside a transaction another account signs.
    VictimTransaction,
    /// A separate transaction in the same block as another one.
    SameBlock,
    /// Attacker code invoked by the victim's own call (a hook, a callback).
    CallbackFromVictimCall,
    /// Triggered by something outside the chain: an oracle update, a relayer.
    ExternalTrigger,
}

#[derive(Debug)]
pub struct DeclarationError(pub String);

impl fmt::Display for DeclarationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for DeclarationError {}

impl Declaration {
    pub fn parse(text: &str) -> Result<Declaration, DeclarationError> {
        let decl: Declaration =
            serde_saphyr::from_str(text).map_err(|e| DeclarationError(e.to_string()))?;
        if decl.version != 1 {
            return Err(DeclarationError(format!(
                "unsupported version {} (this build reads version 1)",
                decl.version
            )));
        }
        for p in &decl.privileged {
            if p.reason.trim().is_empty() {
                return Err(DeclarationError(format!(
                    "privileged identity `{}` has an empty reason; the reason is the point of declaring it",
                    p.identity
                )));
            }
        }
        Ok(decl)
    }

    /// Identities in canonical form, as [`crate::identity::key`] produces them.
    pub fn attackers(&self) -> impl Iterator<Item = String> + '_ {
        self.attacker.identities.iter().map(|s| canonical(s))
    }

    pub fn victims(&self) -> impl Iterator<Item = String> + '_ {
        self.victim.identities.iter().map(|s| canonical(s))
    }

    pub fn privileged_identities(&self) -> impl Iterator<Item = String> + '_ {
        self.privileged.iter().map(|p| canonical(&p.identity))
    }

    /// RG12-RG14. `unit` is the index the declaration's findings are filed
    /// under; `text` is the file's contents, used to find line numbers.
    pub fn check(&self, unit: usize, text: &str) -> Vec<Raw> {
        let mut out = Vec::new();
        for p in &self.preconditions {
            let pos = crate::ir::Pos {
                line: line_of(text, &p.step),
                col: 1,
            };
            let evidence = format!(
                "{}: performed_by {}, boundary {:?}",
                p.step,
                p.performed_by.as_str(),
                p.boundary
            );
            if p.performed_by == Actor::Attacker && p.boundary == Boundary::VictimTransaction {
                out.push(Raw {
                    rule: "RG12",
                    severity: Severity::Error,
                    unit,
                    pos,
                    message: format!(
                        "\"{}\" puts an attacker step inside the victim's transaction. An EVM transaction has one sender; no one can append a call to a transaction another account signs.",
                        p.step
                    ),
                    evidence: evidence.clone(),
                });
            }
            let unordered = p
                .ordering_guarantee
                .as_deref()
                .is_none_or(|g| g.trim().is_empty());
            if p.performed_by == Actor::Attacker && p.boundary == Boundary::SameBlock && unordered {
                out.push(Raw {
                    rule: "RG13",
                    severity: Severity::Warn,
                    unit,
                    pos,
                    message: format!(
                        "\"{}\" needs to land in the same block as another transaction but names no ordering mechanism.",
                        p.step
                    ),
                    evidence: evidence.clone(),
                });
            }
            if matches!(
                p.performed_by,
                Actor::Victim | Actor::Privileged | Actor::ThirdParty
            ) {
                out.push(Raw {
                    rule: "RG14",
                    severity: Severity::Note,
                    unit,
                    pos,
                    message: format!(
                        "The attack depends on the {} acting: \"{}\". The severity argument has to carry that condition.",
                        p.performed_by.as_str().replace('_', " "),
                        p.step
                    ),
                    evidence,
                });
            }
        }
        out
    }
}

/// `address(attacker)` and `attacker` name the same identity.
fn canonical(s: &str) -> String {
    let c = compact(s);
    for wrapper in ["address(", "payable("] {
        if let Some(inner) = c.strip_prefix(wrapper).and_then(|r| r.strip_suffix(')')) {
            return canonical(inner);
        }
    }
    c
}

fn line_of(text: &str, needle: &str) -> u32 {
    text.lines()
        .position(|l| l.contains(needle))
        .map(|i| i as u32 + 1)
        .unwrap_or(1)
}

/// The commented template `realism-gate init` writes.
pub const TEMPLATE: &str = r#"# realism.yaml: the attacker model this PoC claims.
#
# realism-gate infers who is who from names and data flow. Everything you
# declare here replaces that inference, so declare what is true, not what makes
# the output quiet. A triager will read this file next to your report.
version: 1
chain: evm

attacker:
  # The addresses the attacker controls, as written in the test.
  identities: [attacker]

victim:
  identities: []

# Privileged addresses the exploit body is allowed to act as. Each one is a
# condition on the attack, and the report has to argue why it holds.
privileged: []
#  - identity: keeper
#    role: keeper
#    reason: harvest() runs every block from a public keeper network; the attacker only waits for it

# Every step the attack needs, who performs it, and in which transaction.
#   performed_by: attacker | victim | privileged | third_party | state
#   boundary:     own_transaction | victim_transaction | same_block
#                 | callback_from_victim_call | external_trigger
preconditions: []
#  - step: a user deposits into the empty vault
#    performed_by: victim
#    boundary: own_transaction
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_template_parses() {
        let d = Declaration::parse(TEMPLATE).unwrap();
        assert_eq!(d.version, 1);
        assert_eq!(d.attackers().collect::<Vec<_>>(), vec!["attacker"]);
    }

    #[test]
    fn unknown_fields_and_versions_are_rejected() {
        assert!(Declaration::parse("version: 1\nattackers: {}\n").is_err());
        assert!(Declaration::parse("version: 2\n").is_err());
        assert!(Declaration::parse(
            "version: 1\npreconditions:\n  - step: x\n    performed_by: wizard\n    boundary: own_transaction\n"
        )
        .is_err());
    }

    #[test]
    fn a_privileged_entry_needs_a_reason() {
        let err = Declaration::parse(
            "version: 1\nprivileged:\n  - identity: keeper\n    role: keeper\n    reason: \"  \"\n",
        )
        .unwrap_err();
        assert!(err.0.contains("empty reason"));
    }

    #[test]
    fn identities_are_canonical() {
        let d = Declaration::parse(
            "version: 1\nattacker:\n  identities: [\"address(exploit)\", \"payable( attacker )\"]\n",
        )
        .unwrap();
        assert_eq!(
            d.attackers().collect::<Vec<_>>(),
            vec!["exploit", "attacker"]
        );
    }

    #[test]
    fn boundary_rules() {
        let text = "version: 1\npreconditions:\n  - step: swap inside deposit\n    performed_by: attacker\n    boundary: victim_transaction\n  - step: back-run\n    performed_by: attacker\n    boundary: same_block\n  - step: user deposits\n    performed_by: victim\n    boundary: own_transaction\n  - step: ordered\n    performed_by: attacker\n    boundary: same_block\n    ordering_guarantee: bundle\n";
        let d = Declaration::parse(text).unwrap();
        let raws = d.check(0, text);
        let rules: Vec<(&str, u32)> = raws.iter().map(|r| (r.rule, r.pos.line)).collect();
        assert_eq!(rules, vec![("RG12", 3), ("RG13", 6), ("RG14", 9)]);
    }
}
