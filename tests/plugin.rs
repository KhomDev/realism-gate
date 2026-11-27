//! The Claude Code plugin (`plugin/`) is linked to the crate: its version is the
//! crates.io release it installs, so the plugin, the marketplace entry and
//! `Cargo.toml` must carry the same version, and the plugin's skill must be the
//! repository's skill byte for byte. A release that bumps one without the
//! others fails here.

use std::path::Path;

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(root().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

fn json(rel: &str) -> serde_json::Value {
    serde_json::from_str(&read(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

/// The version cargo compiled this crate with (workspace-inherited or not):
/// the crates.io release a plugin of the same version installs.
fn crate_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[test]
fn plugin_version_is_the_crate_version() {
    let want = crate_version();
    let plugin = json("plugin/.claude-plugin/plugin.json");
    assert_eq!(plugin["version"], want.as_str(), "plugin.json version");
    assert_eq!(plugin["name"], "realism-gate");
    let market = json(".claude-plugin/marketplace.json");
    let entry = market["plugins"]
        .as_array()
        .and_then(|p| p.iter().find(|e| e["name"] == "realism-gate"))
        .expect("marketplace lists the realism-gate plugin");
    assert_eq!(entry["version"], want.as_str(), "marketplace.json version");
    assert_eq!(entry["source"], "./plugin");
}

#[test]
fn plugin_skill_is_the_repository_skill() {
    assert_eq!(
        read("plugin/skills/realism-gate/SKILL.md"),
        read("skills/realism-gate/SKILL.md"),
        "copy skills/realism-gate/SKILL.md into plugin/skills/realism-gate/ when it changes"
    );
}

#[test]
fn the_installer_pins_the_plugin_version_and_only_uses_cargo() {
    let script = read("plugin/bin/realism-gate-ensure");
    assert!(
        script.contains(".claude-plugin/plugin.json"),
        "reads its version from the manifest"
    );
    assert!(script.contains("cargo install --locked --force realism-gate --version"));
    assert!(
        script.contains("--status"),
        "has a report-only mode for the SessionStart hook"
    );
    let hooks = json("plugin/hooks/hooks.json");
    let cmd = hooks["hooks"]["SessionStart"][0]["hooks"][0]["command"]
        .as_str()
        .expect("SessionStart command");
    assert!(cmd.ends_with("--status"), "the hook never installs: {cmd}");
}
