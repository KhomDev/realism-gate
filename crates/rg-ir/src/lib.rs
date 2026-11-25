#![forbid(unsafe_code)]
//! The parser-independent model realism-gate analyses.
//!
//! The parsers borrow from an arena (solar) or a source map (syn, oxc) that
//! lives only as long as a parse session. Rules need to look at several files
//! at once, so every frontend lowers its source into these owned types and the
//! session is dropped. The model keeps only what the rules read: names, call
//! structure, assignments, literals, source text and positions.
//!
//! - [`ir`] is the Solidity model. The Solana frontends reuse its expressions
//!   and statements (`rg_ir::solana::ir`).
//! - [`solana::ir`] is the model of one Solana test file, Rust or TypeScript.
//! - [`move_lang`] is the model of one Move test module.
//! - [`finding`] is the output contract shared by every rule and renderer:
//!   [`finding::Severity`], [`finding::Finding`] and [`finding::Suppression`].

pub mod finding;
pub mod ir;
pub mod move_lang;
pub mod solana;

pub use finding::{Finding, Severity, Suppression};

/// Splits an identifier into lowercase words: `USDCWhale` -> `usdc whale`,
/// `getRoleMember` -> `get role member`, `ADMIN_ROLE` -> `admin role`.
/// Trailing digits are dropped from each word, so `user1` reads as `user`.
pub fn words(name: &str) -> Vec<String> {
    let mut out = Vec::new();
    for part in name.split(|c: char| !c.is_ascii_alphanumeric()) {
        let chars: Vec<char> = part.chars().collect();
        let mut start = 0;
        for i in 1..chars.len() {
            let (prev, cur) = (chars[i - 1], chars[i]);
            let next_lower = chars.get(i + 1).is_some_and(|c| c.is_ascii_lowercase());
            let boundary = (prev.is_ascii_lowercase() && cur.is_ascii_uppercase())
                || (prev.is_ascii_uppercase() && cur.is_ascii_uppercase() && next_lower)
                || (prev.is_ascii_digit() && cur.is_ascii_alphabetic());
            if boundary {
                out.push(chars[start..i].iter().collect::<String>());
                start = i;
            }
        }
        if start < chars.len() {
            out.push(chars[start..].iter().collect::<String>());
        }
    }
    out.into_iter()
        .map(|w| {
            w.to_ascii_lowercase()
                .trim_end_matches(|c: char| c.is_ascii_digit())
                .to_string()
        })
        .filter(|w| !w.is_empty())
        .collect()
}

/// Shortens source text for display: one line, at most `max` characters.
pub fn snippet(text: &str, max: usize) -> String {
    let one_line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.chars().count() <= max {
        one_line
    } else {
        let cut: String = one_line.chars().take(max.saturating_sub(1)).collect();
        format!("{cut}…")
    }
}

#[cfg(test)]
mod tests {
    use super::snippet;

    #[test]
    fn snippets_are_single_line_and_bounded() {
        assert_eq!(snippet("a(\n   b,\n   c)", 80), "a( b, c)");
        assert_eq!(snippet("abcdef", 4), "abc…");
    }
}
