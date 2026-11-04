//! Markdown, for a CI job summary or a pull-request comment. Like the text
//! format it is for people and may change between releases; parse the JSON.

use super::{TOOL, VERSION, rule_url};
use crate::engine::Report;
use crate::finding::{Finding, Severity};

/// More rows than this and the table is cut, with a count of the rest. A
/// GitHub comment holds 65,536 characters.
const MAX_ROWS: usize = 50;

fn cell(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace('|', "\\|")
}

fn code(text: &str) -> String {
    format!("`{}`", cell(text).replace('`', "'"))
}

fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("{n} {one}")
    } else {
        format!("{n} {many}")
    }
}

fn who(f: &Finding) -> String {
    match f.tests.len() {
        0 => String::new(),
        1 => code(&format!("{}::{}", f.suite, f.tests[0])),
        n => format!("{} ({n} tests)", code(&f.suite)),
    }
}

fn row(f: &Finding) -> String {
    format!(
        "| {} | [{}]({}) {} | {} | {} | {} |\n",
        if f.is_active() {
            f.severity.as_str()
        } else {
            "allow"
        },
        f.rule,
        rule_url(f.rule),
        code(f.slug),
        code(&format!("{}:{}", f.file, f.line)),
        who(f),
        cell(&f.message),
    )
}

pub fn render(report: &Report, fail_on: Option<Severity>) -> String {
    let s = &report.summary;
    let failed = fail_on.is_some_and(|l| report.fails(l));
    let mut out = format!(
        "### {TOOL} {VERSION}: {}\n\n",
        if failed { "FAIL" } else { "PASS" }
    );
    out.push_str(&format!(
        "**{} · {} · {} · {} suppressed** · {}\n\n",
        plural(s.errors, "error", "errors"),
        plural(s.warnings, "warning", "warnings"),
        plural(s.notes, "note", "notes"),
        s.suppressed,
        match fail_on {
            Some(l) => format!("fails on {} and above", l.as_str()),
            None => "never fails".to_string(),
        }
    ));
    let mut scope = vec![
        plural(s.files, "file", "files"),
        plural(s.suites, "suite", "suites"),
        plural(s.tests, "test", "tests"),
    ];
    if let Some(c) = &report.changed_since {
        scope.push(format!(
            "findings limited to {} changed since {}",
            plural(c.files, "file", "files"),
            code(&c.git_ref)
        ));
    }
    if let Some(d) = &report.declaration {
        scope.push(format!("declaration {}", code(d)));
    }
    if let Some(c) = &report.config {
        scope.push(format!("config {}", code(c)));
    }
    out.push_str(&format!("{}\n\n", scope.join(" · ")));

    let active: Vec<&Finding> = report.active().collect();
    if !active.is_empty() {
        out.push_str("| Level | Rule | Where | Test | Finding |\n|---|---|---|---|---|\n");
        for f in active.iter().take(MAX_ROWS) {
            out.push_str(&row(f));
        }
        if active.len() > MAX_ROWS {
            out.push_str(&format!(
                "\n{} more not shown; the JSON or SARIF report has them all.\n",
                active.len() - MAX_ROWS
            ));
        }
        out.push('\n');
    }
    let silenced: Vec<&Finding> = report.findings.iter().filter(|f| !f.is_active()).collect();
    if !silenced.is_empty() {
        out.push_str(&format!(
            "<details><summary>{} silenced by an allow comment</summary>\n\n| Level | Rule | Where | Test | Reason |\n|---|---|---|---|---|\n",
            plural(silenced.len(), "finding", "findings")
        ));
        for f in silenced.iter().take(MAX_ROWS) {
            let reason = f.suppressed.as_ref().map_or("", |s| s.reason.as_str());
            out.push_str(&format!(
                "| allow | [{}]({}) | {} | {} | {} |\n",
                f.rule,
                rule_url(f.rule),
                code(&format!("{}:{}", f.file, f.line)),
                who(f),
                cell(reason)
            ));
        }
        out.push_str("\n</details>\n\n");
    }
    if s.suppressions_ignored > 0 {
        out.push_str(&format!(
            "{} ignored: an allow comment must give a reason.\n\n",
            plural(
                s.suppressions_ignored,
                "allow comment was",
                "allow comments were"
            )
        ));
    }
    let unparsed: std::collections::BTreeSet<&str> = report
        .parse_errors
        .iter()
        .map(|e| e.file.as_str())
        .collect();
    if !unparsed.is_empty() {
        out.push_str(&format!(
            "{} could not be fully parsed.\n\n",
            plural(unparsed.len(), "file", "files")
        ));
    }
    out.push_str(
        "<sub>realism-gate reads the test; it does not run it. A clean result means no known \
         defect pattern was found, not that the PoC is valid.</sub>\n",
    );
    out
}
