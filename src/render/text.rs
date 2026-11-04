//! Human-readable output.

use super::{TOOL, VERSION};
use crate::engine::Report;
use crate::finding::{Finding, Severity};

#[derive(Debug, Clone, Copy, Default)]
pub struct TextOptions {
    pub color: bool,
    pub show_suppressed: bool,
    pub fail_on: Option<Severity>,
}

struct Paint {
    on: bool,
}

impl Paint {
    fn wrap(&self, code: &str, s: &str) -> String {
        if self.on {
            format!("\x1b[{code}m{s}\x1b[0m")
        } else {
            s.to_string()
        }
    }
    fn severity(&self, s: Severity) -> String {
        let label = format!("{:<5}", s.as_str());
        match s {
            Severity::Error => self.wrap("1;31", &label),
            Severity::Warn => self.wrap("1;33", &label),
            Severity::Note => self.wrap("1;36", &label),
        }
    }
    fn dim(&self, s: &str) -> String {
        self.wrap("2", s)
    }
    fn bold(&self, s: &str) -> String {
        self.wrap("1", s)
    }
}

fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("{n} {one}")
    } else {
        format!("{n} {many}")
    }
}

/// Wraps `text` at `width` columns, indenting every line by `indent` spaces.
fn wrap(text: &str, indent: usize, width: usize) -> String {
    let pad = " ".repeat(indent);
    let mut out = String::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        if !line.is_empty() && indent + line.chars().count() + 1 + word.chars().count() > width {
            out.push_str(&pad);
            out.push_str(&line);
            out.push('\n');
            line.clear();
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        out.push_str(&pad);
        out.push_str(&line);
        out.push('\n');
    }
    out
}

fn tests_label(f: &Finding) -> String {
    match f.tests.len() {
        0 => String::new(),
        1 => format!("{}::{}", f.suite, f.tests[0]),
        n => format!("{} ({n} tests)", f.suite),
    }
}

pub fn render(report: &Report, opts: TextOptions) -> String {
    let p = Paint { on: opts.color };
    let s = &report.summary;
    let mut out = String::new();
    out.push_str(&format!(
        "{} {}  ·  {}  ·  {}  ·  {}\n",
        p.bold(TOOL),
        VERSION,
        plural(s.files, "file", "files"),
        plural(s.suites, "suite", "suites"),
        plural(s.tests, "test", "tests"),
    ));
    if let Some(d) = &report.declaration {
        out.push_str(&format!("declaration  {d}\n"));
    }
    if let Some(c) = &report.config {
        out.push_str(&format!("config       {c}\n"));
    }
    if let Some(c) = &report.changed_since {
        out.push_str(&format!(
            "changed      {} since {}; findings elsewhere are not reported\n",
            plural(c.files, "file", "files"),
            c.git_ref
        ));
    }
    out.push('\n');

    let mut current_file: Option<&str> = None;
    for f in &report.findings {
        if !f.is_active() && !opts.show_suppressed {
            continue;
        }
        if current_file != Some(f.file.as_str()) {
            out.push_str(&format!("{}\n", p.bold(&f.file)));
            current_file = Some(f.file.as_str());
        }
        let sev = if f.is_active() {
            p.severity(f.severity)
        } else {
            p.dim("allow")
        };
        let location = format!("{}:{}:{}", f.file, f.line, f.column);
        let who = tests_label(f);
        out.push_str(&format!(
            "  {sev}  {} {}  {}{}\n",
            p.bold(f.rule),
            f.slug,
            p.dim(&location),
            if who.is_empty() {
                String::new()
            } else {
                format!("  {}", p.dim(&who))
            }
        ));
        out.push_str(&wrap(&f.message, 9, 96));
        if !f.evidence.is_empty() {
            out.push_str(&format!("         ↳ {}\n", p.dim(&f.evidence)));
        }
        if let Some(sup) = &f.suppressed {
            out.push_str(&format!(
                "         allowed on line {}: {}\n",
                sup.line, sup.reason
            ));
        } else {
            let help = wrap(&format!("→ {}", f.help), 9, 96);
            out.push_str(&help);
        }
        if let Some(line) = f.ignored_suppression {
            out.push_str(&format!(
                "         the allow comment on line {line} was ignored: it gives no reason\n"
            ));
        }
        out.push('\n');
    }

    if !report.parse_errors.is_empty() {
        out.push_str(&format!(
            "{}  {} could not be parsed completely; findings in them may be missing:\n",
            p.severity(Severity::Warn),
            plural(
                report
                    .parse_errors
                    .iter()
                    .map(|e| &e.file)
                    .collect::<std::collections::BTreeSet<_>>()
                    .len(),
                "file",
                "files"
            )
        ));
        let mut shown = std::collections::BTreeSet::new();
        for e in &report.parse_errors {
            if shown.insert(&e.file) {
                let first = e.message.lines().next().unwrap_or("");
                out.push_str(&format!("         {}  {}\n", e.file, p.dim(first)));
            }
        }
        out.push('\n');
    }

    out.push_str(&format!(
        "{}  {}  ·  {}  ·  {}  ·  {} suppressed\n",
        p.bold("SUMMARY"),
        plural(s.errors, "error", "errors"),
        plural(s.warnings, "warning", "warnings"),
        plural(s.notes, "note", "notes"),
        s.suppressed
    ));
    if let Some(level) = opts.fail_on {
        let verdict = if report.fails(level) {
            p.wrap("1;31", "FAIL")
        } else {
            p.wrap("1;32", "PASS")
        };
        out.push_str(&format!(
            "{}   {verdict}  (fails on {} and above)\n",
            p.bold("RESULT"),
            level.as_str()
        ));
    }
    out.push('\n');
    out.push_str(&p.dim(
        "realism-gate reads the test; it does not run it. A clean result means no known\n\
         defect pattern was found, not that the PoC is valid. `realism-gate explain <RULE>`\n\
         shows why a rule exists and how it can be wrong.\n",
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::wrap;

    #[test]
    fn wraps_and_indents() {
        assert_eq!(wrap("aaa bbb ccc", 2, 9), "  aaa bbb\n  ccc\n");
        assert_eq!(wrap("", 2, 9), "");
        // A word longer than the width still gets its own line.
        assert_eq!(
            wrap("x averyveryverylongword y", 0, 8),
            "x\naveryveryverylongword\ny\n"
        );
    }
}
