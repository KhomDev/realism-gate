//! The guard in front of oxc (`parse_ts::neutralize_annotations`).
//!
//! `oxc_parser` 0.146.0 fails `debug_assert!(comment.is_pure())` at
//! `lexer/trivia_builder.rs:62` when a `#__PURE__` comment precedes a
//! statement label. The weekly fuzz job found it (input in
//! `fuzz/corpus/typescript/regression_oxc_pure_comment.ts`). These tests run
//! in a debug build, where the assertion is live:
//!
//! 1. the crash input and its minimised form parse without a panic;
//! 2. an annotation before a flagged line moves no finding: the JSON report
//!    is byte-identical to the one for the same file with a plain comment of
//!    the same length;
//! 3. the guard is a no-op on input without the markers, and on any input it
//!    keeps the length and every line break.

use proptest::prelude::*;
use realism_gate::engine::{Options, analyze};
use realism_gate::project::Project;
use realism_gate::render::json;
use realism_gate::solana::parse_ts::{neutralize_annotations, parse_fragment, parse_source};
use std::borrow::Cow;
use std::path::Path;

/// The minimised upstream repro (`_orchestration/upstream/oxc/repro.ts`).
const MINIMAL: &str = "//#__PURE__\n:";

/// The same bug through a block comment and a real label.
const LABEL: &str = "/*#__PURE__*/a:0";

fn crash_input() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fuzz/corpus/typescript/regression_oxc_pure_comment.ts");
    // The fuzz target reads bytes the same way.
    String::from_utf8_lossy(&std::fs::read(path).unwrap()).into_owned()
}

#[test]
fn the_oxc_crash_input_parses_without_a_panic() {
    let crash = crash_input();
    assert!(
        crash.starts_with("// #__PURE__"),
        "the regression input changed"
    );
    for src in [crash.as_str(), MINIMAL, LABEL] {
        let _ = parse_source("fuzz.ts", src);
        let _ = parse_fragment("fuzz.ts", src);
        let _ = parse_fragment("fuzz.js", src);
        let p = Project::from_sources(&[("tests/fuzz.ts", src)]);
        let _ = json::render(&analyze(&p, None, &Options::default()));
    }
}

#[test]
fn the_minimal_input_still_reaches_the_guard() {
    // If oxc stops reading this as an annotation, the guard is not exercised.
    assert_eq!(neutralize_annotations(MINIMAL), "// __PURE__\n:");
    assert_eq!(neutralize_annotations(LABEL), "/* __PURE__*/a:0");
    assert!(matches!(
        neutralize_annotations(&crash_input()),
        Cow::Owned(_)
    ));
}

const RG02: &str = include_str!("fixtures/bad/RG02_privileged_signer.ts");

/// RG02's fixture with `lines` inserted before line `at` (1-based).
fn with_lines(at: usize, lines: &[&str]) -> String {
    let mut out: Vec<&str> = RG02.lines().collect();
    for (i, l) in lines.iter().enumerate() {
        out.insert(at - 1 + i, l);
    }
    out.join("\n") + "\n"
}

fn report_json(src: &str) -> String {
    let p = Project::from_sources(&[("tests/rg02.ts", src)]);
    json::render(&analyze(&p, None, &Options::default()))
}

#[test]
fn an_annotation_before_a_flagged_line_moves_no_finding() {
    // Line 23 is the admin's `await program.methods` call RG02 flags.
    let cases: &[(&[&str], &[&str])] = &[
        // a line annotation on its own line
        (
            &["    // #__PURE__ the admin call"],
            &["    // x__PURE__ the admin call"],
        ),
        // a block annotation on its own line
        (&["    /* @__PURE__ */"], &["    /* x__PURE__ */"]),
        // the crash shape: an annotation before a label
        (
            &["    // #__PURE__", "    setup: 0;"],
            &["    // x__PURE__", "    setup: 0;"],
        ),
        (
            &["    /* @__NO_SIDE_EFFECTS__ */ setup: 0;"],
            &["    /* x__NO_SIDE_EFFECTS__ */ setup: 0;"],
        ),
    ];
    let baseline = report_json(RG02);
    assert!(baseline.contains("\"RG02\""), "{baseline}");
    for (annotated, plain) in cases {
        let a = with_lines(23, annotated);
        let p = with_lines(23, plain);
        assert_eq!(a.len(), p.len());
        let (ja, jp) = (report_json(&a), report_json(&p));
        assert_eq!(ja, jp, "{annotated:?}");
        assert!(ja.contains("\"RG02\""), "{ja}");
        // The finding moved down by exactly the inserted lines.
        let shift = annotated.len() as u32;
        let line = |j: &str| -> Vec<u64> {
            let v: serde_json::Value = serde_json::from_str(j).unwrap();
            v["findings"]
                .as_array()
                .unwrap()
                .iter()
                .map(|f| f["line"].as_u64().unwrap())
                .collect()
        };
        let base: Vec<u64> = line(&baseline)
            .into_iter()
            .map(|l| if l >= 23 { l + u64::from(shift) } else { l })
            .collect();
        assert_eq!(line(&ja), base, "{annotated:?}");
    }
}

/// Text built from the bytes that make up an annotation, so the generator
/// often comes close to one.
fn near_annotation() -> impl Strategy<Value = String> {
    proptest::collection::vec(
        prop_oneof![
            Just("/"),
            Just("*"),
            Just("#"),
            Just("@"),
            Just("_"),
            Just("__"),
            Just("PURE"),
            Just("NO_SIDE_EFFECTS"),
            Just(" "),
            Just("\t"),
            Just("\n"),
            Just("\r"),
            Just("a"),
            Just(":"),
            Just("é"),
        ],
        0..40,
    )
    .prop_map(|parts| parts.concat())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn the_guard_is_a_no_op_without_the_markers(src in near_annotation()) {
        prop_assume!(!src.contains("__PURE__") && !src.contains("__NO_SIDE_EFFECTS__"));
        let out = neutralize_annotations(&src);
        prop_assert!(matches!(out, Cow::Borrowed(_)));
        prop_assert_eq!(out.as_bytes(), src.as_bytes());
    }

    #[test]
    fn the_guard_keeps_every_offset(src in near_annotation()) {
        let out = neutralize_annotations(&src);
        prop_assert_eq!(out.len(), src.len());
        for (a, b) in src.bytes().zip(out.bytes()) {
            // Only a sigil changes, and only to a space.
            prop_assert!(a == b || (matches!(a, b'#' | b'@') && b == b' '));
        }
        let _ = parse_fragment("p.ts", &src);
    }
}
