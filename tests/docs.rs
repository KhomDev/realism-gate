//! Generated documentation must match the code that generates it.
//!
//! `docs/rules/*.md` comes from the rule catalog, and the three example
//! blocks in README.md come from running the binary on `examples/vault-drain`
//! (Foundry), `examples/solana-vault-drain` (Rust) and
//! `examples/anchor-vault-drain` (Anchor TypeScript). Run with
//! `REALISM_GATE_BLESS=1 cargo test --test docs` to regenerate them after a
//! deliberate change.

use realism_gate::rules::{RULES, catalog};
use std::path::PathBuf;
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn bless() -> bool {
    std::env::var_os("REALISM_GATE_BLESS").is_some()
}

/// Normalises line endings so a Windows checkout compares equal.
fn norm(s: &str) -> String {
    s.replace("\r\n", "\n")
}

#[test]
fn rule_pages_are_current() {
    let dir = root().join("docs/rules");
    for r in RULES {
        let path = dir.join(format!("{}.md", r.id));
        let want = catalog::markdown(r);
        if bless() {
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(&path, &want).unwrap();
            continue;
        }
        let have = std::fs::read_to_string(&path).unwrap_or_else(|_| {
            panic!(
                "{} is missing; run with REALISM_GATE_BLESS=1",
                path.display()
            )
        });
        assert_eq!(
            norm(&have),
            want,
            "{} is stale; run with REALISM_GATE_BLESS=1",
            path.display()
        );
    }
    let pages = std::fs::read_dir(&dir).unwrap().count();
    assert_eq!(
        pages,
        RULES.len(),
        "docs/rules has pages for rules that do not exist"
    );
}

/// Replaces the block between `<!-- BEGIN:{name} -->` and `<!-- END:{name} -->`
/// in README.md with the output of `realism-gate check` run in `dir`, or
/// checks that it already holds that output.
fn readme_block_is_current(name: &str, dir: &str) {
    let (begin, end_mark) = (
        format!("<!-- BEGIN:{name} -->"),
        format!("<!-- END:{name} -->"),
    );
    let out = Command::new(env!("CARGO_BIN_EXE_realism-gate"))
        .args(["check", "--color", "never", "--no-declaration"])
        .current_dir(root().join(dir))
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1), "{dir} is meant to fail");
    let text = String::from_utf8(out.stdout).unwrap();
    let block = format!("{begin}\n\n```text\n{text}```\n\n{end_mark}");

    let readme_path = root().join("README.md");
    let readme = norm(&std::fs::read_to_string(&readme_path).unwrap());
    let start = readme
        .find(&begin)
        .unwrap_or_else(|| panic!("README has no {name} block"));
    let end = readme
        .find(&end_mark)
        .unwrap_or_else(|| panic!("README's {name} block is not closed"))
        + end_mark.len();
    let current = &readme[start..end];
    if bless() {
        let updated = format!("{}{}{}", &readme[..start], block, &readme[end..]);
        std::fs::write(&readme_path, updated).unwrap();
        return;
    }
    assert_eq!(
        current, block,
        "README {name} block is stale; run with REALISM_GATE_BLESS=1"
    );
}

#[test]
fn readme_examples_are_current() {
    // One at a time: each rewrites README.md when blessing.
    readme_block_is_current("example", "examples/vault-drain");
    readme_block_is_current("example-rust", "examples/solana-vault-drain");
    readme_block_is_current("example-anchor", "examples/anchor-vault-drain");
}

/// `uses: KhomDev/realism-gate@v0` must download the release it belongs to,
/// so the action's default version moves with the crate version.
#[test]
fn action_default_version_is_the_crate_version() {
    let action = std::fs::read_to_string(root().join("action.yml")).unwrap();
    let want = format!("RG_DEFAULT_VERSION: v{}", env!("CARGO_PKG_VERSION"));
    assert!(
        action.lines().any(|l| l.trim() == want),
        "action.yml must set `{want}`"
    );
}
