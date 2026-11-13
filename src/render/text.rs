//! Human-readable output.

use super::{TOOL, VERSION};
use crate::engine::Report;
use crate::finding::{Finding, Severity};

#[derive(Debug, Clone, Copy, Default)]
pub struct TextOptions {
    pub color: bool,
    pub show_suppressed: bool,
    pub fail_on: Option<Severity>,
    /// `--no-fail-on-claims`: the claim table is printed, but it does not
    /// decide the exit status.
    pub no_fail_on_claims: bool,
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
    fn claim_status(&self, s: crate::claims::Status) -> String {
        use crate::claims::Status;
        match s {
            Status::Shown => self.wrap("1;32", s.label()),
            Status::NotShown => self.wrap("1;33", s.label()),
            Status::Contradicted => self.wrap("1;31", s.label()),
            Status::NotCheckable => self.dim(s.label()),
        }
    }
}

fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("{n} {one}")
    } else {
        format!("{n} {many}")
    }
}

/// Word-wraps `text` into chunks of at most `width` columns. A word longer
/// than the width gets its own line rather than being cut.
fn chunks(text: &str, width: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > width {
            out.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        out.push(line);
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

fn columns(cells: &[(String, usize)]) -> String {
    let mut line = String::new();
    for (i, (text, width)) in cells.iter().enumerate() {
        if i > 0 {
            line.push_str("  ");
        }
        if *width > 0 {
            line.push_str(&format!("{text:<width$}", width = *width));
        } else {
            line.push_str(text);
        }
    }
    line.trim_end().to_string() + "\n"
}

/// The claim table: CLAIM / EVIDENCE / STATUS, one row per claim field.
pub fn claims_table(table: &crate::claims::Table, color: bool) -> String {
    const CLAIM_W: usize = 30;
    const EVIDENCE_W: usize = 44;
    let p = Paint { on: color };
    let mut out = String::new();
    out.push_str(&columns(&[
        ("CLAIM".to_string(), CLAIM_W),
        ("EVIDENCE".to_string(), EVIDENCE_W),
        ("STATUS".to_string(), 0),
    ]));
    for row in &table.rows {
        let claim = chunks(&row.claim, CLAIM_W);
        let evidence = chunks(&row.evidence, EVIDENCE_W);
        let height = claim.len().max(evidence.len());
        for i in 0..height {
            let c = claim.get(i).cloned().unwrap_or_default();
            let e = evidence.get(i).cloned().unwrap_or_default();
            if i == 0 {
                out.push_str(&columns(&[
                    (c, CLAIM_W),
                    (e, EVIDENCE_W),
                    (p.claim_status(row.status), 0),
                ]));
            } else {
                out.push_str(&columns(&[(c, CLAIM_W), (e, EVIDENCE_W)]));
            }
        }
    }
    out
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

    if let Some(t) = &report.claims {
        out.push_str(&format!(
            "{}  {}  ·  {}\n",
            p.bold("CLAIMS"),
            t.source,
            t.binding.as_str()
        ));
        out.push_str(&claims_table(t, opts.color));
        out.push_str(&format!(
            "{}  {}\n\n",
            p.bold("CLAIMS"),
            p.bold(&t.verdict())
        ));
    }

    out.push_str(&format!(
        "{}  {}  ·  {}  ·  {}  ·  {} suppressed\n",
        p.bold("SUMMARY"),
        plural(s.errors, "error", "errors"),
        plural(s.warnings, "warning", "warnings"),
        plural(s.notes, "note", "notes"),
        s.suppressed
    ));
    // The exit status is the rule threshold or the claim table, whichever
    // fails first. Saying PASS on a run that exits 1 because a claim is
    // contradicted would be the one lie a checker cannot afford.
    let claims_fail = !opts.no_fail_on_claims && report.claims.as_ref().is_some_and(|t| t.fails());
    let claim_reason = report.claims.as_ref().and_then(|t| match t.outcome {
        crate::claims::Outcome::Contradicted => {
            Some(format!("{} claim(s) contradicted", t.contradicted))
        }
        _ if t.binding == crate::claims::Binding::Inferred => Some("claims inferred".to_string()),
        _ => None,
    });
    if let Some(level) = opts.fail_on {
        let rule_fail = report.fails(level);
        let verdict = if rule_fail || claims_fail {
            p.wrap("1;31", "FAIL")
        } else {
            p.wrap("1;32", "PASS")
        };
        let reason = match (&claim_reason, claims_fail) {
            (Some(reason), true) => format!("claims: {reason}"),
            _ => format!("fails on {} and above", level.as_str()),
        };
        out.push_str(&format!("{}   {verdict}  ({reason})\n", p.bold("RESULT")));
    } else if claims_fail {
        // --fail-on never still cannot hide a contradicted claim.
        let verdict = p.wrap("1;31", "FAIL");
        let reason = claim_reason.unwrap_or_else(|| "claims".to_string());
        out.push_str(&format!(
            "{}   {verdict}  (claims: {reason})\n",
            p.bold("RESULT")
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
