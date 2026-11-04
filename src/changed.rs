//! `--changed-since REF`: the files that differ from a git revision.
//!
//! The working tree is compared with `REF` (committed, staged and unstaged
//! changes alike), plus untracked files git does not ignore. Deleted files are
//! left out. Paths are relative to the working directory, with forward
//! slashes, so they compare equal to the paths in the report.
//!
//! This runs the local `git` binary. It reads the repository; it never
//! fetches. In CI the base revision must already be in the clone.

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

fn git(cwd: &Path, args: &[&str]) -> Result<Vec<String>, String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|e| format!("--changed-since needs git: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(format!("--changed-since: git {}: {}", args[0], err.trim()));
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .split('\0')
        .filter(|p| !p.is_empty())
        .map(|p| p.replace('\\', "/"))
        .collect())
}

pub fn files(git_ref: &str, cwd: &Path) -> Result<BTreeSet<String>, String> {
    if git_ref.starts_with('-') {
        return Err(format!("--changed-since: `{git_ref}` is not a revision"));
    }
    let mut out: BTreeSet<String> = git(
        cwd,
        &[
            "diff",
            "--name-only",
            "--relative",
            "--diff-filter=d",
            "-z",
            git_ref,
            "--",
        ],
    )?
    .into_iter()
    .collect();
    out.extend(git(
        cwd,
        &["ls-files", "--others", "--exclude-standard", "-z"],
    )?);
    Ok(out)
}
