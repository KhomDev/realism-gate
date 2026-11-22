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

/// Removes control characters from text that came from the files under
/// review: finding messages, evidence, paths, claim text and evidence,
/// suppression notes and parse errors. Newline and tab are kept, so wrapped
/// prose and tables still render; every other control character (including
/// ESC, which starts a terminal escape sequence) is dropped.
///
/// This runs on the user-derived field *before* any colour wrapper. Doing it
/// to the assembled output instead would strip the tool's own ANSI colour
/// codes when `--color always` is requested. JSON and SARIF are deliberately
/// untouched: serde escapes control characters there, and those formats are
/// the machine-readable record.
pub fn sanitize(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_control() || matches!(c, '\n' | '\t'))
        .collect()
}

/// The documentation page for a rule.
pub fn rule_url(id: &str) -> String {
    format!("{REPOSITORY}/blob/main/docs/rules/{id}.md")
}
