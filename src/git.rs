//! Every `git` call the checker makes goes through here.
//!
//! The repository under review and the `--target` tree are untrusted. Their
//! own `.git/config` can name a program that git runs: `core.fsmonitor` runs
//! on an index refresh, and a clean filter runs when git compares the worktree
//! with a commit. `core.fsmonitor` is turned off here. Clean filters are
//! avoided by never asking git to compare the worktree (see `changed.rs`).
//! `tests/security.rs` plants each one and asserts that it does not run.
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
