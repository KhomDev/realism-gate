//! RG-A6: the still-unreadable DEV classes, one must-read fixture and one
//! twin per class, named after the class. Fixtures are synthetic
//! (`tests/fixtures/conformance/dev_classes/<class>/`); none is third-party
//! text.
//!
//! - `ts_import_less_it_block`: the file-level Solana import gate rejected an
//!   `it()` block pasted with no imports (and bare statements whose text
//!   contains `it(`, as `deposit(` does).
//! - `ts_indirect_anchor_import`: the test imports Anchor through its own
//!   `./utils`, so the gate saw no Solana import.
//! - `ts_fragment_parse_failure`: fragments that elide code with `...` or are
//!   pasted as a diff did not parse.
//! - `rust_mollusk_main`: a Mollusk PoC binary's `fn main` at `src/main.rs`.
//! - `rust_macro_generated_tests`: tests that exist only after a macro
//!   expands stay unreadable, and the report says why.
//! - `mixed_language`: a test saved under the other language's extension
//!   stays unreadable, and the report names the language.

use realism_gate::engine::{Options, Report, analyze};
use realism_gate::project::Project;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

const DIR: &str = "tests/fixtures/conformance/dev_classes";

fn read(rel: &str) -> String {
    std::fs::read_to_string(root().join(DIR).join(rel)).expect("fixture exists")
}

/// Reads the fixtures as one project, displayed under their class directory.
fn project(rels: &[&str]) -> Project {
    let sources: Vec<(String, String)> = rels
        .iter()
        .map(|r| (format!("{DIR}/{r}"), read(r)))
        .collect();
    let refs: Vec<(&str, &str)> = sources
        .iter()
        .map(|(p, s)| (p.as_str(), s.as_str()))
        .collect();
    Project::from_sources(&refs)
}

fn report(rels: &[&str]) -> Report {
    analyze(&project(rels), None, &Options::default())
}

fn assert_readable(rels: &[&str]) -> Report {
    let r = report(rels);
    assert!(r.summary.tests >= 1, "{rels:?}: {:?}", r.summary);
    assert!(r.parse_errors.is_empty(), "{rels:?}: {:?}", r.parse_errors);
    r
}

fn run_binary(args: &[&str], cwd: &Path) -> std::process::Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_realism-gate"))
        .args(args)
        .current_dir(cwd)
        .env("NO_COLOR", "1")
        .output()
        .expect("the binary runs")
}

// ---------------------------------------------------------------------------
// ts_import_less_it_block

#[test]
fn ts_import_less_it_block_is_read() {
    let r = assert_readable(&["ts_import_less_it_block/read.ts"]);
    assert_eq!(r.summary.files, 1);
}

#[test]
fn ts_import_less_bare_statements_with_a_deposit_call_are_read() {
    assert_readable(&["ts_import_less_it_block/read_deposit_call.ts"]);
}

#[test]
fn ts_import_less_plain_mocha_stays_unread() {
    let p = project(&["ts_import_less_it_block/skip_plain_mocha.ts"]);
    assert!(
        p.units.is_empty(),
        "a plain mocha test is not a Solana test"
    );
}

#[test]
fn ts_import_less_it_block_is_read_by_the_cli_with_fragment() {
    let dir = root().join(DIR).join("ts_import_less_it_block");
    for file in ["read.ts", "read_deposit_call.ts"] {
        let o = run_binary(
            &[
                "check",
                file,
                "--fragment",
                "--no-declaration",
                "--no-config",
                "--format",
                "json",
                "--fail-on",
                "never",
            ],
            &dir,
        );
        assert_eq!(
            o.status.code(),
            Some(0),
            "{file}: {}",
            String::from_utf8_lossy(&o.stderr)
        );
        let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
        assert_eq!(v["summary"]["tests"], 1, "{file}: {v}");
    }
}

// ---------------------------------------------------------------------------
// ts_indirect_anchor_import

#[test]
fn ts_indirect_anchor_import_through_utils_is_read() {
    let r = assert_readable(&[
        "ts_indirect_anchor_import/read/vault.ts",
        "ts_indirect_anchor_import/read/utils.ts",
    ]);
    assert_eq!(r.summary.tests, 1);
}

#[test]
fn ts_indirect_import_with_an_evm_stack_stays_unread() {
    let p = project(&["ts_indirect_anchor_import/skip/vault.ts"]);
    assert!(p.units.is_empty(), "a Hardhat suite is not a Solana test");
}

// ---------------------------------------------------------------------------
// ts_fragment_parse_failure

#[test]
fn ts_fragment_with_elisions_is_read() {
    assert_readable(&["ts_fragment_parse_failure/read_elided.ts"]);
}

#[test]
fn ts_fragment_pasted_as_a_diff_is_read() {
    let r = assert_readable(&["ts_fragment_parse_failure/read_diff.ts"]);
    // The added test is checked. The context test, "sets a limit", builds the
    // world by its title and runs as setup for the tests after it.
    assert_eq!(r.summary.tests, 1, "{:?}", r.summary);
    assert!(
        r.findings
            .iter()
            .all(|f| f.tests.iter().all(|t| t == "attacker resets the limit")),
        "{:?}",
        r.findings
    );
}

#[test]
fn ts_fragment_broken_otherwise_reports_its_error() {
    let r = report(&["ts_fragment_parse_failure/unreadable_broken.ts"]);
    assert!(!r.parse_errors.is_empty(), "the broken fragment parsed");
    assert!(
        r.parse_errors[0].message.starts_with("error: "),
        "{:?}",
        r.parse_errors
    );
}

/// The repair blanks bytes, so a finding in a repaired fragment points at the
/// same line and column as in the original text.
#[test]
fn a_repaired_fragment_keeps_every_position() {
    let rel = "ts_fragment_parse_failure/read_elided.ts";
    let src = read(rel);
    let r = report(&[rel]);
    for f in r.active() {
        let line = src.lines().nth(f.line as usize - 1).expect("line exists");
        let at = line
            .char_indices()
            .nth(f.column as usize - 1)
            .map(|(i, _)| &line[i..])
            .unwrap_or("");
        let head = f.evidence.lines().next().unwrap_or("");
        assert!(
            at.starts_with(head.trim_start()) || line.contains(head.trim()),
            "{}: {} at {}:{} quotes `{head}`, the line is `{line}`",
            f.rule,
            f.file,
            f.line,
            f.column
        );
    }
}

// ---------------------------------------------------------------------------
// rust_mollusk_main

#[test]
fn rust_mollusk_main_is_read() {
    let r = assert_readable(&["rust_mollusk_main/read/src/main.rs"]);
    assert_eq!(r.summary.tests, 1);
}

#[test]
fn rust_plain_main_stays_unread() {
    let r = report(&["rust_mollusk_main/skip/src/main.rs"]);
    assert_eq!(r.summary.tests, 0, "{:?}", r.summary);
    assert!(r.parse_errors.is_empty(), "{:?}", r.parse_errors);
}

// ---------------------------------------------------------------------------
// rust_macro_generated_tests

#[test]
fn rust_macro_generated_tests_are_unreadable_with_the_macro_named() {
    for (rel, name) in [
        (
            "rust_macro_generated_tests/unreadable_invocation.rs",
            "`for_each_token!`",
        ),
        (
            "rust_macro_generated_tests/unreadable_macro_rules.rs",
            "`macro_rules! drain_test`",
        ),
    ] {
        let r = report(&[rel]);
        assert_eq!(r.summary.tests, 0, "{rel}: {:?}", r.summary);
        assert!(
            r.parse_errors
                .iter()
                .any(|e| e.message.contains(name) && e.message.contains("does not expand macros")),
            "{rel}: {:?}",
            r.parse_errors
        );
    }
}

#[test]
fn rust_macro_invocations_without_a_test_attribute_add_no_note() {
    let r = report(&["rust_macro_generated_tests/unreadable_macro_rules.rs"]);
    // One note for the definition; `drain_test!(drains_one, 1)` holds no
    // attribute itself.
    assert_eq!(r.parse_errors.len(), 1, "{:?}", r.parse_errors);
}

#[test]
fn rust_test_written_out_is_read() {
    assert_readable(&["rust_macro_generated_tests/read_written_out.rs"]);
}

/// The CLI says why nothing was checked.
#[test]
fn the_cli_reports_the_macro_reason() {
    let dir = root().join(DIR).join("rust_macro_generated_tests");
    let o = run_binary(
        &[
            "check",
            "unreadable_invocation.rs",
            "--no-declaration",
            "--no-config",
        ],
        &dir,
    );
    assert_eq!(o.status.code(), Some(2));
    let out = String::from_utf8_lossy(&o.stdout);
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(
        err.contains("no test suite found (1 parse error(s)"),
        "{err}"
    );
    assert!(out.contains("for_each_token!"), "{out}");
}

// ---------------------------------------------------------------------------
// mixed_language

#[test]
fn typescript_saved_as_rust_names_the_language() {
    let r = report(&["mixed_language/typescript_in_rs.rs"]);
    assert_eq!(r.summary.tests, 0);
    assert_eq!(r.parse_errors.len(), 1, "{:?}", r.parse_errors);
    assert!(
        r.parse_errors[0]
            .message
            .contains("reads as TypeScript or JavaScript, not Rust"),
        "{:?}",
        r.parse_errors
    );
}

#[test]
fn rust_saved_as_typescript_names_the_language() {
    let r = report(&["mixed_language/rust_in_ts.ts"]);
    assert_eq!(r.summary.tests, 0);
    assert!(
        r.parse_errors
            .first()
            .is_some_and(|e| e.message.contains("reads as Rust, not TypeScript")),
        "{:?}",
        r.parse_errors
    );
}

#[test]
fn broken_rust_does_not_claim_another_language() {
    let r = report(&["mixed_language/broken_rust.rs"]);
    assert_eq!(r.parse_errors.len(), 1);
    assert!(
        !r.parse_errors[0].message.contains("reads as"),
        "{:?}",
        r.parse_errors
    );
}

// ---------------------------------------------------------------------------
// Malformed input: every prefix of every fixture parses without a panic, in
// both frontends (truncated input), and a fragment at the size cap is handled.

#[test]
fn truncated_fixtures_never_panic() {
    let rels = [
        "ts_import_less_it_block/read.ts",
        "ts_import_less_it_block/read_deposit_call.ts",
        "ts_fragment_parse_failure/read_elided.ts",
        "ts_fragment_parse_failure/read_diff.ts",
        "ts_fragment_parse_failure/unreadable_broken.ts",
        "rust_macro_generated_tests/unreadable_invocation.rs",
        "rust_macro_generated_tests/unreadable_macro_rules.rs",
        "mixed_language/typescript_in_rs.rs",
        "mixed_language/rust_in_ts.ts",
    ];
    for rel in rels {
        let src = read(rel);
        let path = format!("{DIR}/{rel}");
        for (cut, _) in src.char_indices().step_by(7) {
            let _ = Project::from_sources(&[(path.as_str(), &src[..cut])]);
        }
    }
}

#[test]
fn a_large_elided_fragment_is_repaired_in_one_pass() {
    // About 1 MiB of elided account lists: the repair is linear.
    let line =
        "await program.methods.withdraw().accounts({ user: attacker.publicKey, ... }).rpc();\n";
    let body = line.repeat(1 << 20 >> 7);
    let src = format!(
        "it(\"x\", async () => {{\nconst attacker = Keypair.generate();\n{body}assert.isTrue(true);\n}});\n"
    );
    let p = Project::from_sources(&[("big.ts", src.as_str())]);
    assert_eq!(p.units.len(), 1);
    assert!(p.units[0].errors.is_empty(), "{:?}", p.units[0].errors);
}
