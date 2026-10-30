//! Generated documentation must match the code that generates it.
//!
//! `docs/rules/*.md` comes from the rule catalog, and the example block in
//! README.md comes from running the binary on `examples/vault-drain`. Run with
//! `REALISM_GATE_BLESS=1 cargo test --test docs` to regenerate both after a
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

const BEGIN: &str = "<!-- BEGIN:example -->";
const END: &str = "<!-- END:example -->";

#[test]
fn readme_example_is_current() {
    let out = Command::new(env!("CARGO_BIN_EXE_realism-gate"))
        .args(["check", "--color", "never", "--no-declaration"])
        .current_dir(root().join("examples/vault-drain"))
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1), "the example is meant to fail");
    let text = String::from_utf8(out.stdout).unwrap();
    let block = format!("{BEGIN}\n\n```text\n{}```\n\n{END}", text);

    let readme_path = root().join("README.md");
    let readme = norm(&std::fs::read_to_string(&readme_path).unwrap());
    let start = readme.find(BEGIN).expect("README has an example block");
    let end = readme.find(END).expect("README example block is closed") + END.len();
    let current = &readme[start..end];
    if bless() {
        let updated = format!("{}{}{}", &readme[..start], block, &readme[end..]);
        std::fs::write(&readme_path, updated).unwrap();
        return;
    }
    assert_eq!(
        current, block,
        "README example is stale; run with REALISM_GATE_BLESS=1"
    );
}
