//! `--changed-since REF`: the files that differ from a git revision.
//!
//! The working tree is compared with `REF` (committed, staged and unstaged
//! changes alike), plus untracked files git does not ignore. Deleted files are
//! left out. Paths are relative to the working directory, with forward
//! slashes, so they compare equal to the paths in the report.
//!
//! This runs the local `git` binary. It reads the repository; it never
//! fetches. In CI the base revision must already be in the clone.

//! The repository is untrusted, so every call goes through `crate::git`, which
//! turns off `core.fsmonitor`. The worktree comparison also reads an empty
//! attribute source (`--attr-source`, git 2.40 or later), so a `.gitattributes`
//! cannot name a clean filter for git to run. This may list a file that differs
//! only in line endings, which is the safe direction: it is checked, not missed.
//! `tests/security.rs` plants both programs and asserts that neither runs.

use std::collections::BTreeSet;
use std::path::Path;

/// `global` options come before the subcommand, where git reads them.
fn git(cwd: &Path, global: &[&str], sub: &str, args: &[&str]) -> Result<Vec<String>, String> {
    let out = crate::git::command(cwd)
        .args(global)
        .arg(sub)
        .args(args)
        .output()
        .map_err(|e| format!("--changed-since needs git: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(format!("--changed-since: git {sub}: {}", err.trim()));
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
    let empty = crate::git::empty_tree(cwd)?;
    let attr_source = format!("--attr-source={empty}");
    let mut out: BTreeSet<String> = git(
        cwd,
        &[&attr_source],
        "diff",
        &[
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
        &[],
        "ls-files",
        &["--others", "--exclude-standard", "-z"],
    )?);
    Ok(out)
}
