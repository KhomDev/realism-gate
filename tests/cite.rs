//! RG20 reads a target checkout at one commit. The working tree is not trusted.

use realism_gate::cite;
use realism_gate::engine::{Options, analyze};
use realism_gate::project::{Project, discover};
use std::path::PathBuf;
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn git(dir: &PathBuf, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_AUTHOR_NAME", "realism-gate")
        .env("GIT_AUTHOR_EMAIL", "rg@example.com")
        .env("GIT_COMMITTER_NAME", "realism-gate")
        .env("GIT_COMMITTER_EMAIL", "rg@example.com")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn a_missing_contract_and_function_fail_and_a_real_one_does_not() {
    let dir = std::env::temp_dir().join(format!("rg-cite-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join("src/Vault.sol"),
        "contract Vault {\n    function withdraw(uint256) external {}\n}\n",
    )
    .unwrap();
    git(&dir, &["init", "-q"]);
    git(&dir, &["add", "src/Vault.sol"]);
    git(&dir, &["commit", "-q", "-m", "vault"]);
    let rev = String::from_utf8(
        Command::new("git")
            .arg("-C")
            .arg(&dir)
            .args(["rev-parse", "HEAD"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    let rev = rev.trim();
    let index = cite::load(&dir, rev).unwrap();
    assert!(index.contracts.contains_key("Vault"));
    assert!(index.functions.contains("withdraw"));

    let base = root();
    let poc = base.join("tests/fixtures/bad/RG20_missing_citation.t.sol");
    let project = Project::load(&discover(&[poc]).unwrap(), &base).unwrap();
    let report = analyze(
        &project,
        None,
        &Options {
            citations: Some(index),
            ..Options::default()
        },
    );
    let rules: Vec<&str> = report.active().map(|f| f.rule).collect();
    assert!(
        rules.contains(&"RG20"),
        "missing citation should fail, got {rules:?} {:?}",
        report
            .active()
            .map(|f| f.message.clone())
            .collect::<Vec<_>>()
    );
    assert!(report.active().any(|f| f.message.contains("MissingVault")
        || f.message.contains("src/MissingVault.sol")
        || f.message.contains("inventedDrain")));

    // A PoC that cites the contract and function that exist is not RG20.
    let src = r#"
        import {Vault} from "src/Vault.sol";
        contract T {
            Vault vault;
            function test_ok() public {
                uint256 before = vault.balanceOf(address(this));
                vault.withdraw(1);
                assertEq(vault.balanceOf(address(this)), before);
            }
        }
    "#;
    let project = Project::from_sources(&[("test/Ok.t.sol", src)]);
    let index = cite::load(&dir, rev).unwrap();
    let report = analyze(
        &project,
        None,
        &Options {
            citations: Some(index),
            ..Options::default()
        },
    );
    assert!(
        report.active().all(|f| f.rule != "RG20"),
        "{:?}",
        report
            .active()
            .map(|f| f.message.clone())
            .collect::<Vec<_>>()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_unreadable_commit_is_an_error() {
    let dir = std::env::temp_dir().join(format!("rg-cite-bad-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    git(&dir, &["init", "-q"]);
    let err = cite::load(&dir, "0123456789abcdef0123456789abcdef01234567").unwrap_err();
    assert!(!err.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}
