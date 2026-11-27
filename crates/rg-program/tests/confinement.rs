//! The program model reads only inside the target it is given.

use rg_program::{ProgramModel, TargetRoot};
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn a_parent_directory_escape_is_refused() {
    let root = TargetRoot::new(&fixture("anchor-signer-direct")).unwrap();
    for escaped in [
        "../sol-only-owner/src/Vault.sol",
        "src/../../anchor-signer-direct/src/lib.rs",
        "..",
        "",
    ] {
        let error = root
            .read(escaped)
            .expect_err(&format!("{escaped:?} must be refused"));
        assert!(
            error.to_string().contains("outside the target root")
                || error.to_string().contains("no such file"),
            "{escaped:?}: {error}"
        );
    }
}

#[test]
fn an_absolute_path_outside_the_target_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let inside = tmp.path().join("inside");
    std::fs::create_dir_all(inside.join("src")).unwrap();
    std::fs::write(
        inside.join("src/lib.rs"),
        "contract Safe { function open() external {} }",
    )
    .unwrap();
    let outside = tmp.path().join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(
        outside.join("Secret.sol"),
        "contract Secret { function steal() external onlyOwner {} }",
    )
    .unwrap();

    let root = TargetRoot::new(&inside).unwrap();
    let absolute = outside.join("Secret.sol");
    let error = root
        .resolve(&absolute.to_string_lossy())
        .expect_err("an absolute path outside the root must be refused");
    assert!(
        error.to_string().contains("outside the target root"),
        "{error}"
    );
    let error = root
        .resolve(&inside.join("src/lib.rs").to_string_lossy())
        .expect_err("the model's inputs are relative paths");
    assert!(
        error.to_string().contains("outside the target root"),
        "{error}"
    );
}

#[test]
fn a_symlink_out_of_the_target_is_not_read() {
    let tmp = tempfile::tempdir().unwrap();
    let inside = tmp.path().join("inside");
    std::fs::create_dir_all(inside.join("src")).unwrap();
    std::fs::write(
        inside.join("src/Safe.sol"),
        "contract Safe { function open() external {} }",
    )
    .unwrap();
    let outside = tmp.path().join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(
        outside.join("Secret.sol"),
        "contract Secret { function steal() external onlyOwner {} }",
    )
    .unwrap();
    let link = inside.join("src/Secret.sol");
    if !make_file_symlink(&outside.join("Secret.sol"), &link) {
        eprintln!("symlinks cannot be created here; the symlink case is skipped");
        return;
    }

    let root = TargetRoot::new(&inside).unwrap();
    assert!(
        root.read("src/Secret.sol").is_err(),
        "a symlink out of the root must be refused"
    );
    let model = ProgramModel::resolve(&inside).unwrap();
    assert!(
        model.entry_point("Secret", "steal").is_none(),
        "the model followed a symlink out of the target"
    );
    assert!(model.entry_point("Safe", "open").is_some());
}

#[test]
fn no_serialised_output_carries_the_host_path() {
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("checkout");
    std::fs::create_dir_all(target.join("src")).unwrap();
    std::fs::write(
        target.join("src/Vault.sol"),
        "contract Vault { function open() external {} }",
    )
    .unwrap();
    let model = ProgramModel::resolve(&target).unwrap();
    let json = model.to_json_pretty();
    let host = target.to_string_lossy().to_string();
    assert!(
        !json.contains(&host),
        "the model serialised the absolute target path"
    );
    assert!(
        !json.contains("\\\\?\\"),
        "the model serialised a verbatim path"
    );
}

fn make_file_symlink(target: &Path, link: &Path) -> bool {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link).is_ok()
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_file(target, link).is_ok()
    }
}
