//! Optional project configuration, `realism-gate.toml` in the directory
//! realism-gate runs in:
//!
//! ```toml
//! # Globs, relative to that directory. Tests in matching files are not
//! # checked and findings in them are not reported. Base contracts in them
//! # are still read, so inheritance keeps working.
//! exclude = ["test/legacy/**", "test/fork/Archive.t.sol"]
//!
//! # Per-rule severity: error, warn, note, or off (do not run the rule).
//! # A level replaces the severity of every finding of that rule.
//! [rules]
//! RG11 = "off"
//! RG05 = "error"
//! ```
//!
//! Command-line flags win: `--rules` runs a rule the file turns off,
//! `--severity RULE=LEVEL` replaces a level the file sets, and `--exclude`
//! adds patterns to the file's.

use crate::finding::Severity;
use crate::rules;
use regex::Regex;
use serde::Deserialize;
use std::collections::BTreeMap;

pub const FILE_NAME: &str = "realism-gate.toml";

/// What a rule is set to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Off,
    Set(Severity),
}

impl Level {
    pub fn parse(text: &str) -> Result<Level, String> {
        match text.trim().to_ascii_lowercase().as_str() {
            "off" => Ok(Level::Off),
            "error" => Ok(Level::Set(Severity::Error)),
            "warn" | "warning" => Ok(Level::Set(Severity::Warn)),
            "note" => Ok(Level::Set(Severity::Note)),
            other => Err(format!(
                "unknown level `{other}`; use error, warn, note or off"
            )),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    #[serde(default)]
    exclude: Vec<String>,
    #[serde(default)]
    rules: BTreeMap<String, String>,
}

#[derive(Debug, Default, Clone)]
pub struct Config {
    pub exclude: Vec<String>,
    /// Rule id -> level.
    pub rules: BTreeMap<String, Level>,
}

impl Config {
    pub fn parse(text: &str) -> Result<Config, String> {
        let raw: Raw = toml::from_str(text).map_err(|e| e.to_string())?;
        let mut out = Config {
            exclude: raw.exclude,
            rules: BTreeMap::new(),
        };
        for pattern in &out.exclude {
            glob(pattern)?;
        }
        for (name, level) in raw.rules {
            let rule = rules::rule(&name)
                .ok_or_else(|| format!("unknown rule `{name}`; `realism-gate rules` lists them"))?;
            let level = Level::parse(&level).map_err(|e| format!("rules.{name}: {e}"))?;
            out.rules.insert(rule.id.to_string(), level);
        }
        Ok(out)
    }
}

/// `RULE=LEVEL`, as given to `--severity`.
pub fn parse_override(text: &str) -> Result<(String, Level), String> {
    let (name, level) = text
        .split_once('=')
        .ok_or_else(|| format!("--severity {text}: expected RULE=LEVEL"))?;
    let rule = rules::rule(name.trim())
        .ok_or_else(|| format!("--severity {text}: unknown rule `{}`", name.trim()))?;
    let level = Level::parse(level).map_err(|e| format!("--severity {text}: {e}"))?;
    Ok((rule.id.to_string(), level))
}

/// A glob over displayed paths (forward slashes, relative to the working
/// directory): `**` crosses directories, `*` and `?` do not. A pattern also
/// matches everything under it, so `test/legacy` excludes that directory.
pub fn glob(pattern: &str) -> Result<Regex, String> {
    let p = pattern
        .trim()
        .trim_start_matches("./")
        .trim_end_matches('/');
    if p.is_empty() {
        return Err("an empty exclude pattern".to_string());
    }
    let mut re = String::from("^");
    let chars: Vec<char> = p.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '*' if chars.get(i + 1) == Some(&'*') => {
                if chars.get(i + 2) == Some(&'/') {
                    re.push_str("(?:.*/)?");
                    i += 3;
                } else {
                    re.push_str(".*");
                    i += 2;
                }
                continue;
            }
            '*' => re.push_str("[^/]*"),
            '?' => re.push_str("[^/]"),
            c => re.push_str(&regex::escape(&c.to_string())),
        }
        i += 1;
    }
    re.push_str("(?:/.*)?$");
    Regex::new(&re).map_err(|e| format!("exclude `{pattern}`: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rules_and_excludes() {
        let c = Config::parse(
            "exclude = [\"test/legacy/**\"]\n[rules]\nRG11 = \"off\"\nstorage-write = \"warn\"\n",
        )
        .unwrap();
        assert_eq!(c.exclude, vec!["test/legacy/**"]);
        assert_eq!(c.rules["RG11"], Level::Off);
        assert_eq!(c.rules["RG04"], Level::Set(Severity::Warn));
    }

    #[test]
    fn rejects_unknown_rules_levels_and_keys() {
        assert!(Config::parse("[rules]\nRG99 = \"off\"\n").is_err());
        assert!(Config::parse("[rules]\nRG01 = \"loud\"\n").is_err());
        assert!(Config::parse("excludes = []\n").is_err());
    }

    #[test]
    fn globs() {
        let g = glob("test/legacy/**").unwrap();
        assert!(g.is_match("test/legacy/a/B.t.sol"));
        assert!(!g.is_match("test/Legacy.t.sol"));
        let g = glob("**/*.archive.sol").unwrap();
        assert!(g.is_match("x.archive.sol"));
        assert!(g.is_match("test/a/x.archive.sol"));
        let g = glob("test/legacy").unwrap();
        assert!(g.is_match("test/legacy/A.t.sol"));
        assert!(!g.is_match("test/legacyx/A.t.sol"));
        let g = glob("test/*.t.sol").unwrap();
        assert!(g.is_match("test/A.t.sol"));
        assert!(!g.is_match("test/sub/A.t.sol"));
    }

    #[test]
    fn overrides() {
        assert_eq!(
            parse_override("rg05=error").unwrap(),
            ("RG05".to_string(), Level::Set(Severity::Error))
        );
        assert!(parse_override("RG05").is_err());
    }
}
