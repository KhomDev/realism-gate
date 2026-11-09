//! RG20: names a PoC cites have to exist in a target tree at one commit.
//!
//! `check` does not open the protocol unless `--target` and `--target-commit`
//! are set. When they are, this reads that commit with git (no network) and
//! fails closed if the commit cannot be read. It does not decide the bug is
//! real. It only rejects a citation the tree does not contain.
//!
//! The same shape is what a viability-gate cite-check should use for a
//! finding's file and function: a path and a name at one pinned commit, not
//! a string in the report.

use crate::finding::Severity;
use crate::identity::Env;
use crate::ir::{Expr, Pos};
use crate::project::Project;
use crate::rules::{self, Raw};
use crate::trace::{self, CallClass};
use regex::Regex;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;

/// What `git ls-tree` and `git grep` saw at one commit.
#[derive(Debug, Clone)]
pub struct Index {
    pub commit: String,
    pub files: BTreeSet<String>,
    /// Contract, interface or library name, to the functions defined in its
    /// own body. Inherited functions are not in this set; a call is accepted
    /// when the name exists on any contract in the tree.
    pub contracts: BTreeMap<String, BTreeSet<String>>,
    pub functions: BTreeSet<String>,
}

/// Read `commit` from `root`. A missing git, a bad commit or a grep that
/// fails for a reason other than "no matches" is an error: the check does
/// not pass by skipping the tree.
pub fn load(root: &Path, commit: &str) -> Result<Index, String> {
    if commit.trim().is_empty() || commit.starts_with('-') {
        return Err("target commit must be a revision, not a flag".into());
    }
    let commit = git(
        root,
        &["rev-parse", "--verify", &format!("{commit}^{{commit}}")],
    )?;
    let commit = commit.trim().to_string();
    let listing = git(root, &["ls-tree", "-r", "--name-only", &commit])?;
    let files = listing
        .lines()
        .map(|l| l.trim().replace('\\', "/"))
        .filter(|l| !l.is_empty())
        .collect();
    let grep = Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "grep",
            "-I",
            "-n",
            "-E",
            r"^[[:space:]]*(abstract[[:space:]]+)?(contract|interface|library|function)[[:space:]]+[A-Za-z_][A-Za-z0-9_]*",
            &commit,
            "--",
            "*.sol",
        ])
        .output()
        .map_err(|e| format!("could not run git grep: {e}"))?;
    let grep_text = if grep.status.success() {
        String::from_utf8_lossy(&grep.stdout).into_owned()
    } else if grep.status.code() == Some(1) {
        String::new()
    } else {
        return Err(format!(
            "could not read Solidity definitions at {commit}: {}",
            String::from_utf8_lossy(&grep.stderr).trim()
        ));
    };
    let (contracts, functions) = parse_defs(&grep_text);
    Ok(Index {
        commit,
        files,
        contracts,
        functions,
    })
}

fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .map_err(|e| format!("could not run git: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

fn parse_defs(text: &str) -> (BTreeMap<String, BTreeSet<String>>, BTreeSet<String>) {
    let mut contracts: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut functions = BTreeSet::new();
    let mut current: BTreeMap<String, String> = BTreeMap::new();
    for line in text.lines() {
        let mut parts = line.splitn(4, ':');
        let _rev = parts.next();
        let (Some(path), Some(_line), Some(content)) = (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        let content = content.trim();
        if let Some(name) = def_name(content, &["contract", "interface", "library"]) {
            current.insert(path.to_string(), name.clone());
            contracts.entry(name).or_default();
        } else if let Some(name) = def_name(content, &["function"]) {
            functions.insert(name.clone());
            if let Some(contract) = current.get(path) {
                contracts.entry(contract.clone()).or_default().insert(name);
            }
        }
    }
    (contracts, functions)
}

fn def_name(content: &str, kinds: &[&str]) -> Option<String> {
    let mut words = content.split_whitespace();
    let mut first = words.next()?;
    if first == "abstract" {
        first = words.next()?;
    }
    if !kinds.contains(&first) {
        return None;
    }
    let raw = words.next()?;
    let name: String = raw
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();
    if name.is_empty() { None } else { Some(name) }
}

const SKIP_TYPES: &[&str] = &[
    "Test",
    "Script",
    "StdCheats",
    "StdUtils",
    "StdStorage",
    "StdAssertions",
    "StdInvariant",
    "Vm",
    "IERC20",
    "IERC721",
    "IERC165",
    "IERC4626",
    "IERC20Metadata",
];

/// ERC-20 style names a test calls even when the interface file is not in
/// the protocol tree. A name that is not in this list and not in the tree
/// is the fabricated call this rule exists for.
const SKIP_METHODS: &[&str] = &[
    "balanceOf",
    "transfer",
    "transferFrom",
    "approve",
    "allowance",
    "totalSupply",
    "decimals",
    "symbol",
    "name",
    "owner",
    "getOwner",
];

fn import_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"(?m)import\s+(?:[^;"']*\s+from\s+)?["']([^"']+)["']"#).expect("import")
    })
}

fn vendor_import(path: &str) -> bool {
    let p = path.replace('\\', "/");
    p.contains("forge-std")
        || p.contains("openzeppelin")
        || p.contains("solmate")
        || p.contains("ds-test")
        || p.contains("node_modules")
        || p.starts_with('@')
}

fn path_exists(index: &Index, cited: &str) -> bool {
    let cited = cited.trim().trim_start_matches("./").replace('\\', "/");
    index.files.contains(&cited)
        || index
            .files
            .iter()
            .any(|f| f == &cited || f.ends_with(&format!("/{cited}")))
}

fn defined_in_project(project: &Project, name: &str) -> bool {
    project
        .units
        .iter()
        .any(|u| u.contracts.iter().any(|c| c.name == name))
}

fn type_name(ty: &str) -> Option<&str> {
    let ty = ty.trim();
    if ty.is_empty()
        || ty.contains("mapping")
        || ty.starts_with("uint")
        || ty.starts_with("int")
        || ty.starts_with("bytes")
        || ty.starts_with("address")
        || ty.starts_with("bool")
        || ty.starts_with("string")
    {
        return None;
    }
    let name = ty.split([' ', '[', '(']).next()?;
    if !name.chars().next()?.is_ascii_uppercase() {
        return None;
    }
    Some(name)
}

/// Findings for citations that are not in `index`.
pub fn check(project: &Project, index: &Index) -> Vec<Raw> {
    let mut out = Vec::new();
    for (unit, file) in project.units.iter().enumerate() {
        if !file.path.ends_with(".sol") {
            continue;
        }
        let has_test = file.contracts.iter().any(|c| {
            c.functions
                .iter()
                .any(|f| f.name.starts_with("test") && f.body.is_some())
        });
        if !has_test {
            continue;
        }
        for (i, line) in file.source.lines().enumerate() {
            for caps in import_re().captures_iter(line) {
                let path = &caps[1];
                if vendor_import(path) || path_exists(index, path) {
                    continue;
                }
                out.push(Raw {
                    rule: "RG20",
                    severity: Severity::Error,
                    unit,
                    pos: Pos {
                        line: i as u32 + 1,
                        col: 1,
                    },
                    message: format!(
                        "import `{path}` is not in the target at commit `{}`. The PoC cites a file that tree does not have.",
                        short_commit(&index.commit)
                    ),
                    evidence: rules::snippet(line.trim(), 120),
                });
            }
        }
    }

    let mut seen_types = BTreeSet::new();
    let mut seen_fns = BTreeSet::new();
    for suite in project.suites() {
        let helpers: BTreeSet<String> = project.units[suite.unit]
            .contracts
            .iter()
            .map(|c| c.name.clone())
            .collect();
        for test in &suite.tests {
            let tr = trace::build(project, &suite, test);
            let env = Env::new(project, suite.unit, &tr);
            for located in &suite.linear {
                for var in &located.contract.vars {
                    if let Some(name) = type_name(&var.ty) {
                        require_type(
                            TypeQuery {
                                index,
                                project,
                                helpers: &helpers,
                                unit: located.unit,
                                pos: var.pos,
                                name,
                                evidence: &var.ty,
                            },
                            &mut seen_types,
                            &mut out,
                        );
                    }
                }
            }
            for op in &tr.ops {
                let Some(call) = op.call() else { continue };
                if let CallClass::Deploy(ty) = &call.class {
                    require_type(
                        TypeQuery {
                            index,
                            project,
                            helpers: &helpers,
                            unit: op.unit,
                            pos: op.pos,
                            name: ty,
                            evidence: &call.expr.text,
                        },
                        &mut seen_types,
                        &mut out,
                    );
                }
                if call.class == CallClass::External && !call.method.is_empty() {
                    require_function(
                        FnQuery {
                            index,
                            env: &env,
                            unit: op.unit,
                            pos: op.pos,
                            method: &call.method,
                            receiver: call.receiver,
                            evidence: &call.expr.text,
                        },
                        &mut seen_fns,
                        &mut out,
                    );
                }
            }
        }
    }
    out
}

fn short_commit(commit: &str) -> String {
    commit.chars().take(12).collect()
}

struct TypeQuery<'a> {
    index: &'a Index,
    project: &'a Project,
    helpers: &'a BTreeSet<String>,
    unit: usize,
    pos: Pos,
    name: &'a str,
    evidence: &'a str,
}

fn require_type(q: TypeQuery<'_>, seen: &mut BTreeSet<(usize, String)>, out: &mut Vec<Raw>) {
    if SKIP_TYPES.contains(&q.name)
        || q.helpers.contains(q.name)
        || defined_in_project(q.project, q.name)
    {
        return;
    }
    if q.index.contracts.contains_key(q.name) || !seen.insert((q.unit, q.name.to_string())) {
        return;
    }
    out.push(Raw {
        rule: "RG20",
        severity: Severity::Error,
        unit: q.unit,
        pos: q.pos,
        message: format!(
            "contract `{}` is not in the target at commit `{}`.",
            q.name,
            short_commit(&q.index.commit)
        ),
        evidence: rules::snippet(q.evidence, 120),
    });
}

struct FnQuery<'a> {
    index: &'a Index,
    env: &'a Env<'a>,
    unit: usize,
    pos: Pos,
    method: &'a str,
    receiver: Option<&'a Expr>,
    evidence: &'a str,
}

fn require_function(q: FnQuery<'_>, seen: &mut BTreeSet<(usize, u32, String)>, out: &mut Vec<Raw>) {
    if SKIP_METHODS.contains(&q.method) || q.index.functions.contains(q.method) {
        return;
    }
    if q.method.starts_with("assert") || q.method.starts_with("expect") {
        return;
    }
    let typed = q
        .receiver
        .and_then(|r| r.as_ident())
        .and_then(|id| q.env.types.get(id))
        .map(String::as_str);
    if let Some(ty) = typed.filter(|ty| q.index.contracts.contains_key(*ty)) {
        let has = q
            .index
            .contracts
            .get(ty)
            .is_some_and(|fns| fns.contains(q.method));
        if has {
            return;
        }
    }
    if !seen.insert((q.unit, q.pos.line, q.method.to_string())) {
        return;
    }
    out.push(Raw {
        rule: "RG20",
        severity: Severity::Error,
        unit: q.unit,
        pos: q.pos,
        message: format!(
            "function `{}` is not defined in the target at commit `{}`.",
            q.method,
            short_commit(&q.index.commit)
        ),
        evidence: rules::snippet(q.evidence, 120),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn definitions_are_grouped_by_contract() {
        let text = "abc:src/Vault.sol:1:contract Vault {\nabc:src/Vault.sol:4:    function withdraw(uint256) external {}\nabc:src/Vault.sol:8:interface I {\n";
        let (contracts, functions) = parse_defs(text);
        assert!(functions.contains("withdraw"));
        assert!(contracts["Vault"].contains("withdraw"));
        assert!(contracts.contains_key("I"));
    }
}
