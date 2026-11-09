//! Source discovery, loading, and inheritance resolution.

use crate::ir::{Contract, ContractKind, FnKind, Function, Unit};
use crate::parse::parse_source;
use std::fmt;
use std::path::{Path, PathBuf};

/// Directory names that never hold the tests under review: dependencies,
/// build output, and VCS metadata.
const SKIP_DIRS: &[&str] = &[
    "lib",
    "node_modules",
    "out",
    "cache",
    "broadcast",
    "target",
    "artifacts",
];

#[derive(Debug)]
pub enum LoadError {
    Missing(PathBuf),
    Io(PathBuf, std::io::Error),
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::Missing(p) => write!(f, "{}: no such file or directory", p.display()),
            LoadError::Io(p, e) => write!(f, "{}: {e}", p.display()),
        }
    }
}

impl std::error::Error for LoadError {}

/// Every Solidity, Rust and TypeScript or JavaScript file under `paths`,
/// sorted, with dependency and build directories skipped. A path given explicitly is always included, even if it
/// sits inside a skipped directory.
pub fn discover(paths: &[PathBuf]) -> Result<Vec<PathBuf>, LoadError> {
    let mut out = Vec::new();
    for path in paths {
        if !path.exists() {
            return Err(LoadError::Missing(path.clone()));
        }
        if path.is_file() {
            out.push(path.clone());
        } else {
            walk(path, &mut out)?;
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), LoadError> {
    let entries = std::fs::read_dir(dir).map_err(|e| LoadError::Io(dir.to_path_buf(), e))?;
    for entry in entries {
        let entry = entry.map_err(|e| LoadError::Io(dir.to_path_buf(), e))?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let file_type = entry
            .file_type()
            .map_err(|e| LoadError::Io(path.clone(), e))?;
        if file_type.is_dir() {
            if name.starts_with('.') || SKIP_DIRS.contains(&name.as_str()) {
                continue;
            }
            walk(&path, out)?;
        } else if file_type.is_file() && is_source(&name) {
            out.push(path);
        }
    }
    Ok(())
}

fn is_source(name: &str) -> bool {
    name.ends_with(".sol")
        || name.ends_with(".rs")
        || name.ends_with(".move")
        || crate::solana::parse_ts::is_script_path(name)
}

/// The path as displayed in output: relative to `base` when it is inside it,
/// always with forward slashes. Output never carries an absolute host path.
pub fn display_path(path: &Path, base: &Path) -> String {
    let rel = path
        .strip_prefix(base)
        .map(Path::to_path_buf)
        .unwrap_or_else(|_| path.to_path_buf());
    let text = rel.to_string_lossy().replace('\\', "/");
    text.strip_prefix("./").unwrap_or(&text).to_string()
}

/// Parses one source with the parser its extension calls for: `.rs` is Rust,
/// a script is TypeScript or JavaScript, everything else is Solidity. `None`
/// for a script that is not a Solana test: it imports no Solana package.
pub fn parse_any(path: &str, source: &str) -> Option<Unit> {
    if path.ends_with(".rs") {
        Some(crate::solana::parse_rust::parse_source(path, source))
    } else if path.ends_with(".move") {
        Some(parse_move(path, source))
    } else if crate::solana::parse_ts::is_script_path(path) {
        crate::solana::parse_ts::parse_source(path, source)
    } else {
        Some(parse_source(path, source))
    }
}

fn parse_move(path: &str, source: &str) -> Unit {
    let file = crate::move_lang::parse_source(source);
    Unit {
        path: path.to_string(),
        source: source.to_string(),
        contracts: Vec::new(),
        solana: None,
        move_file: Some(file),
        errors: Vec::new(),
    }
}

/// A set of parsed files: Solidity contracts that inherit from one another,
/// and Rust tests with the helpers they share.
#[derive(Debug, Default)]
pub struct Project {
    pub units: Vec<Unit>,
}

/// A contract together with the index of the file that defines it.
#[derive(Debug, Clone, Copy)]
pub struct Located<'p> {
    pub unit: usize,
    pub contract: &'p Contract,
}

/// A concrete contract with at least one test function.
#[derive(Debug, Clone)]
pub struct Suite<'p> {
    pub unit: usize,
    pub contract: &'p Contract,
    /// The contract first, then its bases, depth first, without repeats. This
    /// approximates Solidity's linearization closely enough to find the most
    /// derived definition of a function.
    pub linear: Vec<Located<'p>>,
    pub tests: Vec<&'p Function>,
}

impl<'p> Suite<'p> {
    /// The most derived definition of `name` with a body.
    pub fn function(&self, name: &str) -> Option<(usize, &'p Function)> {
        self.linear
            .iter()
            .enumerate()
            .find_map(|(i, l)| l.contract.function(name).map(|f| (i, f)))
    }

    /// The definition of `name` in the first base after position `after` of
    /// the linearization. This resolves `super.name()`.
    pub fn function_after(&self, after: usize, name: &str) -> Option<(usize, &'p Function)> {
        self.linear
            .iter()
            .enumerate()
            .skip(after + 1)
            .find_map(|(i, l)| l.contract.function(name).map(|f| (i, f)))
    }

    /// The position of the contract called `name` in the linearization.
    pub fn position_of(&self, name: &str) -> Option<usize> {
        self.linear.iter().position(|l| l.contract.name == name)
    }
}

impl Project {
    /// Parses every file. Read errors abort; parse errors are kept per file.
    pub fn load(files: &[PathBuf], base: &Path) -> Result<Project, LoadError> {
        let mut units = Vec::with_capacity(files.len());
        for file in files {
            let bytes = std::fs::read(file).map_err(|e| LoadError::Io(file.clone(), e))?;
            let source = String::from_utf8_lossy(&bytes);
            units.extend(parse_any(&display_path(file, base), &source));
        }
        Ok(Project { units })
    }

    pub fn from_sources(sources: &[(&str, &str)]) -> Project {
        Project {
            units: sources
                .iter()
                .filter_map(|(path, src)| parse_any(path, src))
                .collect(),
        }
    }

    /// Resolves a contract name as seen from file `from`: a definition in the
    /// same file wins, then the first definition in path order. Many PoC
    /// collections reuse one suite name in every file, so the same-file
    /// preference is what keeps them apart.
    pub fn resolve(&self, from: usize, name: &str) -> Option<Located<'_>> {
        let local = self.units[from]
            .contracts
            .iter()
            .find(|c| c.name == name)
            .map(|contract| Located {
                unit: from,
                contract,
            });
        local.or_else(|| {
            self.units.iter().enumerate().find_map(|(unit, u)| {
                u.contracts
                    .iter()
                    .find(|c| c.name == name)
                    .map(|contract| Located { unit, contract })
            })
        })
    }

    pub fn linearize<'p>(&'p self, unit: usize, contract: &'p Contract) -> Vec<Located<'p>> {
        let mut out: Vec<Located<'p>> = Vec::new();
        let mut stack = vec![Located { unit, contract }];
        // Depth-first, most derived first. Solidity lists bases from most base
        // to most derived, so they are pushed in written order and popped in
        // reverse.
        while let Some(next) = stack.pop() {
            if out.iter().any(|l| std::ptr::eq(l.contract, next.contract)) {
                continue;
            }
            for base in &next.contract.bases {
                if let Some(found) = self.resolve(next.unit, base) {
                    stack.push(found);
                }
            }
            out.push(next);
        }
        out
    }

    /// Concrete contracts that Foundry would run tests on.
    pub fn suites(&self) -> Vec<Suite<'_>> {
        let mut out = Vec::new();
        for (unit, u) in self.units.iter().enumerate() {
            for contract in &u.contracts {
                if contract.kind != ContractKind::Contract {
                    continue;
                }
                let linear = self.linearize(unit, contract);
                let mut tests: Vec<&Function> = Vec::new();
                for l in &linear {
                    for f in &l.contract.functions {
                        if is_test_function(f) && !tests.iter().any(|t| t.name == f.name) {
                            tests.push(f);
                        }
                    }
                }
                if !tests.is_empty() {
                    out.push(Suite {
                        unit,
                        contract,
                        linear,
                        tests,
                    });
                }
            }
        }
        out
    }

    /// Whether `ty` names a contract defined in file `unit`. A contract written
    /// next to the test is the PoC's own helper (an attack contract, a
    /// receiver), not the code under review.
    pub fn defined_in(&self, unit: usize, ty: &str) -> bool {
        let name = ty.trim();
        self.units[unit].contracts.iter().any(|c| c.name == name)
    }

    pub fn is_library(&self, name: &str) -> bool {
        self.units.iter().any(|u| {
            u.contracts
                .iter()
                .any(|c| c.name == name && c.kind == ContractKind::Library)
        })
    }
}

/// Foundry runs public or external functions whose name starts with `test`.
/// Invariant and fuzz-campaign entry points are out of scope for v0.1.
pub fn is_test_function(f: &Function) -> bool {
    f.kind == FnKind::Function
        && f.name.starts_with("test")
        && f.is_externally_callable()
        && f.body.is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linearization_puts_the_most_derived_definition_first() {
        let p = Project::from_sources(&[
            (
                "a.sol",
                "contract Base { function setUp() public virtual { a(); } }",
            ),
            (
                "b.sol",
                "contract T is Base { function setUp() public override { super.setUp(); } function test_a() public {} }",
            ),
        ]);
        let suites = p.suites();
        assert_eq!(suites.len(), 1);
        let s = &suites[0];
        assert_eq!(s.linear.len(), 2);
        let (pos, _) = s.function("setUp").unwrap();
        assert_eq!(pos, 0);
        let (base_pos, _) = s.function_after(pos, "setUp").unwrap();
        assert_eq!(base_pos, 1);
    }

    #[test]
    fn same_file_definition_wins_over_a_homonym_elsewhere() {
        let p = Project::from_sources(&[
            ("a.sol", "contract Helper { function x() public {} }"),
            (
                "b.sol",
                "contract Helper { function y() public {} } contract T is Helper { function test_a() public {} }",
            ),
        ]);
        let found = p.resolve(1, "Helper").unwrap();
        assert_eq!(found.unit, 1);
    }

    #[test]
    fn only_public_test_prefixed_functions_are_tests() {
        let p = Project::from_sources(&[(
            "t.sol",
            "contract T { function testA() public {} function testB() internal {} function invariant_c() public {} function helper() public {} }",
        )]);
        let suites = p.suites();
        let names: Vec<&str> = suites[0].tests.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["testA"]);
    }

    #[test]
    fn display_paths_are_relative_with_forward_slashes() {
        let base = Path::new("root");
        assert_eq!(
            display_path(&Path::new("root").join("test").join("A.t.sol"), base),
            "test/A.t.sol"
        );
    }
}
