//! Runs the rules over a project and assembles the report.

use crate::declaration::Declaration;
use crate::finding::{Finding, Severity, Suppression};
use crate::identity::Env;
use crate::project::Project;
use crate::rules::{self, Raw, help};
use crate::suppress::{self, Directive};
use crate::trace;
use regex::Regex;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

/// Index used in [`Raw::unit`] for findings filed against the declaration.
pub const DECLARATION_UNIT: usize = usize::MAX;

#[derive(Debug, Default, Clone)]
pub struct Options {
    /// Run only these rules (ids). Empty means all.
    pub only: BTreeSet<String>,
    /// Never run these rules (ids).
    pub skip: BTreeSet<String>,
    pub match_test: Option<Regex>,
    pub match_contract: Option<Regex>,
}

impl Options {
    fn enabled(&self, rule: &str) -> bool {
        (self.only.is_empty() || self.only.contains(rule)) && !self.skip.contains(rule)
    }
}

/// A declaration together with where it came from.
pub struct LoadedDeclaration<'a> {
    pub decl: &'a Declaration,
    pub path: String,
    pub text: &'a str,
}

#[derive(Debug, Clone, Serialize)]
pub struct ParseError {
    pub file: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Summary {
    pub files: usize,
    pub suites: usize,
    pub tests: usize,
    pub errors: usize,
    pub warnings: usize,
    pub notes: usize,
    pub suppressed: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub summary: Summary,
    pub declaration: Option<String>,
    pub findings: Vec<Finding>,
    pub parse_errors: Vec<ParseError>,
}

impl Report {
    pub fn active(&self) -> impl Iterator<Item = &Finding> {
        self.findings.iter().filter(|f| f.is_active())
    }

    /// Whether any active finding is at or above `level`.
    pub fn fails(&self, level: Severity) -> bool {
        self.active().any(|f| f.severity >= level)
    }
}

pub fn analyze(
    project: &Project,
    declaration: Option<&LoadedDeclaration<'_>>,
    opts: &Options,
) -> Report {
    let directives: Vec<Vec<Directive>> = project
        .units
        .iter()
        .map(|u| suppress::directives(&u.source))
        .collect();

    // (rule, file, line, column) -> finding, so a setUp() line shared by many
    // tests is reported once, listing them all.
    let mut merged: BTreeMap<(String, u32, u32, &'static str), Finding> = BTreeMap::new();
    let mut summary = Summary {
        files: project.units.len(),
        ..Summary::default()
    };

    let mut add = |raw: Raw, suite: &str, test: &str| {
        if !opts.enabled(raw.rule) {
            return;
        }
        let info = rules::rule(raw.rule).expect("every raw finding names a catalog rule");
        let (file, dirs): (String, &[Directive]) = if raw.unit == DECLARATION_UNIT {
            (declaration.map(|d| d.path.clone()).unwrap_or_default(), &[])
        } else {
            (project.units[raw.unit].path.clone(), &directives[raw.unit])
        };
        let k = (file.clone(), raw.pos.line, raw.pos.col, raw.rule);
        if let Some(existing) = merged.get_mut(&k) {
            if !test.is_empty() && !existing.tests.iter().any(|t| t == test) {
                existing.tests.push(test.to_string());
            }
            return;
        }
        let directive = suppress::find(dirs, raw.rule, raw.pos.line);
        let (suppressed, ignored_suppression) = match directive {
            Some(d) if !d.reason.is_empty() => (
                Some(Suppression {
                    line: d.line,
                    reason: d.reason.clone(),
                }),
                None,
            ),
            Some(d) => (None, Some(d.line)),
            None => (None, None),
        };
        merged.insert(
            k,
            Finding {
                rule: info.id,
                slug: info.slug,
                severity: raw.severity,
                file,
                line: raw.pos.line,
                column: raw.pos.col,
                suite: suite.to_string(),
                tests: if test.is_empty() {
                    vec![]
                } else {
                    vec![test.to_string()]
                },
                message: raw.message,
                evidence: raw.evidence,
                help: help(info),
                suppressed,
                ignored_suppression,
            },
        );
    };

    for suite in project.suites() {
        if let Some(re) = &opts.match_contract
            && !re.is_match(&suite.contract.name)
        {
            continue;
        }
        summary.suites += 1;
        for test in &suite.tests {
            if let Some(re) = &opts.match_test
                && !re.is_match(&test.name)
            {
                continue;
            }
            summary.tests += 1;
            let tr = trace::build(project, &suite, test);
            let mut env = Env::new(project, suite.unit, &tr);
            if let Some(d) = declaration {
                env.declared_attackers = d.decl.attackers().collect();
                env.declared_victims = d.decl.victims().collect();
                env.declared_privileged = d.decl.privileged_identities().collect();
            }
            for raw in rules::check_test(project, &suite, &tr, &env) {
                add(raw, &suite.contract.name, &test.name);
            }
        }
    }
    if let Some(d) = declaration {
        for raw in d.decl.check(DECLARATION_UNIT, d.text) {
            add(raw, "", "");
        }
    }

    let mut findings: Vec<Finding> = merged.into_values().collect();
    findings.sort_by(|a, b| {
        (&a.file, a.line, a.column, a.rule).cmp(&(&b.file, b.line, b.column, b.rule))
    });
    for f in &findings {
        if !f.is_active() {
            summary.suppressed += 1;
            continue;
        }
        match f.severity {
            Severity::Error => summary.errors += 1,
            Severity::Warn => summary.warnings += 1,
            Severity::Note => summary.notes += 1,
        }
    }
    let parse_errors = project
        .units
        .iter()
        .flat_map(|u| {
            u.errors.iter().map(|m| ParseError {
                file: u.path.clone(),
                message: m.clone(),
            })
        })
        .collect();
    Report {
        summary,
        declaration: declaration.map(|d| d.path.clone()),
        findings,
        parse_errors,
    }
}
