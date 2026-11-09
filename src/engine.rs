//! Runs the rules over a project and assembles the report.

use crate::declaration::Declaration;
use crate::finding::{Finding, Severity, Suppression};
use crate::identity::Env;
use crate::project::Project;
use crate::rules::{self, Lang, Raw, help};
use crate::solana;
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
    /// Rule id -> the severity every finding of that rule takes.
    pub severity: BTreeMap<String, Severity>,
    /// Displayed paths whose suites are not checked and whose findings are not
    /// reported. Their contracts are still read as bases.
    pub exclude: Vec<Regex>,
    /// When set, only findings in these files, or raised in a suite defined
    /// in one of them, are reported (`--changed-since`).
    pub only_files: Option<BTreeSet<String>>,
    /// Honor `realism-gate: allow` comments in the files under review.
    /// Default off: the repo under review cannot silence the gate. The author
    /// switch is `--trust-inline-allows`.
    pub trust_inline_allows: bool,
    /// Protocol tree and commit for RG20. Absent means the citation check
    /// does not run.
    pub citations: Option<crate::cite::Index>,
}

impl Options {
    fn enabled(&self, rule: &str) -> bool {
        (self.only.is_empty() || self.only.contains(rule)) && !self.skip.contains(rule)
    }

    fn excluded(&self, path: &str) -> bool {
        self.exclude.iter().any(|re| re.is_match(path))
    }

    fn in_scope(&self, file: &str, suite_file: &str) -> bool {
        self.only_files
            .as_ref()
            .is_none_or(|set| set.contains(file) || set.contains(suite_file))
    }
}

/// `--changed-since`: the revision and how many files differ from it.
#[derive(Debug, Clone, Serialize)]
pub struct ChangedSince {
    #[serde(rename = "ref")]
    pub git_ref: String,
    pub files: usize,
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
    /// Allow comments ignored because they give no reason. The findings they
    /// name stay active.
    pub suppressions_ignored: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub summary: Summary,
    pub declaration: Option<String>,
    /// The `realism-gate.toml` that was applied, if any.
    pub config: Option<String>,
    pub changed_since: Option<ChangedSince>,
    pub findings: Vec<Finding>,
    pub parse_errors: Vec<ParseError>,
    /// Policy files found inside the repo under review and not applied.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ignored_policy: Vec<String>,
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

    let mut add = |raw: Raw, suite: &str, suite_file: &str, test: &str, lang: Lang| {
        if !opts.enabled(raw.rule) {
            return;
        }
        let info = rules::rule(raw.rule).expect("every raw finding names a catalog rule");
        let (file, dirs): (String, &[Directive]) = if raw.unit == DECLARATION_UNIT {
            (declaration.map(|d| d.path.clone()).unwrap_or_default(), &[])
        } else {
            (project.units[raw.unit].path.clone(), &directives[raw.unit])
        };
        if opts.excluded(&file) || !opts.in_scope(&file, suite_file) {
            return;
        }
        let k = (file.clone(), raw.pos.line, raw.pos.col, raw.rule);
        if let Some(existing) = merged.get_mut(&k) {
            if !test.is_empty() && !existing.tests.iter().any(|t| t == test) {
                existing.tests.push(test.to_string());
            }
            return;
        }
        let directive = suppress::find(dirs, raw.rule, raw.pos.line);
        let (suppressed, ignored_suppression, ignored_reason) = match directive {
            Some(d) if !opts.trust_inline_allows => (
                None,
                Some(d.line),
                Some("allow comments in the repo under review are not policy".to_string()),
            ),
            Some(d) if !d.reason.is_empty() => (
                Some(Suppression {
                    line: d.line,
                    reason: d.reason.clone(),
                }),
                None,
                None,
            ),
            Some(d) => (
                None,
                Some(d.line),
                Some("the allow comment gives no reason".to_string()),
            ),
            None => (None, None, None),
        };
        merged.insert(
            k,
            Finding {
                rule: info.id,
                slug: info.slug,
                severity: opts.severity.get(info.id).copied().unwrap_or(raw.severity),
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
                help: help(info, lang),
                suppressed,
                ignored_suppression,
                ignored_reason,
            },
        );
    };

    for suite in project.suites() {
        if let Some(re) = &opts.match_contract
            && !re.is_match(&suite.contract.name)
        {
            continue;
        }
        let suite_file = project.units[suite.unit].path.as_str();
        if opts.excluded(suite_file) {
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
                env.deployment_evidence = d.decl.deployment_names().collect();
            }
            for raw in rules::check_test(project, &suite, &tr, &env) {
                add(
                    raw,
                    &suite.contract.name,
                    suite_file,
                    &test.name,
                    Lang::Solidity,
                );
            }
        }
    }

    let declared = solana::Declared {
        attackers: declaration
            .map(|d| d.decl.attackers().collect())
            .unwrap_or_default(),
        privileged: declaration
            .map(|d| d.decl.privileged_identities().collect())
            .unwrap_or_default(),
        victims: declaration
            .map(|d| d.decl.victims().collect())
            .unwrap_or_default(),
    };
    let scopes = solana::trace::Scopes::new(project);
    for suite in solana::suites(project) {
        if let Some(re) = &opts.match_contract
            && !re.is_match(&suite.name)
        {
            continue;
        }
        let suite_file = project.units[suite.unit].path.as_str();
        if opts.excluded(suite_file) {
            continue;
        }
        let mut counted = false;
        for test in &suite.tests {
            if let Some(re) = &opts.match_test
                && !re.is_match(&test.name)
            {
                continue;
            }
            // A test that drives no Solana harness is not a PoC.
            let Some(raws) = solana::check_test(project, &scopes, &suite, test, &declared) else {
                continue;
            };
            if !counted {
                summary.suites += 1;
                counted = true;
            }
            summary.tests += 1;
            for raw in raws {
                let lang = match suite.dialect {
                    solana::ir::Dialect::Rust => Lang::Rust,
                    solana::ir::Dialect::TypeScript => Lang::TypeScript,
                };
                add(raw, &suite.name, suite_file, &test.name, lang);
            }
        }
    }

    for (unit_index, unit) in project.units.iter().enumerate() {
        let Some(file) = &unit.move_file else {
            continue;
        };
        if file.tests.is_empty() {
            continue;
        }
        let suite_name = if file.module.is_empty() {
            unit.path
                .rsplit('/')
                .next()
                .unwrap_or(&unit.path)
                .trim_end_matches(".move")
                .to_string()
        } else {
            file.module.clone()
        };
        if let Some(re) = &opts.match_contract
            && !re.is_match(&suite_name)
        {
            continue;
        }
        if opts.excluded(&unit.path) {
            continue;
        }
        let mut counted = false;
        for test in &file.tests {
            if let Some(re) = &opts.match_test
                && !re.is_match(&test.name)
            {
                continue;
            }
            if !counted {
                summary.suites += 1;
                counted = true;
            }
            summary.tests += 1;
            for raw in crate::move_lang::check_test(unit_index, test) {
                add(raw, &suite_name, &unit.path, &test.name, Lang::Move);
            }
        }
    }

    if let Some(d) = declaration {
        for raw in d.decl.check(DECLARATION_UNIT, d.text) {
            add(raw, "", "", "", Lang::Solidity);
        }
    }

    if let Some(index) = &opts.citations {
        for raw in crate::cite::check(project, index) {
            let lang = Lang::Solidity;
            add(raw, "", "", "", lang);
        }
    }

    let mut findings: Vec<Finding> = merged.into_values().collect();
    findings.sort_by(|a, b| {
        (&a.file, a.line, a.column, a.rule).cmp(&(&b.file, b.line, b.column, b.rule))
    });
    for f in &findings {
        if f.ignored_suppression.is_some() {
            summary.suppressions_ignored += 1;
        }
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
        config: None,
        changed_since: None,
        findings,
        parse_errors,
        ignored_policy: Vec::new(),
    }
}
