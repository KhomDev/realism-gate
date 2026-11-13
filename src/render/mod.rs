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

/// The documentation page for a rule.
pub fn rule_url(id: &str) -> String {
    format!("{REPOSITORY}/blob/main/docs/rules/{id}.md")
}
