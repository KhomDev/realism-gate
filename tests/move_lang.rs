//! Move examples in the rule pages, and the fixture modules.

use realism_gate::Severity::{self, Error, Warn};
use realism_gate::engine::{Options, analyze};
use realism_gate::project::{Project, discover};
use realism_gate::rules::{MoveNotes, RULES};
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn rules_of(src: &str) -> Vec<&'static str> {
    let project = Project::from_sources(&[("tests/t.move", src)]);
    let report = analyze(&project, None, &Options::default());
    assert!(
        report.parse_errors.is_empty(),
        "{src}: {:?}",
        report.parse_errors
    );
    assert_eq!(report.summary.tests, 1, "{src}");
    report.active().map(|f| f.rule).collect()
}

#[test]
fn move_examples_in_the_rule_pages_are_true() {
    for r in RULES {
        let MoveNotes::Checked { bad, good, .. } = r.move_lang else {
            continue;
        };
        let fired = rules_of(bad);
        assert!(
            fired.contains(&r.id),
            "{}: bad example raised {fired:?}",
            r.id
        );
        let fired = rules_of(good);
        assert!(
            !fired.contains(&r.id),
            "{}: good example raised {fired:?}\n{good}",
            r.id
        );
    }
}

fn expect(path: &str, want: &[(&str, Severity, u32)]) {
    let base = root();
    let files = discover(&[base.join(path)]).unwrap();
    let project = Project::load(&files, &base).unwrap();
    let report = analyze(&project, None, &Options::default());
    let got: Vec<(String, Severity, u32)> = report
        .active()
        .map(|f| (f.rule.to_string(), f.severity, f.line))
        .collect();
    let want: Vec<(String, Severity, u32)> = want
        .iter()
        .map(|(r, s, l)| (r.to_string(), *s, *l))
        .collect();
    assert_eq!(got, want, "{path}");
}

#[test]
fn move_fixtures() {
    expect(
        "tests/fixtures/move/bad/rg01_admin_cap.move",
        &[("RG01", Error, 8)],
    );
    expect(
        "tests/fixtures/move/bad/rg06_mint.move",
        &[("RG06", Warn, 5)],
    );
    expect(
        "tests/fixtures/move/bad/rg08_no_assert.move",
        &[("RG08", Error, 3)],
    );
    expect(
        "tests/fixtures/move/bad/rg10_no_baseline.move",
        &[("RG10", Warn, 7)],
    );
    let base = root();
    let files = discover(&[base.join("tests/fixtures/move/good")]).unwrap();
    let project = Project::load(&files, &base).unwrap();
    let report = analyze(&project, None, &Options::default());
    assert!(report.active().next().is_none(), "{:?}", report.findings);
}
