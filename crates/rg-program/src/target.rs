//! A bounded, target-confined view of the tree the model reads.
//!
//! The program model answers what the audited program requires. A deployment
//! configuration, a workspace sibling or a `/etc` file is not evidence for it,
//! so every read goes through [`TargetRoot`]: a relative path with no parent
//! component, resolved under the canonical target root, with the final path
//! checked to still be under it. A symlink that points outside the root is
//! refused at that check, and the directory walk never follows one.

use crate::ProgramError;
use std::path::{Component, Path, PathBuf};

/// Directory names that are never walked for sources: dependencies, build
/// output and VCS metadata. Kept in step with `project.rs`'s list.
const SKIP_DIRS: &[&str] = &[
    "lib",
    "node_modules",
    "out",
    "cache",
    "broadcast",
    "target",
    "artifacts",
];

/// The largest single source file the model reads. Checked from metadata
/// before the read, so an oversized file is refused without allocating it.
pub const MAX_SOURCE_FILE_BYTES: u64 = 8 * 1024 * 1024;
/// The largest total source text read in one model.
pub const MAX_SOURCE_TOTAL_BYTES: u64 = 128 * 1024 * 1024;
/// The largest number of source files walked in one model.
pub const MAX_SOURCE_FILES: usize = 20_000;
/// How deep the directory walk goes.
pub const MAX_WALK_DEPTH: usize = 64;

/// The canonical target directory every read is confined to.
#[derive(Debug, Clone)]
pub struct TargetRoot {
    root: PathBuf,
    name: String,
}

/// The source files and IDL files found under a target root, in a stable
/// order, with notes for anything skipped.
#[derive(Debug, Default)]
pub(crate) struct Inventory {
    /// Rust sources, relative to the root, in scope for both Anchor and native
    /// detection.
    pub rust: Vec<String>,
    /// Solidity sources in scope: the contracts whose functions are entry
    /// points.
    pub solidity: Vec<String>,
    /// Solidity sources read only to resolve inherited contracts and
    /// modifiers (Foundry `lib/`).
    pub solidity_deps: Vec<String>,
    /// Anchor IDL files (`<dir>/target/idl/*.json`).
    pub idl: Vec<String>,
    /// Files skipped, and why. Part of the model's notes.
    pub notes: Vec<String>,
}

impl TargetRoot {
    /// Opens `root`. The path is canonicalised once, so every later prefix
    /// check compares canonical paths.
    pub fn new(root: &Path) -> Result<Self, ProgramError> {
        let canonical = std::fs::canonicalize(root).map_err(|source| ProgramError::Io {
            path: root.to_path_buf(),
            source,
        })?;
        if !canonical.is_dir() {
            return Err(ProgramError::Unsupported(format!(
                "{}: not a directory; --target must name a program checkout",
                root.display()
            )));
        }
        let name = canonical
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "target".to_string());
        Ok(Self {
            root: canonical,
            name,
        })
    }

    /// The target directory's own name. Serialised instead of the absolute
    /// path: output never carries a host path.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The root path. Not serialised; used by tests and by the walk.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Resolves a relative path under the root and proves it stays there.
    ///
    /// Rejects, before touching the filesystem: an empty path, an absolute
    /// path (including a Windows prefix or a drive-relative path), any `.` or
    /// `..` component, and any other component that is not a normal name. It
    /// then canonicalises the join - which resolves symlinks - and refuses the
    /// result unless it is still under the canonical root.
    pub fn resolve(&self, relative: &str) -> Result<PathBuf, ProgramError> {
        let rel = Path::new(relative);
        if relative.is_empty() {
            return Err(ProgramError::OutsideTarget {
                path: relative.to_string(),
                root: self.root.clone(),
            });
        }
        let mut parts: Vec<&std::ffi::OsStr> = Vec::new();
        for component in rel.components() {
            match component {
                Component::Normal(part) => parts.push(part),
                // RootDir, Prefix and ParentDir are escapes; CurDir is
                // ambiguous enough to refuse rather than normalise.
                Component::Prefix(_)
                | Component::RootDir
                | Component::CurDir
                | Component::ParentDir => {
                    return Err(ProgramError::OutsideTarget {
                        path: relative.to_string(),
                        root: self.root.clone(),
                    });
                }
            }
        }
        if parts.is_empty() {
            return Err(ProgramError::OutsideTarget {
                path: relative.to_string(),
                root: self.root.clone(),
            });
        }
        let joined = parts
            .iter()
            .fold(self.root.clone(), |path, part| path.join(part));
        let canonical = std::fs::canonicalize(&joined).map_err(|source| ProgramError::Io {
            path: joined.clone(),
            source,
        })?;
        if !canonical.starts_with(&self.root) {
            return Err(ProgramError::OutsideTarget {
                path: relative.to_string(),
                root: self.root.clone(),
            });
        }
        Ok(canonical)
    }

    /// Reads one source file as UTF-8 after the size check.
    ///
    /// An oversized file is refused by name from metadata; a file that changes
    /// size between the metadata check and the read is refused after the read.
    /// Non-UTF-8 bytes are refused: a source file the parser cannot decode is
    /// not evidence, and lossy decoding would feed the parser characters that
    /// are not in the file.
    pub fn read(&self, relative: &str) -> Result<String, ProgramError> {
        let full = self.resolve(relative)?;
        let meta = std::fs::symlink_metadata(&full).map_err(|source| ProgramError::Io {
            path: full.clone(),
            source,
        })?;
        if !meta.file_type().is_file() {
            return Err(ProgramError::NotRegular {
                path: relative.to_string(),
            });
        }
        if meta.len() > MAX_SOURCE_FILE_BYTES {
            return Err(ProgramError::InputTooLarge {
                path: relative.to_string(),
                bytes: meta.len(),
                limit: MAX_SOURCE_FILE_BYTES,
            });
        }
        let bytes = std::fs::read(&full).map_err(|source| ProgramError::Io {
            path: full.clone(),
            source,
        })?;
        if bytes.len() as u64 > MAX_SOURCE_FILE_BYTES {
            return Err(ProgramError::InputTooLarge {
                path: relative.to_string(),
                bytes: bytes.len() as u64,
                limit: MAX_SOURCE_FILE_BYTES,
            });
        }
        String::from_utf8(bytes).map_err(|_| ProgramError::NotUtf8 {
            path: relative.to_string(),
        })
    }

    /// Walks the target tree once, collecting Rust and Solidity sources and
    /// Anchor IDLs in a stable order.
    ///
    /// Skips dependency and build directories, hidden directories and every
    /// non-regular file (the walk never follows a symlink: `DirEntry`'s file
    /// type is the link's own). Foundry `lib/` is read for Solidity because a
    /// base contract's `onlyOwner` lives there, but its contracts are not
    /// entry points, so those paths are recorded as dependencies. An Anchor
    /// IDL lives in build output (`target/idl/`), so a directory named
    /// `target` is not walked but its `idl` child is read.
    pub(crate) fn inventory(&self) -> Result<Inventory, ProgramError> {
        let mut inventory = Inventory::default();
        let mut total = 0u64;
        let mut count = 0usize;
        self.walk(&self.root, "", 1, &mut inventory, &mut total, &mut count)?;
        inventory.rust.sort();
        inventory.rust.dedup();
        inventory.solidity.sort();
        inventory.solidity.dedup();
        inventory.solidity_deps.sort();
        inventory.solidity_deps.dedup();
        inventory.idl.sort();
        inventory.idl.dedup();
        Ok(inventory)
    }

    fn walk(
        &self,
        dir: &Path,
        prefix: &str,
        depth: usize,
        inventory: &mut Inventory,
        total: &mut u64,
        count: &mut usize,
    ) -> Result<(), ProgramError> {
        if depth > MAX_WALK_DEPTH {
            return Err(ProgramError::TooDeep {
                path: dir.to_path_buf(),
                limit: MAX_WALK_DEPTH,
            });
        }
        let entries = std::fs::read_dir(dir).map_err(|source| ProgramError::Io {
            path: dir.to_path_buf(),
            source,
        })?;
        // `read_dir` order is filesystem order; sorting here is what makes the
        // model byte-identical across runs and machines.
        let mut entries: Vec<std::fs::DirEntry> =
            entries
                .collect::<Result<Vec<_>, _>>()
                .map_err(|source| ProgramError::Io {
                    path: dir.to_path_buf(),
                    source,
                })?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let name = entry.file_name().to_string_lossy().to_string();
            let file_type = entry.file_type().map_err(|source| ProgramError::Io {
                path: entry.path(),
                source,
            })?;
            // A symlink is neither a directory nor a regular file here, so it
            // is skipped rather than followed. That is the no-escape rule.
            if file_type.is_dir() {
                if name == "target" {
                    self.collect_idl(&entry.path(), prefix, &name, inventory)?;
                    continue;
                }
                if name.starts_with('.') || SKIP_DIRS.contains(&name.as_str()) {
                    if name == "lib" {
                        self.walk_lib(&entry.path(), prefix, depth + 1, inventory, total, count)?;
                    }
                    continue;
                }
                let child_prefix = child(prefix, &name);
                self.walk(
                    &entry.path(),
                    &child_prefix,
                    depth + 1,
                    inventory,
                    total,
                    count,
                )?;
            } else if file_type.is_file() {
                let relative = child(prefix, &name);
                let len = entry
                    .metadata()
                    .map_err(|source| ProgramError::Io {
                        path: entry.path(),
                        source,
                    })?
                    .len();
                if is_rust_source(&name) || is_solidity_source(&name) {
                    if len > MAX_SOURCE_FILE_BYTES {
                        inventory.notes.push(format!(
                            "{relative}: {len} bytes is over the {MAX_SOURCE_FILE_BYTES}-byte \
                             per-file limit; it was not read"
                        ));
                        continue;
                    }
                    *count = count.saturating_add(1);
                    if *count > MAX_SOURCE_FILES {
                        return Err(ProgramError::TooManyFiles {
                            limit: MAX_SOURCE_FILES,
                        });
                    }
                    *total = total.saturating_add(len);
                    if *total > MAX_SOURCE_TOTAL_BYTES {
                        return Err(ProgramError::TotalTooLarge {
                            bytes: *total,
                            limit: MAX_SOURCE_TOTAL_BYTES,
                        });
                    }
                    if is_rust_source(&name) {
                        inventory.rust.push(relative);
                    } else if is_dependency_path(&relative) {
                        inventory.solidity_deps.push(relative);
                    } else {
                        inventory.solidity.push(relative);
                    }
                }
            }
        }
        Ok(())
    }

    /// Walks `lib/` for Solidity only. Foundry keeps base contracts there; a
    /// Rust file under `lib/` is never program source.
    fn walk_lib(
        &self,
        dir: &Path,
        prefix: &str,
        depth: usize,
        inventory: &mut Inventory,
        total: &mut u64,
        count: &mut usize,
    ) -> Result<(), ProgramError> {
        if depth > MAX_WALK_DEPTH {
            return Err(ProgramError::TooDeep {
                path: dir.to_path_buf(),
                limit: MAX_WALK_DEPTH,
            });
        }
        let entries = std::fs::read_dir(dir).map_err(|source| ProgramError::Io {
            path: dir.to_path_buf(),
            source,
        })?;
        let mut entries: Vec<std::fs::DirEntry> =
            entries
                .collect::<Result<Vec<_>, _>>()
                .map_err(|source| ProgramError::Io {
                    path: dir.to_path_buf(),
                    source,
                })?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let name = entry.file_name().to_string_lossy().to_string();
            let file_type = entry.file_type().map_err(|source| ProgramError::Io {
                path: entry.path(),
                source,
            })?;
            if file_type.is_dir() {
                if name.starts_with('.') {
                    continue;
                }
                let child_prefix = child(prefix, &name);
                self.walk_lib(
                    &entry.path(),
                    &child_prefix,
                    depth + 1,
                    inventory,
                    total,
                    count,
                )?;
            } else if file_type.is_file() && is_solidity_source(&name) {
                let relative = child(prefix, &name);
                let len = entry
                    .metadata()
                    .map_err(|source| ProgramError::Io {
                        path: entry.path(),
                        source,
                    })?
                    .len();
                if len > MAX_SOURCE_FILE_BYTES {
                    inventory.notes.push(format!(
                        "{relative}: {len} bytes is over the {MAX_SOURCE_FILE_BYTES}-byte \
                         per-file limit; it was not read"
                    ));
                    continue;
                }
                *count = count.saturating_add(1);
                if *count > MAX_SOURCE_FILES {
                    return Err(ProgramError::TooManyFiles {
                        limit: MAX_SOURCE_FILES,
                    });
                }
                *total = total.saturating_add(len);
                if *total > MAX_SOURCE_TOTAL_BYTES {
                    return Err(ProgramError::TotalTooLarge {
                        bytes: *total,
                        limit: MAX_SOURCE_TOTAL_BYTES,
                    });
                }
                inventory.solidity_deps.push(relative);
            }
        }
        Ok(())
    }

    /// Records `<dir>/idl/*.json` for an Anchor `target/` directory.
    fn collect_idl(
        &self,
        target_dir: &Path,
        prefix: &str,
        name: &str,
        inventory: &mut Inventory,
    ) -> Result<(), ProgramError> {
        let idl_dir = target_dir.join("idl");
        let meta = match std::fs::metadata(&idl_dir) {
            Ok(meta) => meta,
            // No IDL directory is normal outside Anchor.
            Err(_) => return Ok(()),
        };
        if !meta.is_dir() {
            return Ok(());
        }
        let entries = std::fs::read_dir(&idl_dir).map_err(|source| ProgramError::Io {
            path: idl_dir.clone(),
            source,
        })?;
        let mut names: Vec<String> = entries
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_ok_and(|t| t.is_file()))
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .filter(|name| name.to_ascii_lowercase().ends_with(".json"))
            .collect();
        names.sort();
        for json in names {
            inventory
                .idl
                .push(child(&child(prefix, name), &format!("idl/{json}")));
        }
        Ok(())
    }
}

fn child(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{prefix}/{name}")
    }
}

fn is_rust_source(name: &str) -> bool {
    name.to_ascii_lowercase().ends_with(".rs")
}

fn is_solidity_source(name: &str) -> bool {
    name.to_ascii_lowercase().ends_with(".sol")
}

/// Whether a relative path sits under a dependency or build directory. Such a
/// Solidity file is parsed for base contracts but is not an entry point.
fn is_dependency_path(relative: &str) -> bool {
    relative
        .split('/')
        .any(|part| part.starts_with('.') || part == "lib" || SKIP_DIRS.contains(&part))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outside_target_notes(error: ProgramError) -> bool {
        matches!(error, ProgramError::OutsideTarget { .. })
    }

    #[test]
    fn a_parent_component_is_refused_before_any_read() {
        let tmp = tempfile::tempdir().unwrap();
        let root = TargetRoot::new(tmp.path()).unwrap();
        assert!(outside_target_notes(
            root.resolve("../escape.rs").unwrap_err()
        ));
        assert!(outside_target_notes(
            root.resolve("a/../../b.rs").unwrap_err()
        ));
        assert!(outside_target_notes(root.resolve("./a.rs").unwrap_err()));
        assert!(outside_target_notes(root.resolve("").unwrap_err()));
    }

    #[test]
    fn an_absolute_path_outside_the_root_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let outside = tmp.path().join("outside");
        std::fs::create_dir(&outside).unwrap();
        std::fs::write(outside.join("secret.rs"), "fn main() {}").unwrap();
        let inside = tmp.path().join("inside");
        std::fs::create_dir(&inside).unwrap();
        let root = TargetRoot::new(&inside).unwrap();
        let absolute = outside.join("secret.rs");
        assert!(outside_target_notes(
            root.resolve(&absolute.to_string_lossy()).unwrap_err()
        ));
        // A path that names an in-root file by an absolute path is refused
        // too: the model's inputs are relative.
        let local = inside.join("ok.rs");
        std::fs::write(&local, "fn main() {}").unwrap();
        assert!(outside_target_notes(
            root.resolve(&local.to_string_lossy()).unwrap_err()
        ));
    }

    #[test]
    fn a_symlink_out_of_the_root_is_refused_or_skipped() {
        let tmp = tempfile::tempdir().unwrap();
        let inside = tmp.path().join("inside");
        std::fs::create_dir(&inside).unwrap();
        let outside = tmp.path().join("outside");
        std::fs::create_dir(&outside).unwrap();
        std::fs::write(outside.join("secret.rs"), "pub fn hidden() {}").unwrap();
        let link = inside.join("link.rs");
        let created = make_file_symlink(&outside.join("secret.rs"), &link);
        let root = TargetRoot::new(&inside).unwrap();
        if created {
            assert!(outside_target_notes(root.resolve("link.rs").unwrap_err()));
            // The walk never follows the link either.
            let inventory = root.inventory().unwrap();
            assert!(!inventory.rust.iter().any(|p| p == "link.rs"));
        } else {
            eprintln!("symlinks cannot be created here; the symlink case is skipped");
        }
    }

    #[test]
    fn a_regular_file_under_the_root_is_read() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir(tmp.path().join("src")).unwrap();
        std::fs::write(tmp.path().join("src/lib.rs"), "pub fn x() {}\n").unwrap();
        let root = TargetRoot::new(tmp.path()).unwrap();
        assert_eq!(root.read("src/lib.rs").unwrap(), "pub fn x() {}\n");
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
}
