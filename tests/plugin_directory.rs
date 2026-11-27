//! The Anthropic Directory plugin (`plugin-directory/`) must stay inside the
//! rules the Directory applies to an unverified publisher: no hooks, no
//! `bin/` installer, no shell `allowed-tools`, and a square PNG icon. The
//! full plugin in `plugin/` is a different tree and is not checked here.

use std::fs;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn lite() -> PathBuf {
    root().join("plugin-directory")
}

fn read(rel: &str) -> String {
    fs::read_to_string(root().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

fn json(rel: &str) -> serde_json::Value {
    serde_json::from_str(&read(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

fn crate_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

const ALLOWED_TOOLS: &[&str] = &["Read", "Grep", "Glob", "LS"];
const COMMANDS: &[&str] = &["check", "install", "upgrade"];

/// The Directory build must not grow a hook or an installer. Those belong
/// only in `plugin/`.
#[test]
fn the_lite_plugin_has_no_hooks_and_no_bin() {
    let root = lite();
    assert!(root.is_dir(), "plugin-directory/ must exist");
    assert!(
        !root.join("hooks").exists() && !root.join("hooks/hooks.json").exists(),
        "plugin-directory must not contain hooks/"
    );
    assert!(
        !root.join("bin").exists(),
        "plugin-directory must not contain bin/"
    );
    for entry in walk(&root) {
        let name = entry.file_name().unwrap().to_string_lossy();
        let rel = entry.strip_prefix(&root).unwrap().display().to_string();
        assert!(
            name != "hooks.json",
            "no hooks.json under plugin-directory (found {rel})"
        );
        let forbidden_ext = [".sh", ".bash", ".ps1", ".bat", ".cmd", ".exe"];
        assert!(
            forbidden_ext.iter().all(|ext| !name.ends_with(ext)),
            "plugin-directory must not ship a script or binary ({rel})"
        );
    }
}

/// Command frontmatter may list only the read-only tools the Directory
/// allows, and may not use `$` (no plugin-root expansion, no
/// shell parameters).
#[test]
fn lite_command_allowed_tools_are_read_only() {
    for name in COMMANDS {
        let rel = format!("plugin-directory/commands/{name}.md");
        let text = read(&rel);
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
        assert!(
            !tools.contains('$'),
            "{name}: allowed-tools must not contain `$`, got `{tools}`"
        );
        assert!(
            !tools.to_ascii_lowercase().contains("bash"),
            "{name}: allowed-tools must not include Bash, got `{tools}`"
        );
        for part in tools.split(',') {
            let tool = part.trim();
            assert!(
                ALLOWED_TOOLS.contains(&tool),
                "{name}: allowed-tools must be a subset of {ALLOWED_TOOLS:?}, got `{tool}` in `{tools}`"
            );
        }
        let body = text.split("---").nth(2).unwrap();
        assert!(
            body.contains("Do not start a shell")
                || body.contains("does not install")
                || body.contains("cannot run the CLI"),
            "{name}: body must tell the agent not to run the CLI itself"
        );
    }
}

/// The Directory form requires a square PNG icon, 512–2048 px on a side and
/// under 2 MiB, at `.claude-plugin/icon.png`.
#[test]
fn lite_plugin_icon_meets_directory_rules() {
    let path = lite().join(".claude-plugin/icon.png");
    let bytes = fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert!(
        bytes.len() < 2 * 1024 * 1024,
        "icon.png is {} bytes; the Directory limit is 2 MiB",
        bytes.len()
    );
    assert!(
        bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "icon.png must be a PNG"
    );
    // IHDR follows the 8-byte signature: 4-byte length, "IHDR", then width
    // and height as big-endian u32.
    assert!(bytes.len() >= 24, "icon.png is too short to have an IHDR");
    assert_eq!(&bytes[12..16], b"IHDR", "icon.png IHDR chunk");
    let width = u32::from_be_bytes(bytes[16..20].try_into().unwrap());
    let height = u32::from_be_bytes(bytes[20..24].try_into().unwrap());
    assert_eq!(
        width, height,
        "icon.png must be square, got {width}x{height}"
    );
    assert!(
        (512..=2048).contains(&width),
        "icon.png must be 512–2048 px on a side, got {width}"
    );
}

#[test]
fn lite_plugin_manifest_and_skill_are_present() {
    let want = crate_version();
    let plugin = json("plugin-directory/.claude-plugin/plugin.json");
    assert_eq!(plugin["name"], "realism-gate");
    assert_eq!(plugin["version"], want.as_str(), "lite plugin.json version");
    assert!(
        plugin.get("mcpServers").is_none(),
        "lite plugin.json must not declare mcpServers"
    );
    assert!(
        !lite().join(".mcp.json").exists(),
        "plugin-directory must not ship .mcp.json"
    );
    let skill = lite().join("skills/realism-gate/SKILL.md");
    assert!(skill.is_file(), "lite plugin must ship the skill");
    let text = fs::read_to_string(&skill).unwrap();
    assert!(
        text.contains("does not run the CLI") || text.contains("Do not start a shell"),
        "lite skill must tell the agent not to run the CLI itself"
    );
    for name in COMMANDS {
        assert!(
            lite().join(format!("commands/{name}.md")).is_file(),
            "missing plugin-directory/commands/{name}.md"
        );
    }
}

/// The marketplace may list the lite plugin as a second entry. The full
/// plugin entry, which `tests/plugin.rs` pins, must stay on `./plugin`.
#[test]
fn marketplace_lists_the_lite_plugin_without_replacing_the_full_one() {
    let want = crate_version();
    let market = json(".claude-plugin/marketplace.json");
    let plugins = market["plugins"]
        .as_array()
        .expect("marketplace.json plugins");
    let full = plugins
        .iter()
        .find(|e| e["name"] == "realism-gate" && e["source"] == "./plugin")
        .expect("marketplace still lists the full plugin at ./plugin");
    assert_eq!(full["version"], want.as_str());
    let lite = plugins
        .iter()
        .find(|e| e["source"] == "./plugin-directory")
        .expect("marketplace lists the Directory plugin at ./plugin-directory");
    assert_eq!(lite["version"], want.as_str());
    assert_ne!(
        lite["name"], "realism-gate",
        "the lite marketplace entry needs a distinct name so it does not collide with ./plugin"
    );
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(cur) = stack.pop() {
        for entry in fs::read_dir(&cur).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path.clone());
            }
            out.push(path);
        }
    }
    out
}
