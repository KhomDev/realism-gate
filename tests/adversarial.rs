//! Adversarial inputs: PoCs, reports and claims written to fool the checker.
//!
//! Each case says what the tool SHOULD do. A case the tool does not handle yet
//! is `#[ignore = "bug: ..."]` with a one-line reason, and its input is listed
//! in the PR. No expectation is weakened to make a case pass: a failing case is
//! a finding about the tool.
//!
//! Every input is written to a temporary directory at run time, so no file in
//! this repository holds an attack payload.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

const HEADER: &str = "// SPDX-License-Identifier: MIT\n\
pragma solidity ^0.8.20;\n\
\n\
import {Test} from \"forge-std/Test.sol\";\n\
\n\
contract Vault {\n\
    uint256 public fee;\n\
    uint256 public balance;\n\
\n\
    function setFee(uint256 f) external {\n\
        fee = f;\n\
    }\n\
\n\
    function withdrawAll() external {\n\
        balance = 0;\n\
    }\n\
}\n";

/// A temporary directory, removed when the case ends.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir =
            std::env::temp_dir().join(format!("rg-adv-{name}-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }

    fn write(&self, name: &str, text: &str) -> PathBuf {
        let path = self.0.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, text).unwrap();
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Runs the binary and waits at most `limit`. Panics if it is still running
/// then, and returns the output with the elapsed time otherwise.
fn run_bounded(dir: &Path, args: &[&str], limit: Duration) -> (Output, Duration) {
    let start = Instant::now();
    let mut child = Command::new(env!("CARGO_BIN_EXE_realism-gate"))
        .args(args)
        .current_dir(dir)
        .env("NO_COLOR", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("binary runs");
    // Drain the pipes on helper threads, so a chatty child cannot block on a
    // full pipe while this loop waits for it to exit.
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let out_t = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = std::io::Read::read_to_end(&mut stdout, &mut b);
        b
    });
    let err_t = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = std::io::Read::read_to_end(&mut stderr, &mut b);
        b
    });
    let status = loop {
        match child.try_wait().expect("wait") {
            Some(status) => break status,
            None if start.elapsed() > limit => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("still running after {limit:?}: {args:?}");
            }
            None => std::thread::sleep(Duration::from_millis(20)),
        }
    };
    let elapsed = start.elapsed();
    let output = Output {
        status,
        stdout: out_t.join().unwrap(),
        stderr: err_t.join().unwrap(),
    };
    (output, elapsed)
}

/// `check FILE --no-declaration --format json`, plus `--claims` if given.
fn check_json(file: &Path, claims: Option<&Path>) -> (Output, Value) {
    let dir = file.parent().unwrap().to_path_buf();
    let file_arg = file.to_string_lossy().to_string();
    let mut args = vec!["check", &file_arg, "--no-declaration", "--format", "json"];
    let claims_arg = claims.map(|c| c.to_string_lossy().to_string());
    if let Some(c) = claims_arg.as_deref() {
        args.push("--claims");
        args.push(c);
    }
    let (out, _) = run_bounded(&dir, &args, Duration::from_secs(120));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!stderr.contains("panicked"), "panic: {stderr}");
    let value = serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
        panic!(
            "not JSON ({e}): {}{stderr}",
            String::from_utf8_lossy(&out.stdout)
        )
    });
    (out, value)
}

/// Rule ids of the active findings.
fn rules(v: &Value) -> Vec<String> {
    v["findings"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|f| f["rule"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// The status of the first claim row for `field`, or "" if there is none.
fn status(v: &Value, field: &str) -> String {
    v["claims"]["rows"]
        .as_array()
        .and_then(|rows| rows.iter().find(|r| r["field"] == field))
        .and_then(|r| r["status"].as_str())
        .unwrap_or("")
        .to_string()
}

/// A PoC whose `test_case` body is `body`.
fn poc(body: &str) -> String {
    format!(
        "{HEADER}\n\
contract AdversaryTest is Test {{\n\
    Vault vault;\n\
    address admin = makeAddr(\"admin\");\n\
    address attacker = makeAddr(\"attacker\");\n\
\n\
    function setUp() public {{\n\
        vault = new Vault();\n\
    }}\n\
\n\
    function test_case() public {{\n\
{body}\n\
    }}\n\
}}\n"
    )
}

// ---------------------------------------------------------------------------
// Privilege hidden from RG02: the sender is the privileged role, but the PoC
// reaches it through a path the checker might not follow.
// ---------------------------------------------------------------------------

#[test]
fn privileged_address_aliased_through_a_local_variable() {
    // The prank target is `admin` under a local name. The privileged step must
    // still be reported as RG02, as a direct `vm.prank(admin)` is.
    let s = Scratch::new("alias");
    let file = s.write(
        "test/Alias.t.sol",
        &poc("        address who = admin;\n        vm.prank(who);\n        vault.setFee(0);"),
    );
    let (_, v) = check_json(&file, None);
    assert!(rules(&v).contains(&"RG02".to_string()), "{v}");
}

#[test]
#[ignore = "bug: a privileged prank inside a helper contract the test calls is not reported as RG02"]
fn privileged_address_aliased_through_a_helper_contract() {
    // INPUT: `Operator.act()` does `vm.prank(owner); vault.setFee(0);` and
    // `HelperTest.test_case` calls `op.act()`. SHOULD: RG02 (the step runs as
    // the privileged `owner`). ACTUAL: only RG08 fires.
    // The privileged prank lives in a helper contract the test calls. The
    // exploit crosses a contract boundary; it must still count.
    let s = Scratch::new("helper");
    let src = format!(
        "{HEADER}\n\
contract Operator is Test {{\n\
    address owner;\n\
    Vault vault;\n\
    constructor(address o, Vault v) {{\n\
        owner = o;\n\
        vault = v;\n\
    }}\n\
    function act() external {{\n\
        vm.prank(owner);\n\
        vault.setFee(0);\n\
    }}\n\
}}\n\
\n\
contract HelperTest is Test {{\n\
    function test_case() public {{\n\
        Vault vault = new Vault();\n\
        address admin = makeAddr(\"admin\");\n\
        Operator op = new Operator(admin, vault);\n\
        op.act();\n\
    }}\n\
}}\n"
    );
    let file = s.write("test/Helper.t.sol", &src);
    let (_, v) = check_json(&file, None);
    assert!(rules(&v).contains(&"RG02".to_string()), "{v}");
}

#[test]
#[ignore = "bug: a privileged address round-tripped through abi.encode/abi.decode is not reported as RG02"]
fn privileged_address_round_tripped_through_abi_encode_decode() {
    // INPUT: `address who = abi.decode(abi.encode(admin), (address)); vm.prank(who);`
    // SHOULD: RG02. ACTUAL: no RG02; only RG08 fires.
    // `admin` reaches the prank through abi.encode and abi.decode. The value is
    // the same address and must still be seen as the privileged one.
    let s = Scratch::new("abi");
    let file = s.write(
        "test/Abi.t.sol",
        &poc(
            "        address who = abi.decode(abi.encode(admin), (address));\n\
        vm.prank(who);\n\
        vault.setFee(0);",
        ),
    );
    let (_, v) = check_json(&file, None);
    assert!(rules(&v).contains(&"RG02".to_string()), "{v}");
}

#[test]
fn live_prank_after_a_dead_prank_is_still_reported() {
    // A prank inside `if (false)` never runs. It must not hide the live
    // privileged prank that follows it.
    let s = Scratch::new("deadprank");
    let file = s.write(
        "test/Dead.t.sol",
        &poc("        if (false) {\n\
            vm.prank(admin);\n\
        }\n\
        vm.prank(admin);\n\
        vault.setFee(0);"),
    );
    let (_, v) = check_json(&file, None);
    assert!(rules(&v).contains(&"RG02".to_string()), "{v}");
}

#[test]
fn a_prank_only_inside_if_false_does_not_contradict_the_attacker_claim() {
    // INPUT: `if (false) { vm.prank(admin); }` then `vm.prank(attacker); vault.withdrawAll();`
    // SHOULD: the dead prank does not run, so the attacker claim is not CONTRADICTED.
    // WAS (1.0.2): RG02 fired on the dead `vm.prank(admin)` and the claim was CONTRADICTED.
    // The only privileged prank is dead, so nothing privileged runs. The
    // attacker claim must not be CONTRADICTED by a prank that never happened.
    let s = Scratch::new("deadonly");
    let file = s.write(
        "test/DeadOnly.t.sol",
        &poc("        if (false) {\n\
            vm.prank(admin);\n\
        }\n\
        vm.prank(attacker);\n\
        vault.withdrawAll();"),
    );
    let claims = s.write(
        "claims.yaml",
        "schema: realism-gate.claims/v1\nattacker: any-user\n",
    );
    let (_, v) = check_json(&file, Some(&claims));
    assert_ne!(status(&v, "attacker"), "contradicted", "{v}");
}

// ---------------------------------------------------------------------------
// Assertions that cannot run, or that prove nothing.
// ---------------------------------------------------------------------------

#[test]
fn an_impact_assertion_after_return_is_not_evidence() {
    // INPUT: `vault.withdrawAll(); return; assertGt(before, vault.balance());`
    // SHOULD: drain is not SHOWN. WAS (1.0.2): drain was SHOWN.
    // The only value assertion sits after `return`. It never runs, so the drain
    // claim must not be SHOWN.
    let s = Scratch::new("unreach");
    let file = s.write(
        "test/Unreach.t.sol",
        &poc("        uint256 before = vault.balance();\n\
        vm.prank(attacker);\n\
        vault.withdrawAll();\n\
        return;\n\
        assertGt(before, vault.balance());"),
    );
    let claims = s.write(
        "claims.yaml",
        "schema: realism-gate.claims/v1\nimpact: drain\n",
    );
    let (_, v) = check_json(&file, Some(&claims));
    assert_ne!(status(&v, "impact"), "shown", "{v}");
}

#[test]
fn an_impact_assertion_inside_if_false_is_not_evidence() {
    // INPUT: `vault.withdrawAll(); if (false) { assertGt(before, vault.balance()); }`
    // SHOULD: drain is not SHOWN. WAS (1.0.2): drain was SHOWN.
    let s = Scratch::new("iffalse");
    let file = s.write(
        "test/IfFalse.t.sol",
        &poc("        uint256 before = vault.balance();\n\
        vm.prank(attacker);\n\
        vault.withdrawAll();\n\
        if (false) {\n\
            assertGt(before, vault.balance());\n\
        }"),
    );
    let claims = s.write(
        "claims.yaml",
        "schema: realism-gate.claims/v1\nimpact: drain\n",
    );
    let (_, v) = check_json(&file, Some(&claims));
    assert_ne!(status(&v, "impact"), "shown", "{v}");
}

// ---------------------------------------------------------------------------
// Claims that contradict each other.
// ---------------------------------------------------------------------------

#[test]
fn a_field_named_inside_and_outside_front_matter_is_an_error() {
    // INPUT: front matter with `attacker: any-user` at the top level and
    // `claims: attacker: role:admin` under `claims:`.
    // SHOULD: exit 2 (docs/claims.md: an error). WAS (1.0.2): exit 0 for the
    // claim table (COMPLETE, attacker any-user shown) and the duplicate dropped.
    // docs/claims.md: a field named both inside and outside the block is an
    // error, not a choice of which one wins.
    let s = Scratch::new("bothplaces");
    let file = s.write(
        "test/Both.t.sol",
        &poc("        vm.prank(attacker);\n        vault.withdrawAll();"),
    );
    let report = s.write(
        "finding.md",
        "---\nschema: realism-gate.claims/v1\nattacker: any-user\nclaims:\n  attacker: role:admin\n---\n\n# Finding\n",
    );
    let (out, _) = run_bounded(
        &s.0,
        &[
            "check",
            &file.to_string_lossy(),
            "--report",
            &report.to_string_lossy(),
            "--no-declaration",
            "--format",
            "json",
        ],
        Duration::from_secs(60),
    );
    assert_eq!(
        out.status.code(),
        Some(2),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn claims_yaml_does_not_silently_override_contradicting_front_matter() {
    // INPUT: claims.yaml says `attacker: any-user`; the report's front matter
    // says `attacker: role:admin`. SHOULD: the disagreement is named (an error,
    // or a note that the front matter was not used). WAS (1.0.2): claims.yaml
    // won and the other block was dropped without a word.
    let s = Scratch::new("contradict");
    let file = s.write(
        "test/Contra.t.sol",
        &poc("        vm.prank(attacker);\n        vault.withdrawAll();"),
    );
    let report = s.write(
        "finding.md",
        "---\nschema: realism-gate.claims/v1\nclaims:\n  attacker: role:admin\n---\n\n# Finding\n",
    );
    let claims = s.write(
        "claims.yaml",
        "schema: realism-gate.claims/v1\nattacker: any-user\n",
    );
    let (out, _) = run_bounded(
        &s.0,
        &[
            "check",
            &file.to_string_lossy(),
            "--report",
            &report.to_string_lossy(),
            "--claims",
            &claims.to_string_lossy(),
            "--no-declaration",
            "--format",
            "json",
        ],
        Duration::from_secs(60),
    );
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out.status.code() == Some(2) || text.contains("front matter"),
        "the disagreement was dropped silently: {text}"
    );
}

// ---------------------------------------------------------------------------
// Identifiers that look like privileged names.
// ---------------------------------------------------------------------------

#[test]
fn cyrillic_lookalike_identifier_is_not_silently_accepted() {
    // Solidity identifiers are ASCII. `\u{0430}dmin` (Cyrillic small a, U+0430)
    // is not the identifier `admin`. The file must be reported or refused, not
    // pass as if it named the privileged address.
    let s = Scratch::new("cyrillic");
    let file = s.write(
        "test/Cyr.t.sol",
        &poc("        address \u{0430}dmin2 = admin;\n        vm.prank(\u{0430}dmin2);\n        vault.setFee(0);"),
    );
    let (out, v) = check_json(&file, None);
    let parse_errors = v["parse_errors"].as_array().map(Vec::len).unwrap_or(0);
    assert!(
        parse_errors > 0 || out.status.code() == Some(2),
        "a non-ASCII identifier was accepted without a parse error: {v}"
    );
}

// ---------------------------------------------------------------------------
// Size and nesting: finish within a bound, and do not panic.
// ---------------------------------------------------------------------------

#[test]
fn a_five_megabyte_source_file_finishes_and_does_not_panic() {
    // INPUT: about 5 MB of small test functions (about 40 000 of them).
    // SHOULD: finish within 120 s. Before the suite trace was memoized the
    // callback scan ran once per test, so 885 KB (8 000 functions) took about
    // 165 s and 4 000 functions took 48 s. A suite base is now built once and
    // shared, which this test pins. Many small test functions, about 5 MB in
    // total. The CLI's three exit codes (0, 1, 2) are the only acceptable
    // outcomes; a crash is not.
    let s = Scratch::new("big");
    let mut src = String::from(HEADER);
    src.push_str("contract BigTest is Test {\n    Vault vault = new Vault();\n");
    let mut i: u64 = 0;
    while src.len() < 5 * 1024 * 1024 {
        src.push_str(&format!(
            "    function test_n{i}() public {{\n        vm.prank(address(uint160({i} + 1)));\n        vault.setFee({i});\n    }}\n"
        ));
        i += 1;
    }
    src.push_str("}\n");
    let file = s.write("test/Big.t.sol", &src);
    let (out, elapsed) = run_bounded(
        &s.0,
        &[
            "check",
            &file.to_string_lossy(),
            "--no-declaration",
            "--format",
            "json",
        ],
        Duration::from_secs(120),
    );
    assert!(!String::from_utf8_lossy(&out.stderr).contains("panicked"));
    assert!(
        matches!(out.status.code(), Some(0..=2)),
        "exit {:?} after {elapsed:?}",
        out.status
    );
}

#[test]
fn a_deeply_nested_expression_finishes_and_does_not_panic() {
    // 2 000 nested parentheses in one expression: well past any honest PoC.
    let s = Scratch::new("nest");
    let depth = 2_000;
    let expr = format!("{}1{}", "(".repeat(depth), ")".repeat(depth));
    let file = s.write(
        "test/Nest.t.sol",
        &poc(&format!(
            "        uint256 x = {expr};\n        vault.setFee(x);"
        )),
    );
    let (out, _) = run_bounded(
        &s.0,
        &[
            "check",
            &file.to_string_lossy(),
            "--no-declaration",
            "--format",
            "json",
        ],
        Duration::from_secs(120),
    );
    assert!(!String::from_utf8_lossy(&out.stderr).contains("panicked"));
    assert!(
        matches!(out.status.code(), Some(0..=2)),
        "exit {:?}",
        out.status
    );
}

#[test]
fn a_very_deeply_nested_expression_does_not_overflow_the_stack() {
    // INPUT: `uint256 x = ((...((1))...));` with 50 000 pairs of parentheses.
    // SHOULD: exit 0, 1 or 2 with a message, never a crash. Observed: the
    // parser stops with a parse error (exit 2), so the stack is not reached.
    let s = Scratch::new("nest-deep");
    let depth = 50_000;
    let expr = format!("{}1{}", "(".repeat(depth), ")".repeat(depth));
    let file = s.write(
        "test/NestDeep.t.sol",
        &poc(&format!(
            "        uint256 x = {expr};\n        vault.setFee(x);"
        )),
    );
    let (out, _) = run_bounded(
        &s.0,
        &[
            "check",
            &file.to_string_lossy(),
            "--no-declaration",
            "--format",
            "json",
        ],
        Duration::from_secs(120),
    );
    assert!(
        matches!(out.status.code(), Some(0..=2)),
        "exit {:?}",
        out.status
    );
}

#[test]
fn four_thousand_test_functions_finish_within_the_bound() {
    // The callback scan used to run once per test, so this case was quadratic
    // in the number of test functions (4 000 took about 48 s in a debug build).
    // A debug build gets a wider bound than the release suite the timeout is
    // really about.
    let bound = if cfg!(debug_assertions) {
        Duration::from_secs(60)
    } else {
        Duration::from_secs(10)
    };
    let s = Scratch::new("four-thousand");
    let mut src = String::from(HEADER);
    src.push_str("contract ManyTest is Test {\n    Vault vault = new Vault();\n");
    for i in 0..4_000u64 {
        src.push_str(&format!(
            "    function test_n{i}() public {{\n        vm.prank(address(uint160({i} + 1)));\n        vault.setFee({i});\n    }}\n"
        ));
    }
    src.push_str("}\n");
    let file = s.write("test/Many.t.sol", &src);
    let (out, elapsed) = run_bounded(
        &s.0,
        &[
            "check",
            &file.to_string_lossy(),
            "--no-declaration",
            "--format",
            "json",
        ],
        Duration::from_secs(120),
    );
    assert!(
        matches!(out.status.code(), Some(0..=2)),
        "exit {:?}: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        elapsed <= bound,
        "4 000 test functions took {elapsed:?}, over the {bound:?} bound"
    );
}

#[test]
fn an_oversized_source_file_is_refused_by_name_before_it_is_read() {
    // A 1 GiB file is refused from its metadata, not read and then rejected.
    // The error names the file and the limit.
    let s = Scratch::new("huge");
    let path = s.0.join("test/Huge.t.sol");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let file = std::fs::File::create(&path).unwrap();
    file.set_len(1024 * 1024 * 1024).unwrap();
    drop(file);
    let (out, elapsed) = run_bounded(
        &s.0,
        &[
            "check",
            &path.to_string_lossy(),
            "--no-declaration",
            "--format",
            "json",
        ],
        Duration::from_secs(60),
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(2), "{stderr}");
    assert!(
        stderr.contains("Huge.t.sol") && stderr.contains("limit"),
        "the refusal did not name the file and the limit: {stderr}"
    );
    assert!(
        elapsed < Duration::from_secs(30),
        "an oversized file was read before it was refused: {elapsed:?}"
    );
}

#[test]
fn control_characters_in_source_never_reach_text_or_markdown_output() {
    // A Solidity string literal can hold a raw ESC (0x1b): the parser keeps
    // the bytes, and the rule message and evidence quote them. The render
    // boundary drops control characters from user-derived fields, so a report
    // cannot drive the reader's terminal. The tool's own colour codes must
    // still appear when --color always asks for them.
    let s = Scratch::new("esc");
    let file = s.write(
        "test/Esc.t.sol",
        &poc(
            "        vm.createSelectFork(\"http://127.0.0.1/\u{1b}[31m\");\n        assertEq(\"v\u{1b}[31m\", \"v\u{1b}[31m\");",
        ),
    );
    let file_arg = file.to_string_lossy().to_string();
    let run = |extra: &[&str]| {
        let mut args = vec!["check", file_arg.as_str(), "--no-declaration", "--color"];
        args.extend_from_slice(extra);
        let (out, _) = run_bounded(&s.0, &args, Duration::from_secs(60));
        out
    };

    let text = run(&["never", "--format", "text"]);
    let text_out = String::from_utf8_lossy(&text.stdout);
    assert!(
        text_out.contains("RG09"),
        "the fixture produced no finding: {text_out}"
    );
    assert!(
        !text.stdout.contains(&0x1b),
        "text output carries a control character"
    );
    assert!(
        text_out.contains("[31m"),
        "the sanitized text should remain: {text_out}"
    );

    let markdown = run(&["never", "--format", "markdown"]);
    assert!(
        !markdown.stdout.contains(&0x1b),
        "markdown output carries a control character"
    );

    let coloured = run(&["always", "--format", "text"]);
    let coloured_out = String::from_utf8_lossy(&coloured.stdout);
    assert!(
        coloured.stdout.windows(4).any(|w| w == b"\x1b[1;"),
        "the tool's own colour codes are missing with --color always"
    );
    assert!(
        !coloured.stdout.windows(5).any(|w| w == b"\x1b[31m"),
        "the source's own escape sequence survived in colour output"
    );
    assert!(
        coloured_out.contains("[31m"),
        "the sanitized source text should remain: {coloured_out}"
    );
}
