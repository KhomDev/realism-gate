//! Git runs inside trees the checker does not trust: the `--target` repository
//! (`--target`, `--target-commit`) and the working directory (`--changed-since`).
//! The repository's own `.git/config` can name a program git will run:
//! `core.fsmonitor`, `core.pager`, an alias, `diff.external`, a filter driver.
//! Each test below plants one such setting that writes a marker file, runs the
//! checker, and asserts that the marker was not written. Option-looking values
//! for `--target-commit` and `--changed-since` must be refused.
//!
//! Fictional fixtures only. Each test uses its own temporary directories.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).to_string()
}

fn unique() -> u128 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "rg-security-{name}-{}-{}",
        std::process::id(),
        unique()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A path as a POSIX shell sees it: forward slashes, which Git for Windows'
/// `sh` reads, and the shell quoting is the caller's.
fn sh_path(p: &Path) -> String {
    p.display().to_string().replace('\\', "/")
}

fn git(dir: &Path, args: &[&str]) -> Output {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_AUTHOR_NAME", "realism-gate")
        .env("GIT_AUTHOR_EMAIL", "rg@example.com")
        .env("GIT_COMMITTER_NAME", "realism-gate")
        .env("GIT_COMMITTER_EMAIL", "rg@example.com")
        .output()
        .expect("git runs");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

const VAULT: &str = "contract Vault {\n    uint256 public balance = 100;\n    function sweep(address) external { balance = 0; }\n}\n";
const POC: &str = "contract Poc {\n    function test_drain() public {}\n}\n";

/// A repository with two committed files and one commit. Returns the commit.
fn repo(dir: &Path) -> String {
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::create_dir_all(dir.join("test")).unwrap();
    std::fs::write(dir.join("src/Vault.sol"), VAULT).unwrap();
    std::fs::write(dir.join("test/Poc.t.sol"), POC).unwrap();
    git(dir, &["init", "-q"]);
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", "fixture"]);
    let out = git(dir, &["rev-parse", "HEAD"]);
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// A shell command that writes `marker` and then passes stdin through, so it
/// also works as a filter driver.
fn touch(marker: &Path) -> String {
    format!("touch '{}' && cat", sh_path(marker))
}

fn realism_gate(cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_realism-gate"))
        .args(args)
        .current_dir(cwd)
        .env("NO_COLOR", "1")
        .output()
        .expect("binary runs")
}

// ---------------------------------------------------------------- --target

#[test]
fn a_target_repo_fsmonitor_does_not_run_under_target_and_target_commit() {
    let root = scratch("target-fsmonitor");
    let target = root.join("target");
    let commit = repo(&target);
    let marker = root.join("fsmonitor-ran");
    git(&target, &["config", "core.fsmonitor", &touch(&marker)]);

    let work = root.join("work");
    std::fs::create_dir_all(&work).unwrap();
    std::fs::write(work.join("Poc.t.sol"), POC).unwrap();
    let o = realism_gate(
        &work,
        &[
            "check",
            "Poc.t.sol",
            "--target",
            &sh_path(&target),
            "--target-commit",
            &commit,
            "--no-config",
            "--no-declaration",
            "--format",
            "json",
        ],
    );
    assert!(
        !marker.exists(),
        "core.fsmonitor in the target ran; stderr: {}",
        stderr(&o)
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_target_repo_pager_alias_and_external_diff_do_not_run_under_target() {
    let root = scratch("target-misc");
    let target = root.join("target");
    let commit = repo(&target);
    let pager = root.join("pager-ran");
    let alias = root.join("alias-ran");
    let external = root.join("external-ran");
    git(&target, &["config", "core.pager", &touch(&pager)]);
    git(
        &target,
        &["config", "alias.rev-parse", &format!("!{}", touch(&alias))],
    );
    git(&target, &["config", "diff.external", &touch(&external)]);

    let work = root.join("work");
    std::fs::create_dir_all(&work).unwrap();
    std::fs::write(work.join("Poc.t.sol"), POC).unwrap();
    let _ = realism_gate(
        &work,
        &[
            "check",
            "Poc.t.sol",
            "--target",
            &sh_path(&target),
            "--target-commit",
            &commit,
            "--no-config",
            "--no-declaration",
            "--format",
            "json",
        ],
    );
    assert!(!pager.exists(), "core.pager ran");
    assert!(!alias.exists(), "an alias ran");
    assert!(!external.exists(), "diff.external ran");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn target_commit_that_looks_like_an_option_is_refused() {
    let root = scratch("target-option");
    let target = root.join("target");
    let _ = repo(&target);
    let work = root.join("work");
    std::fs::create_dir_all(&work).unwrap();
    std::fs::write(work.join("Poc.t.sol"), POC).unwrap();
    let o = realism_gate(
        &work,
        &[
            "check",
            "Poc.t.sol",
            "--target",
            &sh_path(&target),
            "--target-commit=--output=x",
            "--no-config",
            "--no-declaration",
            "--format",
            "json",
        ],
    );
    assert_eq!(o.status.code(), Some(2), "{}", stderr(&o));
    assert!(
        !work.join("x").exists(),
        "an option was read as a git argument"
    );
    let _ = std::fs::remove_dir_all(&root);
}

// -------------------------------------------------------- --changed-since

#[test]
fn a_working_tree_fsmonitor_does_not_run_under_changed_since() {
    let root = scratch("cwd-fsmonitor");
    let repo_dir = root.join("repo");
    let _ = repo(&repo_dir);
    let marker = root.join("fsmonitor-ran");
    git(&repo_dir, &["config", "core.fsmonitor", &touch(&marker)]);
    // Make the index stale, so git has to refresh it.
    std::fs::write(repo_dir.join("src/Vault.sol"), format!("{VAULT}// edit\n")).unwrap();

    let o = realism_gate(
        &repo_dir,
        &[
            "check",
            "test/Poc.t.sol",
            "--changed-since",
            "HEAD",
            "--no-config",
            "--no-declaration",
            "--format",
            "json",
        ],
    );
    assert!(
        !marker.exists(),
        "core.fsmonitor in the working tree ran; stderr: {}",
        stderr(&o)
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_clean_filter_in_the_working_tree_does_not_run_under_changed_since() {
    let root = scratch("cwd-filter");
    let repo_dir = root.join("repo");
    let _ = repo(&repo_dir);
    std::fs::write(repo_dir.join(".gitattributes"), "*.sol filter=rgtest\n").unwrap();
    git(&repo_dir, &["add", ".gitattributes"]);
    git(&repo_dir, &["commit", "-q", "-m", "attributes"]);
    let marker = root.join("filter-ran");
    git(
        &repo_dir,
        &["config", "filter.rgtest.clean", &touch(&marker)],
    );
    std::fs::write(repo_dir.join("src/Vault.sol"), format!("{VAULT}// edit\n")).unwrap();

    let o = realism_gate(
        &repo_dir,
        &[
            "check",
            "test/Poc.t.sol",
            "--changed-since",
            "HEAD",
            "--no-config",
            "--no-declaration",
            "--format",
            "json",
        ],
    );
    assert!(
        !marker.exists(),
        "a clean filter in the working tree ran; stderr: {}",
        stderr(&o)
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_diff_external_in_the_working_tree_does_not_run_under_changed_since() {
    let root = scratch("cwd-external");
    let repo_dir = root.join("repo");
    let _ = repo(&repo_dir);
    let marker = root.join("external-ran");
    git(&repo_dir, &["config", "diff.external", &touch(&marker)]);
    std::fs::write(repo_dir.join("src/Vault.sol"), format!("{VAULT}// edit\n")).unwrap();
    let _ = realism_gate(
        &repo_dir,
        &[
            "check",
            "test/Poc.t.sol",
            "--changed-since",
            "HEAD",
            "--no-config",
            "--no-declaration",
            "--format",
            "json",
        ],
    );
    assert!(!marker.exists(), "diff.external ran");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_changed_since_ref_that_looks_like_an_option_is_refused() {
    let root = scratch("cwd-option");
    let repo_dir = root.join("repo");
    let _ = repo(&repo_dir);
    let o = realism_gate(
        &repo_dir,
        &[
            "check",
            "test/Poc.t.sol",
            "--changed-since=--output=x",
            "--no-config",
            "--no-declaration",
            "--format",
            "json",
        ],
    );
    assert_eq!(o.status.code(), Some(2), "{}", stderr(&o));
    assert!(
        !repo_dir.join("x").exists(),
        "an option was read as a git argument"
    );
    let _ = std::fs::remove_dir_all(&root);
}
