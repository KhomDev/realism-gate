//! `--changed-since REF`: the files that differ from a git revision.
//!
//! The working tree is compared with `REF` (committed, staged and unstaged
//! changes alike), plus untracked files git does not ignore. Deleted files are
//! left out. Paths are relative to the working directory, with forward
//! slashes, so they compare equal to the paths in the report.
//!
//! This runs the local `git` binary. It reads the repository; it never
//! fetches. In CI the base revision must already be in the clone.
//!
//! The repository is untrusted. Its own config can name programs git runs
//! when it reads the worktree: `core.fsmonitor` on an index refresh, and a
//! clean filter, selected by `.gitattributes` *or* `.git/info/attributes`,
//! when it compares a file with a commit. So git is never asked to compare
//! the worktree. It lists the blob ids at `REF` (`ls-tree`, which reads
//! objects only) and the candidate paths (`ls-files`), and it hashes the
//! current files with `hash-object --no-filters`, which applies no filter.
//! The ids are compared here. Every call goes through `crate::git`, which
//! also turns `core.fsmonitor` off. `tests/security.rs` plants each program
//! and asserts that none runs.
//!
//! Without filters a file is hashed as its raw bytes, so a file that differs
//! from `REF` only in line endings is listed. That is the safe direction: it is
//! checked, not missed. A symbolic link or other non-regular file in the
//! worktree is listed without being read.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::Path;
use std::process::Stdio;

fn git(cwd: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let out = crate::git::command(cwd)
        .args(args)
        .output()
        .map_err(|e| format!("--changed-since needs git: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(format!("--changed-since: git {}: {}", args[0], err.trim()));
    }
    Ok(out.stdout)
}

fn nul_separated(bytes: &[u8]) -> impl Iterator<Item = String> + '_ {
    bytes
        .split(|b| *b == 0)
        .filter(|p| !p.is_empty())
        .map(|p| String::from_utf8_lossy(p).into_owned())
}

pub fn files(git_ref: &str, cwd: &Path) -> Result<BTreeSet<String>, String> {
    if git_ref.starts_with('-') {
        return Err(format!("--changed-since: `{git_ref}` is not a revision"));
    }
    let commit = git(
        cwd,
        &["rev-parse", "--verify", &format!("{git_ref}^{{commit}}")],
    )?;
    let commit = String::from_utf8_lossy(&commit).trim().to_string();

    // Blob ids at REF, under the working directory, paths relative to it.
    let mut at_ref: BTreeMap<String, String> = BTreeMap::new();
    for line in nul_separated(&git(cwd, &["ls-tree", "-r", "-z", &commit, "--", "."])?) {
        // "<mode> SP <type> SP <id> TAB <path>"
        let Some((meta, path)) = line.split_once('\t') else {
            continue;
        };
        let mut parts = meta.split(' ');
        let (_mode, kind, id) = (parts.next(), parts.next(), parts.next());
        if kind == Some("blob")
            && let Some(id) = id
        {
            at_ref.insert(path.replace('\\', "/"), id.to_string());
        }
    }

    // Tracked and untracked (not ignored) paths in the working tree.
    let candidates: BTreeSet<String> = nul_separated(&git(
        cwd,
        &[
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
            "--",
            ".",
        ],
    )?)
    .map(|p| p.replace('\\', "/"))
    .collect();

    let mut changed = BTreeSet::new();
    let mut to_hash = Vec::new();
    for path in candidates {
        let Ok(meta) = std::fs::symlink_metadata(cwd.join(&path)) else {
            continue; // deleted in the working tree: left out
        };
        if !meta.is_file() || path.contains('\n') || !at_ref.contains_key(&path) {
            // Not a regular file, a name hash-object cannot take on a line,
            // or new since REF: listed without reading it.
            changed.insert(path);
        } else {
            to_hash.push(path);
        }
    }

    for (path, id) in to_hash.iter().zip(hash_without_filters(cwd, &to_hash)?) {
        if at_ref.get(path) != Some(&id) {
            changed.insert(path.clone());
        }
    }
    Ok(changed)
}

/// `git hash-object --no-filters --stdin-paths`: one id per path, in order.
fn hash_without_filters(cwd: &Path, paths: &[String]) -> Result<Vec<String>, String> {
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    let mut child = crate::git::command(cwd)
        .args(["hash-object", "--no-filters", "--stdin-paths"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("--changed-since needs git: {e}"))?;
    let mut stdin = child.stdin.take().expect("stdin is piped");
    let input: String = paths.iter().map(|p| format!("{p}\n")).collect();
    // Written from a thread, so a full stdout pipe cannot deadlock the two.
    let writer = std::thread::spawn(move || stdin.write_all(input.as_bytes()));
    let out = child
        .wait_with_output()
        .map_err(|e| format!("--changed-since: git hash-object: {e}"))?;
    let _ = writer.join();
    if !out.status.success() {
        return Err(format!(
            "--changed-since: git hash-object: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let ids: Vec<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| l.trim().to_string())
        .collect();
    if ids.len() != paths.len() {
        return Err(format!(
            "--changed-since: git hash-object returned {} ids for {} files",
            ids.len(),
            paths.len()
        ));
    }
    Ok(ids)
}
