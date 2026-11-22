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

/// The largest single source file this program reads. A PoC is a test, not a
/// vendored tree, and the limit is checked from metadata before the read, so
/// an oversized file is refused without allocating it.
pub const MAX_SOURCE_FILE_BYTES: u64 = 8 * 1024 * 1024;
/// The largest total source text read in one run.
pub const MAX_SOURCE_TOTAL_BYTES: u64 = 128 * 1024 * 1024;
/// The largest number of source files read in one run.
pub const MAX_SOURCE_FILES: usize = 20_000;
/// How deep directory discovery goes.
pub const MAX_WALK_DEPTH: usize = 64;
/// The largest `--report`, `--claims`, `--declaration` or `--config` file.
pub const MAX_INPUT_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug)]
pub enum LoadError {
    Missing(PathBuf),
    Io(PathBuf, std::io::Error),
    FileTooLarge {
        path: PathBuf,
        bytes: u64,
        limit: u64,
    },
    TotalTooLarge {
        bytes: u64,
        limit: u64,
    },
    TooManyFiles {
        limit: usize,
    },
    TooDeep {
        path: PathBuf,
        limit: usize,
    },
    InputTooLarge {
        path: PathBuf,
        bytes: u64,
        limit: u64,
    },
    NotRegular {
        path: PathBuf,
    },
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::Missing(p) => write!(f, "{}: no such file or directory", p.display()),
            LoadError::Io(p, e) => write!(f, "{}: {e}", p.display()),
            LoadError::FileTooLarge { path, bytes, limit } => write!(
                f,
                "{}: {bytes} bytes is over the {limit}-byte per-file limit; it was not read",
                path.display()
            ),
            LoadError::TotalTooLarge { bytes, limit } => write!(
                f,
                "the sources total {bytes} bytes, over the {limit}-byte total limit; the run was not started"
            ),
            LoadError::TooManyFiles { limit } => write!(
                f,
                "more than {limit} source files; that is the per-run file limit"
            ),
            LoadError::TooDeep { path, limit } => write!(
                f,
                "{}: directory nesting is deeper than the {limit}-level depth limit",
                path.display()
            ),
            LoadError::InputTooLarge { path, bytes, limit } => write!(
                f,
                "{}: {bytes} bytes is over the {limit}-byte input limit; it was not read",
                path.display()
            ),
            LoadError::NotRegular { path } => write!(
                f,
                "{}: not a regular file; a report, claims, declaration or config input must be a regular file (a FIFO could block, and a symlink can change after the check)",
                path.display()
            ),
        }
    }
}

impl std::error::Error for LoadError {}

/// Reads a `--report`, `--claims`, `--declaration` or `--config` file after
/// checking its size from metadata, so a huge input is refused by name rather
/// than read whole.
///
/// Only regular files are read, and the type is checked with
/// `symlink_metadata`: a FIFO has length zero and `read_to_string` would
/// block forever on it, and a symlink can point at a different file each
/// time. Symlinks are refused too; `docs/limitations.md` states the policy.
/// The check is not atomic with the open: a path swapped in between can still
/// block or read a different file, so these inputs must come from a trusted
/// location.
pub fn read_input(path: &Path) -> Result<String, LoadError> {
    let meta = std::fs::symlink_metadata(path).map_err(|e| LoadError::Io(path.to_path_buf(), e))?;
    if !meta.file_type().is_file() {
        return Err(LoadError::NotRegular {
            path: path.to_path_buf(),
        });
    }
    if meta.len() > MAX_INPUT_BYTES {
        return Err(LoadError::InputTooLarge {
            path: path.to_path_buf(),
            bytes: meta.len(),
            limit: MAX_INPUT_BYTES,
        });
    }
    std::fs::read_to_string(path).map_err(|e| LoadError::Io(path.to_path_buf(), e))
}

fn file_len(path: &Path) -> Result<u64, LoadError> {
    Ok(std::fs::metadata(path)
        .map_err(|e| LoadError::Io(path.to_path_buf(), e))?
        .len())
}

fn add_file(
    path: &Path,
    len: u64,
    out: &mut Vec<PathBuf>,
    total: &mut u64,
) -> Result<(), LoadError> {
    if len > MAX_SOURCE_FILE_BYTES {
        return Err(LoadError::FileTooLarge {
            path: path.to_path_buf(),
            bytes: len,
            limit: MAX_SOURCE_FILE_BYTES,
        });
    }
    if out.len() >= MAX_SOURCE_FILES {
        return Err(LoadError::TooManyFiles {
            limit: MAX_SOURCE_FILES,
        });
    }
    *total = total.saturating_add(len);
    if *total > MAX_SOURCE_TOTAL_BYTES {
        return Err(LoadError::TotalTooLarge {
            bytes: *total,
            limit: MAX_SOURCE_TOTAL_BYTES,
        });
    }
    out.push(path.to_path_buf());
    Ok(())
}

/// Every Solidity, Rust and TypeScript or JavaScript file under `paths`,
/// sorted, with dependency and build directories skipped. A path given explicitly is always included, even if it
/// sits inside a skipped directory.
pub fn discover(paths: &[PathBuf]) -> Result<Vec<PathBuf>, LoadError> {
    let mut out = Vec::new();
    let mut total = 0u64;
    for path in paths {
        if !path.exists() {
            return Err(LoadError::Missing(path.clone()));
        }
        if path.is_file() {
            add_file(path, file_len(path)?, &mut out, &mut total)?;
        } else {
            walk(path, 1, &mut out, &mut total)?;
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}

fn walk(
    dir: &Path,
    depth: usize,
    out: &mut Vec<PathBuf>,
    total: &mut u64,
) -> Result<(), LoadError> {
    if depth > MAX_WALK_DEPTH {
        return Err(LoadError::TooDeep {
            path: dir.to_path_buf(),
            limit: MAX_WALK_DEPTH,
        });
    }
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
            walk(&path, depth + 1, out, total)?;
        } else if file_type.is_file() && is_source(&name) {
            let len = entry
                .metadata()
                .map_err(|e| LoadError::Io(path.clone(), e))?
                .len();
            add_file(&path, len, out, total)?;
        }
    }
    Ok(())
}

fn extension(name: &str) -> Option<String> {
    Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
}

fn is_source(name: &str) -> bool {
    matches!(extension(name).as_deref(), Some("sol" | "rs" | "move"))
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
    match extension(path).as_deref() {
        Some("rs") => Some(crate::solana::parse_rust::parse_source(path, source)),
        Some("move") => Some(parse_move(path, source)),
        _ if crate::solana::parse_ts::is_script_path(path) => {
            crate::solana::parse_ts::parse_source(path, source)
        }
        _ => Some(parse_source(path, source)),
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
    /// A file over the per-file limit, or a set over the total limit, is
    /// refused from metadata before it is read.
    pub fn load(files: &[PathBuf], base: &Path) -> Result<Project, LoadError> {
        let mut units = Vec::with_capacity(files.len().min(MAX_SOURCE_FILES));
        let mut total = 0u64;
        for file in files {
            let len = file_len(file)?;
            if len > MAX_SOURCE_FILE_BYTES {
                return Err(LoadError::FileTooLarge {
                    path: file.clone(),
                    bytes: len,
                    limit: MAX_SOURCE_FILE_BYTES,
                });
            }
            total = total.saturating_add(len);
            if total > MAX_SOURCE_TOTAL_BYTES {
                return Err(LoadError::TotalTooLarge {
                    bytes: total,
                    limit: MAX_SOURCE_TOTAL_BYTES,
                });
            }
            let bytes = std::fs::read(file).map_err(|e| LoadError::Io(file.clone(), e))?;
            // The file can grow between the metadata check and the read.
            if bytes.len() as u64 > MAX_SOURCE_FILE_BYTES {
                return Err(LoadError::FileTooLarge {
                    path: file.clone(),
                    bytes: bytes.len() as u64,
                    limit: MAX_SOURCE_FILE_BYTES,
                });
            }
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
                // A set for the duplicate check: scanning `tests` for every
                // function made suite construction quadratic in the number
                // of test functions in one contract.
                let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
                for l in &linear {
                    for f in &l.contract.functions {
                        if is_test_function(f) && seen.insert(f.name.as_str()) {
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
    fn source_extensions_are_case_insensitive() {
        for name in ["A.SOL", "x.Rs", "m.Move", "t.TS", "t.MTS", "t.Js"] {
            assert!(is_source(name), "{name}");
        }
        assert!(!is_source("t.d.ts"));
        assert!(!is_source("t.D.TS"));
        assert!(!is_source("notes.md"));
    }

    #[test]
    fn display_paths_are_relative_with_forward_slashes() {
        let base = Path::new("root");
        assert_eq!(
            display_path(&Path::new("root").join("test").join("A.t.sol"), base),
            "test/A.t.sol"
        );
    }

    fn make_file_symlink(target: &Path, link: &Path) -> bool {
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(target, link).is_ok()
        }
        #[cfg(windows)]
        {
            std::os::windows::fs::symlink_file(target, link).is_ok()
        }
    }

    #[test]
    fn policy_inputs_must_be_regular_files() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("finding.md");
        std::fs::write(&file, "text\n").unwrap();
        assert_eq!(read_input(&file).unwrap(), "text\n");

        let dir = tmp.path().join("a-directory");
        std::fs::create_dir(&dir).unwrap();
        let err = read_input(&dir).unwrap_err();
        assert!(err.to_string().contains("not a regular file"), "{err}");

        let link = tmp.path().join("link.md");
        if make_file_symlink(&file, &link) {
            let err = read_input(&link).unwrap_err();
            assert!(err.to_string().contains("not a regular file"), "{err}");
        } else {
            eprintln!("symlinks cannot be created here; the symlink case is skipped");
        }
    }

    #[test]
    fn the_source_file_count_and_total_are_capped() {
        let dummy = PathBuf::from("x.sol");
        let mut out = Vec::new();
        let mut total = 0u64;
        for _ in 0..MAX_SOURCE_FILES {
            add_file(&dummy, 0, &mut out, &mut total).unwrap();
        }
        let err = add_file(&dummy, 0, &mut out, &mut total).unwrap_err();
        assert!(
            err.to_string().contains(&MAX_SOURCE_FILES.to_string()),
            "{err}"
        );

        let mut out = Vec::new();
        let mut total = 0u64;
        // 16 files of 8 MiB reach the 128 MiB total exactly; the next one
        // exceeds it without any single file exceeding the per-file cap.
        for _ in 0..16 {
            add_file(&dummy, MAX_SOURCE_FILE_BYTES, &mut out, &mut total).unwrap();
        }
        let err = add_file(&dummy, MAX_SOURCE_FILE_BYTES, &mut out, &mut total).unwrap_err();
        assert!(
            err.to_string()
                .contains(&MAX_SOURCE_TOTAL_BYTES.to_string()),
            "{err}"
        );
    }

    #[test]
    fn a_directory_past_the_depth_limit_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let mut out = Vec::new();
        let mut total = 0u64;
        let err = walk(tmp.path(), MAX_WALK_DEPTH + 1, &mut out, &mut total).unwrap_err();
        assert!(
            err.to_string().contains(&MAX_WALK_DEPTH.to_string()),
            "{err}"
        );
    }
}
