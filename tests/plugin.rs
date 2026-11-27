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
    assert_eq!(
        cmd, "bash \"${CLAUDE_PLUGIN_ROOT}/bin/realism-gate-ensure\" --status",
        "shell-form hook: quoted plugin root, report-only"
    );
}

/// Command files take the same frontmatter as skills. Current Claude Code docs
/// substitute `${CLAUDE_PLUGIN_ROOT}` in a plugin skill's markdown body and in
/// its `allowed-tools` Bash rules, and `:*` at the end of a rule is the same
/// trailing wildcard as ` *` (it is only a wildcard at the end). The body and
/// the rule must name the same path, so the expanded rule matches the command
/// the body tells Claude to run.
#[test]
fn command_allowed_tools_expand_the_plugin_root() {
    for name in ["check", "install", "upgrade"] {
        let text = read(&format!("plugin/commands/{name}.md"));
        let front = text
            .split_once("---")
            .and_then(|(_, rest)| rest.split_once("---"))
            .map(|(front, _)| front)
            .unwrap_or_else(|| panic!("{name}: no frontmatter"));
        let tools = front
            .lines()
            .find_map(|line| line.strip_prefix("allowed-tools:"))
            .unwrap_or_else(|| panic!("{name}: no allowed-tools"))
            .trim();
        let rule = "Bash(${CLAUDE_PLUGIN_ROOT}/bin/realism-gate-ensure:*)";
        assert!(
            tools.split(',').any(|part| part.trim() == rule),
            "{name}: allowed-tools must be the expandable trailing-wildcard form `{rule}`, got `{tools}`"
        );
        assert!(
            !tools.contains("Bash(${CLAUDE_PLUGIN_ROOT}/bin/realism-gate-ensure:* ")
                && !tools.contains("Bash(${CLAUDE_PLUGIN_ROOT}/*:"),
            "{name}: `:*` is only a wildcard at the end of a pattern"
        );
        let body = text.split("---").nth(2).unwrap();
        assert!(
            body.contains("${CLAUDE_PLUGIN_ROOT}/bin/realism-gate-ensure"),
            "{name}: body must tell Claude to run the same path the rule allows"
        );
    }
}

/// Claude Code connects a plugin MCP server at session start. The ensure
/// script installs only when a command asks it to, and its stdout is a path,
/// not MCP framing. See `plugin/README.md`.
#[test]
fn the_plugin_does_not_declare_an_mcp_server() {
    assert!(
        !root().join("plugin/.mcp.json").exists(),
        "do not add plugin/.mcp.json unless the server can start without installing and without writing on stdout"
    );
    let plugin = json("plugin/.claude-plugin/plugin.json");
    assert!(
        plugin.get("mcpServers").is_none(),
        "plugin.json must not declare mcpServers"
    );
}
