//! Every `git` call the checker makes goes through here.
//!
//! The repository under review and the `--target` tree are untrusted. Their
//! own `.git/config` can name a program that git runs: `core.fsmonitor` runs
//! on an index refresh, and a clean filter runs when git compares the worktree
//! with a commit. `tests/security.rs` plants each one and asserts that it does
//! not run. The options here are the ones the checker can set.
//!
//! What is not changed, on purpose: the system and global config are not the
//! repository's, so they are left as the user has them (a global
//! `safe.directory` is needed to read some checkouts at all).

use std::path::Path;
use std::process::{Command, Stdio};

/// `git -C DIR` with `core.fsmonitor` off and no stdin, so no call can wait
/// for input.
pub(crate) fn command(dir: &Path) -> Command {
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(dir)
        .args(["-c", "core.fsmonitor=false"])
        .stdin(Stdio::null());
    cmd
}

/// The id of the empty tree, in the repository's own hash algorithm. Passed
/// as `--attr-source`, it makes git read no `.gitattributes`, so no `filter`
/// attribute can select a clean filter. Computed, not written: this does not
/// add an object to the repository.
pub(crate) fn empty_tree(dir: &Path) -> Result<String, String> {
    let out = command(dir)
        .args(["hash-object", "-t", "tree", "--stdin"])
        .output()
        .map_err(|e| format!("--changed-since needs git: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "--changed-since: could not compute the empty tree: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let id = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("--changed-since: git printed no tree id".into());
    }
    Ok(id)
}
