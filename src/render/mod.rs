//! Output formats. JSON and SARIF are the stable, machine-readable interface;
//! the text and Markdown formats are for people and may change between
//! releases.

pub mod card;
pub mod json;
pub mod markdown;
pub mod sarif;
mod sha256;
pub mod text;

pub const TOOL: &str = "realism-gate";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const REPOSITORY: &str = "https://github.com/KhomDev/realism-gate";

/// Makes text from the files under review safe to place on one rendered
/// line: finding messages, evidence, paths, suite and test names, claim text
/// and evidence, suppression notes and parse errors.
///
/// * Newline, carriage return and tab become a single space. A Linux file
///   name can contain a newline, and a path is printed directly, so without
///   this a file named `x.sol\n  ERROR RG01 ...` could fabricate a report
///   line.
/// * Every other Unicode control character is dropped (ESC starts a terminal
///   escape sequence).
/// * The bidi embedding/override controls U+202A-U+202E and U+2066-U+2069,
///   and the line/paragraph separators U+2028/U+2029, are dropped or turned
///   into a space as well; they can reorder or break a rendered line.
///
/// The prose renderers wrap with `split_whitespace`, so no user-derived
/// newline is needed. This runs on the field *before* any colour wrapper:
/// doing it to the assembled output would strip the tool's own ANSI colour
/// codes when `--color always` is requested. JSON and SARIF are deliberately
/// untouched: serde escapes those characters there, and those formats are the
/// machine-readable record.
pub fn sanitize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        let whitespace = matches!(c, '\n' | '\r' | '\t' | '\u{2028}' | '\u{2029}');
        let bidi = matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}');
        if whitespace {
            out.push(' ');
        } else if !c.is_control() && !bidi {
            out.push(c);
        }
    }
    out
}

/// The documentation page for a rule.
pub fn rule_url(id: &str) -> String {
    format!("{REPOSITORY}/blob/main/docs/rules/{id}.md")
}
