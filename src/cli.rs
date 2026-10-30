//! Command-line interface.
//!
//! Exit codes: 0 when no active finding reaches `--fail-on`, 1 when one does,
//! 2 on a usage, input or declaration error.

use crate::declaration::{self, Declaration};
use crate::engine::{self, LoadedDeclaration, Options};
use crate::finding::Severity;
use crate::project::{self, Project};
use crate::render::{self, text::TextOptions};
use crate::rules::{self, RULES};
use clap::{Parser, Subcommand, ValueEnum};
use regex::Regex;
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(
    name = "realism-gate",
    version,
    about = "Checks Foundry PoC tests for what makes a passing PoC prove less than it claims.",
    long_about = "Checks Foundry proof-of-concept tests for what makes a passing PoC prove less \
                  than it claims: privileged pranks, state written with cheatcodes, assertions \
                  that cannot fail, measurements without a baseline, unpinned forks.\n\n\
                  It reads the Solidity; it never runs it and never touches the network."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Check test files or directories (default: ./test if it exists, else .).
    Check(CheckArgs),
    /// List the rules.
    Rules {
        #[arg(long, value_enum, default_value_t = ListFormat::Text)]
        format: ListFormat,
    },
    /// Explain one rule: why it exists, how to fix it, how it can be wrong.
    Explain {
        /// A rule id (RG04) or slug (storage-write).
        rule: String,
        /// Print the Markdown page published under docs/rules/.
        #[arg(long)]
        markdown: bool,
    },
    /// Write a commented realism.yaml template to the current directory.
    Init {
        /// Overwrite an existing realism.yaml.
        #[arg(long)]
        force: bool,
    },
}

#[derive(clap::Args, Debug)]
pub struct CheckArgs {
    /// Files or directories to check. Directories are searched for .sol files;
    /// lib/, node_modules/, out/, cache/ and hidden directories are skipped.
    pub paths: Vec<PathBuf>,
    /// The attacker-model declaration. Defaults to ./realism.yaml if present.
    #[arg(long, value_name = "FILE")]
    pub declaration: Option<PathBuf>,
    /// Ignore ./realism.yaml even if it exists.
    #[arg(long, conflicts_with = "declaration")]
    pub no_declaration: bool,
    #[arg(long, value_enum, default_value_t = Format::Text)]
    pub format: Format,
    /// Write the report to a file instead of standard output.
    #[arg(long, short, value_name = "FILE")]
    pub output: Option<PathBuf>,
    /// Exit with status 1 if an active finding is at or above this level.
    #[arg(long, value_enum, default_value_t = FailOn::Error)]
    pub fail_on: FailOn,
    /// Run only these rules (comma-separated ids or slugs).
    #[arg(long, value_delimiter = ',', value_name = "RULES")]
    pub rules: Vec<String>,
    /// Skip these rules (comma-separated ids or slugs).
    #[arg(long, value_delimiter = ',', value_name = "RULES")]
    pub skip: Vec<String>,
    /// Only check tests whose name matches this regular expression.
    #[arg(long, value_name = "REGEX")]
    pub match_test: Option<String>,
    /// Only check test contracts whose name matches this regular expression.
    #[arg(long, value_name = "REGEX")]
    pub match_contract: Option<String>,
    /// Also print findings silenced by an allow comment.
    #[arg(long)]
    pub show_suppressed: bool,
    #[arg(long, value_enum, default_value_t = ColorChoice::Auto)]
    pub color: ColorChoice,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Text,
    Json,
    Sarif,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListFormat {
    Text,
    Json,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailOn {
    Error,
    Warn,
    Note,
    Never,
}

impl FailOn {
    fn level(self) -> Option<Severity> {
        match self {
            FailOn::Error => Some(Severity::Error),
            FailOn::Warn => Some(Severity::Warn),
            FailOn::Note => Some(Severity::Note),
            FailOn::Never => None,
        }
    }
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorChoice {
    Auto,
    Always,
    Never,
}

/// Entry point shared by both binaries.
pub fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("realism-gate: {message}");
            ExitCode::from(2)
        }
    }
}

pub fn run(cli: Cli) -> Result<ExitCode, String> {
    match cli.command {
        Command::Check(args) => check(args),
        Command::Rules { format } => {
            print!("{}", list_rules(format));
            Ok(ExitCode::SUCCESS)
        }
        Command::Explain { rule, markdown } => {
            let r = rules::rule(&rule)
                .ok_or_else(|| format!("unknown rule `{rule}`; `realism-gate rules` lists them"))?;
            if markdown {
                print!("{}", rules::catalog::markdown(r));
            } else {
                print!("{}", explain_text(r));
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::Init { force } => {
            let path = Path::new(declaration::FILE_NAME);
            if path.exists() && !force {
                return Err(format!(
                    "{} already exists; pass --force to overwrite it",
                    declaration::FILE_NAME
                ));
            }
            std::fs::write(path, declaration::TEMPLATE)
                .map_err(|e| format!("{}: {e}", declaration::FILE_NAME))?;
            println!("wrote {}", declaration::FILE_NAME);
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn resolve_rules(names: &[String]) -> Result<std::collections::BTreeSet<String>, String> {
    names
        .iter()
        .filter(|n| !n.trim().is_empty())
        .map(|n| {
            rules::rule(n)
                .map(|r| r.id.to_string())
                .ok_or_else(|| format!("unknown rule `{n}`; `realism-gate rules` lists them"))
        })
        .collect()
}

fn compile(re: &Option<String>, flag: &str) -> Result<Option<Regex>, String> {
    re.as_deref()
        .map(|r| Regex::new(r).map_err(|e| format!("{flag}: {e}")))
        .transpose()
}

fn check(args: CheckArgs) -> Result<ExitCode, String> {
    let cwd = std::env::current_dir().map_err(|e| format!("current directory: {e}"))?;
    let paths = if args.paths.is_empty() {
        if Path::new("test").is_dir() {
            vec![PathBuf::from("test")]
        } else {
            vec![PathBuf::from(".")]
        }
    } else {
        args.paths.clone()
    };
    let files = project::discover(&paths).map_err(|e| e.to_string())?;
    if files.is_empty() {
        return Err(format!(
            "no .sol files found under {}",
            paths
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    let project = Project::load(&files, &cwd).map_err(|e| e.to_string())?;

    let decl_path = match (&args.declaration, args.no_declaration) {
        (Some(p), _) => Some(p.clone()),
        (None, false) if Path::new(declaration::FILE_NAME).is_file() => {
            Some(PathBuf::from(declaration::FILE_NAME))
        }
        _ => None,
    };
    let decl_text = match &decl_path {
        Some(p) => Some(std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?),
        None => None,
    };
    let decl = match (&decl_path, &decl_text) {
        (Some(p), Some(t)) => {
            Some(Declaration::parse(t).map_err(|e| format!("{}: {e}", p.display()))?)
        }
        _ => None,
    };
    let loaded = match (&decl, &decl_path, &decl_text) {
        (Some(d), Some(p), Some(t)) => Some(LoadedDeclaration {
            decl: d,
            path: project::display_path(p, &cwd),
            text: t,
        }),
        _ => None,
    };

    let opts = Options {
        only: resolve_rules(&args.rules)?,
        skip: resolve_rules(&args.skip)?,
        match_test: compile(&args.match_test, "--match-test")?,
        match_contract: compile(&args.match_contract, "--match-contract")?,
    };
    let report = engine::analyze(&project, loaded.as_ref(), &opts);

    let to_terminal = args.output.is_none() && std::io::stdout().is_terminal();
    let color = match args.color {
        ColorChoice::Always => true,
        ColorChoice::Never => false,
        ColorChoice::Auto => to_terminal && std::env::var_os("NO_COLOR").is_none(),
    };
    let rendered = match args.format {
        Format::Text => render::text::render(
            &report,
            TextOptions {
                color,
                show_suppressed: args.show_suppressed,
                fail_on: args.fail_on.level(),
            },
        ),
        Format::Json => render::json::render(&report),
        Format::Sarif => render::sarif::render(&report),
    };
    match &args.output {
        Some(path) => {
            std::fs::write(path, rendered).map_err(|e| format!("{}: {e}", path.display()))?
        }
        None => {
            let mut stdout = std::io::stdout().lock();
            // A closed pipe (`| head`) is not an error worth reporting.
            let _ = stdout.write_all(rendered.as_bytes());
        }
    }

    // Nothing checked is not the same as nothing found.
    if report.summary.suites == 0 {
        return Err(if report.parse_errors.is_empty() {
            "no test suite found: no concrete contract with a public test* function matched"
                .to_string()
        } else {
            format!(
                "no test suite found ({} parse error(s); see the report)",
                report.parse_errors.len()
            )
        });
    }
    let failed = args.fail_on.level().is_some_and(|l| report.fails(l));
    Ok(if failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    })
}

fn list_rules(format: ListFormat) -> String {
    match format {
        ListFormat::Text => {
            let mut out = String::new();
            for r in RULES {
                out.push_str(&format!(
                    "{}  {:<5}  {:<32} {}\n",
                    r.id,
                    r.severity.as_str(),
                    r.slug,
                    r.title
                ));
            }
            out
        }
        ListFormat::Json => {
            let rows: Vec<serde_json::Value> = RULES
                .iter()
                .map(|r| {
                    serde_json::json!({
                        "id": r.id,
                        "slug": r.slug,
                        "title": r.title,
                        "severity": r.severity.as_str(),
                        "severity_note": r.severity_note,
                        "summary": r.summary,
                        "url": render::rule_url(r.id),
                    })
                })
                .collect();
            let mut out = serde_json::to_string_pretty(&rows).expect("rules serialize");
            out.push('\n');
            out
        }
    }
}

fn explain_text(r: &rules::RuleInfo) -> String {
    let mut out = format!(
        "{} {}: {}\nseverity: {} ({})\n\n{}\n\nWhy it matters\n  {}\n\nHow to fix it\n  {}\n",
        r.id,
        r.slug,
        r.title,
        r.severity.as_str(),
        r.severity_note,
        r.summary,
        r.why,
        r.fix
    );
    if !r.bad.is_empty() {
        out.push_str("\nFires on\n");
        for line in r.bad.lines() {
            out.push_str(&format!("  {line}\n"));
        }
    }
    if !r.good.is_empty() {
        out.push_str("\nDoes not fire on\n");
        for line in r.good.lines() {
            out.push_str(&format!("  {line}\n"));
        }
    }
    if !r.false_positives.is_empty() {
        out.push_str("\nKnown false-positive modes\n");
        for fp in r.false_positives {
            out.push_str(&format!("  - {fp}\n"));
        }
    }
    out.push_str(&format!("\n{}\n", render::rule_url(r.id)));
    out
}
