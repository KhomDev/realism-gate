//! Malformed and hostile inputs are refused or noted, never guessed from.

use rg_program::{MAX_SOURCE_FILE_BYTES, ProgramError, ProgramModel};
use std::io::Write;
use std::path::Path;

fn target_with(files: &[(&str, &[u8])]) -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    for (relative, bytes) in files {
        let path = tmp.path().join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, bytes).unwrap();
    }
    tmp
}

#[test]
fn a_truncated_rust_program_is_an_error_not_a_panic() {
    let tmp = target_with(&[(
        "src/lib.rs",
        b"#[program]\npub mod vault {\n    pub fn open(ctx: Context<Open>",
    )]);
    let error = ProgramModel::resolve(tmp.path()).unwrap_err();
    assert!(error.to_string().contains("no Anchor program"), "{error}");
    assert!(matches!(error, ProgramError::Unsupported(_)));
}

#[test]
fn a_truncated_solidity_file_is_noted_beside_the_good_contract() {
    let tmp = target_with(&[
        (
            "src/Good.sol",
            b"pragma solidity ^0.8.20;\ncontract Good { function open() external {} }\n",
        ),
        (
            "src/Bad.sol",
            b"pragma solidity ^0.8.20;\ncontract Bad { function broken() external {",
        ),
    ]);
    let model = ProgramModel::resolve(tmp.path()).unwrap();
    assert!(model.entry_point("Good", "open").is_some());
    assert!(
        model
            .notes
            .iter()
            .any(|note| note.contains("Bad.sol") && note.contains("did not parse")),
        "{:?}",
        model.notes
    );
}

#[test]
fn a_file_over_the_per_file_cap_is_skipped_by_name() {
    let huge = MAX_SOURCE_FILE_BYTES + 1;
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join("src")).unwrap();
    std::fs::write(
        tmp.path().join("src/Good.sol"),
        "contract Good { function open() external {} }",
    )
    .unwrap();
    let file = std::fs::File::create(tmp.path().join("src/huge.rs")).unwrap();
    let mut writer = std::io::BufWriter::new(file);
    // A comment is enough: the size check refuses the file before it is read.
    writer.write_all(b"//").unwrap();
    let chunk = vec![b'x'; 64 * 1024];
    let mut written = 2u64;
    while written < huge {
        let take = std::cmp::min(chunk.len() as u64, huge - written) as usize;
        writer.write_all(&chunk[..take]).unwrap();
        written += take as u64;
    }
    writer.flush().unwrap();
    drop(writer);

    let model = ProgramModel::resolve(tmp.path()).unwrap();
    assert!(model.entry_point("Good", "open").is_some());
    assert!(
        model.notes.iter().any(|note| note.contains("src/huge.rs")
            && note.contains(&MAX_SOURCE_FILE_BYTES.to_string())
            && note.contains("per-file limit")),
        "{:?}",
        model.notes
    );
}

#[test]
fn a_file_at_the_per_file_cap_is_read() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join("src")).unwrap();
    std::fs::write(
        tmp.path().join("src/Good.sol"),
        "contract Good { function open() external {} }",
    )
    .unwrap();
    // Exactly the cap: a comment of MAX-3 bytes plus `//` and a newline.
    let mut bytes = Vec::with_capacity(MAX_SOURCE_FILE_BYTES as usize);
    bytes.extend_from_slice(b"//");
    bytes.extend(std::iter::repeat_n(
        b'x',
        MAX_SOURCE_FILE_BYTES as usize - 3,
    ));
    bytes.push(b'\n');
    assert_eq!(bytes.len() as u64, MAX_SOURCE_FILE_BYTES);
    std::fs::write(tmp.path().join("src/at_cap.rs"), &bytes).unwrap();

    let model = ProgramModel::resolve(tmp.path()).unwrap();
    assert!(model.entry_point("Good", "open").is_some());
    assert!(
        !model.notes.iter().any(|note| note.contains("at_cap.rs")),
        "a file exactly at the cap was skipped: {:?}",
        model.notes
    );
}

#[test]
fn a_mixed_language_file_is_noted_and_does_not_hide_the_solidity() {
    let tmp = target_with(&[
        (
            "src/Vault.sol",
            b"pragma solidity ^0.8.20;\ncontract Vault { function open() external {} }\n",
        ),
        // Solidity text under a Rust extension: syn refuses it.
        (
            "src/lib.rs",
            b"contract Wrong { function open() external {} }\n",
        ),
    ]);
    let model = ProgramModel::resolve(tmp.path()).unwrap();
    assert!(model.entry_point("Vault", "open").is_some());
    assert!(
        model
            .notes
            .iter()
            .any(|note| note.contains("src/lib.rs") && note.contains("did not parse")),
        "{:?}",
        model.notes
    );
}

#[test]
fn a_non_utf8_source_is_refused_by_name_not_decoded_lossily() {
    let tmp = target_with(&[
        (
            "src/Good.sol",
            b"contract Good { function open() external {} }",
        ),
        ("src/Bad.sol", b"contract Bad { // \xff\xfe not utf-8\n}\n"),
    ]);
    let model = ProgramModel::resolve(tmp.path()).unwrap();
    assert!(model.entry_point("Good", "open").is_some());
    assert!(
        model
            .notes
            .iter()
            .any(|note| note.contains("src/Bad.sol") && note.contains("not valid UTF-8")),
        "{:?}",
        model.notes
    );
}

#[test]
fn a_target_whose_only_source_is_not_utf8_is_a_clean_error() {
    let tmp = target_with(&[("src/Bad.sol", b"contract Bad { // \xff\n}\n")]);
    assert!(ProgramModel::resolve(tmp.path()).is_err());
}

#[test]
fn an_empty_target_is_unsupported() {
    let tmp = tempfile::tempdir().unwrap();
    let error = ProgramModel::resolve(tmp.path()).unwrap_err();
    assert!(matches!(error, ProgramError::Unsupported(_)), "{error}");
    assert!(error.to_string().contains("no Anchor program"), "{error}");
}

#[test]
fn a_target_that_is_a_file_is_unsupported() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("not-a-directory");
    std::fs::write(&file, "nothing").unwrap();
    assert!(ProgramModel::resolve(&file).is_err());
}

#[test]
fn a_missing_target_is_an_io_error() {
    let missing = Path::new("rg-program-no-such-target-2f8a1c");
    assert!(ProgramModel::resolve(missing).is_err());
}
