//! Inline suppressions:
//!
//! ```solidity
//! // realism-gate: allow(RG04) the slot is the attacker's own token balance
//! vm.store(address(token), slot, bytes32(amount));
//! ```
//!
//! The parentheses are optional: `realism-gate: allow RG04 <reason>` and
//! `realism-gate: allow RG04, RG06 <reason>` mean the same thing.
//!
//! A directive applies to its own line and the line below it. It must give a
//! reason; a bare `allow(RG04)` is ignored and the finding stays active, with a
//! note in the output saying why.

use regex::Regex;
use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Directive {
    pub line: u32,
    pub rules: Vec<String>,
    pub reason: String,
}

fn pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i:(?://|/\*|^\s*\*)\s*realism-gate:\s*allow)(?:\(([^)]*)\)|\s+((?i:RG\d{2})(?:\s*,\s*(?i:RG\d{2}))*)\b)(.*)",
        )
        .expect("valid")
    })
}

/// Every directive in a source file.
pub fn directives(source: &str) -> Vec<Directive> {
    let mut out = Vec::new();
    for (i, line) in source.lines().enumerate() {
        let Some(caps) = pattern().captures(line) else {
            continue;
        };
        let list = caps
            .get(1)
            .or_else(|| caps.get(2))
            .map_or("", |m| m.as_str());
        let rules = list
            .split(',')
            .map(|r| r.trim().to_ascii_uppercase())
            .filter(|r| !r.is_empty())
            .collect();
        let reason = caps[3]
            .trim()
            .trim_end_matches("*/")
            .trim()
            .trim_start_matches(['-', ':', '\u{2014}', '\u{2013}'])
            .trim()
            .to_string();
        out.push(Directive {
            line: i as u32 + 1,
            rules,
            reason,
        });
    }
    out
}

/// The directive that applies to a finding of `rule` at `line`, if any.
pub fn find<'a>(directives: &'a [Directive], rule: &str, line: u32) -> Option<&'a Directive> {
    directives
        .iter()
        .find(|d| (d.line == line || d.line + 1 == line) && d.rules.iter().any(|r| r == rule))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rules_and_reason() {
        let src = "a();\n// realism-gate: allow(RG04, rg06) -- funding the attacker\nb();";
        let d = directives(src);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].line, 2);
        assert_eq!(d[0].rules, vec!["RG04", "RG06"]);
        assert_eq!(d[0].reason, "funding the attacker");
        assert!(find(&d, "RG04", 3).is_some());
        assert!(find(&d, "RG04", 2).is_some());
        assert!(find(&d, "RG04", 4).is_none());
        assert!(find(&d, "RG05", 3).is_none());
    }

    #[test]
    fn a_bare_directive_has_an_empty_reason() {
        let d = directives("x(); // realism-gate: allow(RG11)");
        assert_eq!(d[0].reason, "");
    }

    #[test]
    fn text_outside_comments_is_not_a_directive() {
        assert!(directives("string s = \"realism-gate: allow(RG01) x\";").is_empty());
    }

    #[test]
    fn parentheses_are_optional() {
        let d = directives("// realism-gate: allow RG04, rg06 funding the attacker\nb();");
        assert_eq!(d[0].rules, vec!["RG04", "RG06"]);
        assert_eq!(d[0].reason, "funding the attacker");
        let d = directives("x(); // realism-gate: allow RG11");
        assert_eq!(d[0].rules, vec!["RG11"]);
        assert_eq!(d[0].reason, "");
    }

    #[test]
    fn a_word_after_allow_is_not_a_rule() {
        assert!(directives("// realism-gate: allow everything").is_empty());
    }

    #[test]
    fn block_comments_work() {
        let d = directives("/* realism-gate: allow(RG07) gas oracle only */");
        assert_eq!(d[0].reason, "gas oracle only");
    }
}
