#![forbid(unsafe_code)]
//! The program model: what authority each entry point requires.
//!
//! This crate is a documented skeleton. RG-A0 creates it so that the rules and
//! the exploit model have one place to ask the question, and RG-A1 fills it
//! with the Anchor, native Solana and Solidity resolvers. Nothing here reads a
//! file, parses source, resolves an inherited modifier or guesses: the whole
//! crate is the type contract, and every method that could answer is left for
//! RG-A1.
//!
//! The contract exists because a rule that says "the attacker needed a role"
//! is only as good as its answer to *which* role, *where* the requirement is
//! written, and *how sure* the resolver is. [`Authority::Unknown`] is a
//! first-class answer with a reason, never a silent guess.
//!
//! [`EntryPoint`] and [`Authority`] are deliberately independent of any one
//! language. A Solidity function with `onlyOwner`, an Anchor instruction with a
//! `Signer<'info>` and a native Solana handler that checks `is_signer` all
//! produce the same shape, so the exploit model in `rg-model` can consume them
//! without caring which frontend produced them.

use rg_ir::ir::Pos;
use std::path::Path;
use std::path::PathBuf;

/// The authority a caller must hold to reach an entry point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Authority {
    /// No caller-held authority is required. This is an answer, not a default:
    /// a resolver that could not decide returns [`Authority::Unknown`].
    None,
    /// A signature by the named account: an Anchor `Signer<'info>`, a native
    /// `is_signer` check, or a Foundry caller the resolver can name.
    Signer(String),
    /// Ownership of the named account: Solidity `owner()` / `onlyOwner`, an
    /// Anchor `has_one`, or an `owner =` constraint.
    Owner(String),
    /// Membership in the named role: Solidity `onlyRole(X)` or `hasRole`, or an
    /// Anchor authority field compared against a configured key.
    Role(String),
    /// The resolver could not decide, and says why. Never a guess.
    Unknown(String),
}

/// One externally reachable entry point and its required authorities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryPoint {
    /// The program or contract the entry point belongs to, as its source names
    /// it.
    pub program: String,
    /// The instruction, function or handler name.
    pub name: String,
    /// Every authority the entry point requires. An empty vector means the
    /// resolver checked and found none; it does not mean "not resolved".
    pub requires: Vec<Authority>,
    /// Where the declaration starts: line and column, 1-based.
    pub pos: Pos,
}

/// Why a program model could not answer.
#[derive(Debug)]
pub enum ProgramError {
    /// The target tree could not be read.
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    /// The target uses a language, framework or layout the resolver does not
    /// support. The string says which, for the report.
    Unsupported(String),
}

/// A language frontend that resolves entry points for one target tree.
///
/// An implementation reads only inside `target`: the program model is what the
/// audited program requires, and a deployment configuration elsewhere on the
/// machine is not evidence for it.
pub trait ProgramModel {
    /// Every entry point in every program under `target`.
    fn entry_points(&self, target: &Path) -> Result<Vec<EntryPoint>, ProgramError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_is_an_answer_with_a_reason() {
        let a = Authority::Unknown("inherited modifier not resolved".to_string());
        assert_eq!(
            a,
            Authority::Unknown("inherited modifier not resolved".to_string())
        );
        assert_ne!(a, Authority::None);
    }

    #[test]
    fn an_entry_point_reports_every_requirement_in_order() {
        let ep = EntryPoint {
            program: "vault".to_string(),
            name: "withdraw".to_string(),
            requires: vec![
                Authority::Signer("attacker".to_string()),
                Authority::Role("keeper".to_string()),
            ],
            pos: Pos { line: 42, col: 5 },
        };
        assert_eq!(ep.requires.len(), 2);
        assert_eq!(ep.pos.line, 42);
    }
}
