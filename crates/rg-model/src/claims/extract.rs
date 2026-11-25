//! Heuristic claim extraction from a report that has no claims block.
//!
//! This is the fallback, and it is deliberately small: backticked
//! identifiers, the headline's impact verb, and "any user" / "unprivileged" /
//! role words. Everything else comes back empty, and an empty field is
//! printed as NOT CHECKABLE rather than guessed. A run bound this way is
//! labelled `inferred` and never counts as a pass.

use super::{Attacker, Claims, Impact};
use crate::identity;
use regex::Regex;
use std::sync::OnceLock;

fn backtick() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"`([^`\n]{2,120})`").expect("backtick"))
}

/// Command names that appear in backticks in prose but are not citations.
const NOISE: &[&str] = &[
    "realism-gate",
    "rgate",
    "cargo",
    "forge",
    "git",
    "priorart",
    "npm",
    "pnpm",
    "yarn",
    "foundry",
];

/// Identifiers a report writes in backticks, in the order they appear, with
/// duplicates removed and a cap. Only shapes that parse reliably are kept:
/// a name, a dotted member path, a `file:function`, or a source path.
pub fn identifiers(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for caps in backtick().captures_iter(text) {
        let raw = caps[1].trim();
        if !is_identifier(raw) {
            continue;
        }
        let name = raw.trim_end_matches("()").to_string();
        if out.iter().any(|seen| seen == &name) {
            continue;
        }
        out.push(name);
        if out.len() == 8 {
            break;
        }
    }
    out
}

fn is_identifier(raw: &str) -> bool {
    if raw.is_empty() || raw.contains(char::is_whitespace) {
        return false;
    }
    let lower = raw.to_ascii_lowercase();
    if NOISE
        .iter()
        .any(|n| lower == *n || lower.starts_with(&format!("{n} ")))
    {
        return false;
    }
    // A source path, with or without a trailing `:function`.
    if raw.contains('/') && raw.contains('.') && !raw.contains("://") {
        return raw
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "/._-:".contains(c));
    }
    // A name, a dotted path, or `file.sol:function`.
    let body = raw.trim_end_matches("()");
    let mut chars = body.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    body.chars()
        .all(|c| c.is_ascii_alphanumeric() || "._:".contains(c))
}

/// The first heading in the report, with its `#` marks removed.
pub fn headline(text: &str) -> Option<&str> {
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix('#') {
            let title = rest.trim_start_matches('#').trim();
            if !title.is_empty() {
                return Some(title);
            }
        }
    }
    text.lines().map(str::trim).find(|l| !l.is_empty())
}

/// The impact verb in the headline. Only the headline is read: a verb buried
/// in the body is as likely to be describing the fix as the bug.
pub fn impact_of(headline: &str) -> Option<Impact> {
    let l = headline.to_ascii_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| l.contains(w));
    if has(&["drain", "stole", "steal", "theft", "siphon"]) {
        return Some(Impact::Drain);
    }
    if has(&["freeze", "frozen", "lock", "stuck", "cannot withdraw"]) {
        return Some(Impact::Freeze);
    }
    if has(&["halt", "brick", "shut down", "shutdown", "stop working"]) {
        return Some(Impact::Halt);
    }
    if has(&[
        "denial of service",
        "denial-of-service",
        " dos",
        "grief",
        "revert",
        "blocked",
    ]) {
        return Some(Impact::Dos);
    }
    if has(&[
        "mis-account",
        "miscount",
        "misaccount",
        "wrong accounting",
        "inflate",
        "double count",
    ]) {
        return Some(Impact::MisAccounting);
    }
    None
}

/// `any user`, `anyone`, `unprivileged`, or a role word in the headline.
pub fn attacker_of(text: &str, headline: &str) -> Option<Attacker> {
    let l = text.to_ascii_lowercase();
    for phrase in [
        "any user",
        "anyone can",
        "any account",
        "unprivileged",
        "without privileges",
        "no privileges",
        "no special privileges",
    ] {
        if l.contains(phrase) {
            return Some(Attacker::AnyUser);
        }
    }
    for word in identity::words(headline) {
        if identity::ROLE_WORDS.contains(&word.as_str()) || word == "owner" {
            return Some(Attacker::Role(word));
        }
    }
    None
}

/// The claims the heuristics can stand behind, and nothing else.
pub fn inferred(text: &str) -> Claims {
    let headline = headline(text).unwrap_or("");
    Claims {
        attacker: attacker_of(text, headline),
        impact: impact_of(headline),
        asset: None,
        amount: None,
        preconditions: Vec::new(),
        cited: identifiers(text),
        severity: None,
        fix: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REPORT: &str = "# Self-backed vaults switch off a stablecoin's surplus\n\n`VaultEngine.open` lets any user open a vault on a fee-exempt type.\n`BalanceSheet.distributeSurplus` reverts with `ReserveBackingShortfall`.\nRun `cargo test` and see `src/VaultEngine.sol:open`.\n";

    #[test]
    fn reads_the_headline_impact() {
        assert_eq!(impact_of("Vaults drain the reserve"), Some(Impact::Drain));
        assert_eq!(
            impact_of("Self-backed vaults switch off a stablecoin's surplus"),
            None
        );
        assert_eq!(
            impact_of("Deposits are frozen forever"),
            Some(Impact::Freeze)
        );
    }

    #[test]
    fn finds_the_attacker_claim() {
        assert_eq!(attacker_of(REPORT, ""), Some(Attacker::AnyUser));
        assert_eq!(
            attacker_of("only the admin can do it", "Admin sweep"),
            Some(Attacker::Role("admin".to_string()))
        );
        assert_eq!(attacker_of("nothing said here", "A title"), None);
    }

    #[test]
    fn keeps_code_identifiers_and_drops_commands() {
        let ids = identifiers(REPORT);
        assert!(ids.contains(&"VaultEngine.open".to_string()), "{ids:?}");
        assert!(
            ids.contains(&"src/VaultEngine.sol:open".to_string()),
            "{ids:?}"
        );
        assert!(!ids.iter().any(|i| i.starts_with("cargo")), "{ids:?}");
    }

    #[test]
    fn everything_else_stays_empty() {
        let c = inferred(REPORT);
        assert_eq!(c.attacker, Some(Attacker::AnyUser));
        assert!(c.asset.is_none() && c.amount.is_none() && c.fix.is_none());
        assert!(c.preconditions.is_empty());
    }
}
