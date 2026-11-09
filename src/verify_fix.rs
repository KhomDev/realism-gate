//! `realism-gate verify-fix`: run a PoC against a patched tree and require
//! the PoC to fail.
//!
//! This is the only command that executes a test. `check` does not call it.
//! Forge runs locally; this module opens no socket. A fork test that forge
//! itself runs may, which is forge's network use, not this program's.

use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct Request {
    /// The Foundry test file, relative to `root` (or absolute).
    pub poc: PathBuf,
    /// A unified diff to apply to a copy of `root`.
    pub patch: Option<PathBuf>,
    /// A directory that already contains the patched sources.
    pub patched_dir: Option<PathBuf>,
    pub match_test: Option<String>,
    /// The forge binary. Tests pass a stand-in.
    pub forge: PathBuf,
    /// The project forge should run in when `--patch` is used. Defaults to
    /// the current directory at the call site.
    pub root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The PoC failed on the patched tree. The fix holds.
    FixHolds { detail: String },
    /// The PoC still passed. The fix does not kill it.
    FixDoesNotHold { detail: String },
}

#[derive(Debug)]
pub struct VerifyError(pub String);

impl std::fmt::Display for VerifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Runs forge. `Ok` is a pass-or-fail of the fix; `Err` is forge missing, a
/// patch that did not apply, or a compile error (the PoC never ran).
pub fn verify(req: &Request) -> Result<Outcome, VerifyError> {
    if req.patch.is_some() == req.patched_dir.is_some() {
        return Err(VerifyError(
            "pass exactly one of --patch FILE and --patched DIR".into(),
        ));
    }
    let work = if let Some(dir) = &req.patched_dir {
        if !dir.is_dir() {
            return Err(VerifyError(format!("{}: not a directory", dir.display())));
        }
        dir.clone()
    } else {
        let patch = req.patch.as_ref().expect("patch or dir");
        if !patch.is_file() {
            return Err(VerifyError(format!("{}: no such patch", patch.display())));
        }
        let tmp = std::env::temp_dir().join(format!(
            "realism-gate-verify-{}-{}",
            std::process::id(),
            unique()
        ));
        copy_tree(&req.root, &tmp)?;
        apply_patch(&tmp, patch)?;
        tmp
    };
    let poc = poc_arg(&req.poc, &req.root, &work);
    let mut cmd = Command::new(&req.forge);
    cmd.current_dir(&work)
        .arg("test")
        .arg("--match-path")
        .arg(&poc)
        .arg("--json");
    // Offline by default for the forge invocation we control: a PoC that
    // forks still needs the RPC the test names, and forge will say so. We do
    // not add a URL.
    if let Some(name) = &req.match_test {
        cmd.arg("--match-test").arg(name);
    }
    let output = cmd.output().map_err(|e| {
        VerifyError(format!(
            "could not run {}: {e}. verify-fix needs forge on PATH, or --forge.",
            req.forge.display()
        ))
    })?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}\n{stderr}");
    // A compile error exits non-zero and does not report a test result.
    let saw_test = combined.contains("\"success\"")
        || combined.contains("test result:")
        || combined.contains("[PASS]")
        || combined.contains("[FAIL]");
    if !output.status.success() && !saw_test {
        return Err(VerifyError(format!(
            "forge did not run the PoC (compile or usage error):\n{}",
            first_lines(&combined, 20)
        )));
    }
    let passed = if combined.contains("\"success\"") {
        // Any failing test means the PoC no longer holds. A JSON report with
        // only `"success": true` means it still passes.
        let failed =
            combined.contains("\"success\": false") || combined.contains("\"success\":false");
        let ok = combined.contains("\"success\": true") || combined.contains("\"success\":true");
        if failed {
            false
        } else if ok && output.status.success() {
            true
        } else if ok && !output.status.success() {
            // Forge sometimes exits non-zero for a skipped or a trace. Trust
            // the success flags when they are present and unanimous.
            false
        } else {
            !output.status.success()
        }
    } else {
        // Text forge: exit 0 is a pass of the tests.
        output.status.success()
    };
    let detail = first_lines(&combined, 12);
    if passed {
        Ok(Outcome::FixDoesNotHold { detail })
    } else {
        Ok(Outcome::FixHolds { detail })
    }
}

fn poc_arg(poc: &Path, root: &Path, work: &Path) -> String {
    if poc.is_absolute() {
        poc.strip_prefix(root)
            .or_else(|_| poc.strip_prefix(work))
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| poc.to_string_lossy().replace('\\', "/"))
    } else {
        poc.to_string_lossy().replace('\\', "/")
    }
}

fn apply_patch(dir: &Path, patch: &Path) -> Result<(), VerifyError> {
    let git = Command::new("git")
        .args(["apply", "--unsafe-paths"])
        .arg(patch)
        .current_dir(dir)
        .output();
    if let Ok(out) = git
        && out.status.success()
    {
        return Ok(());
    }
    let patch_cmd = Command::new("patch")
        .args(["-p1", "--forward", "--input"])
        .arg(patch)
        .current_dir(dir)
        .output()
        .map_err(|e| VerifyError(format!("could not run patch or git apply: {e}")))?;
    if patch_cmd.status.success() {
        Ok(())
    } else {
        Err(VerifyError(format!(
            "the patch did not apply:\n{}",
            first_lines(&String::from_utf8_lossy(&patch_cmd.stderr), 15)
        )))
    }
}

fn copy_tree(from: &Path, to: &Path) -> Result<(), VerifyError> {
    std::fs::create_dir_all(to).map_err(|e| VerifyError(format!("{}: {e}", to.display())))?;
    let skip = [
        "out",
        "cache",
        "broadcast",
        "target",
        "node_modules",
        ".git",
        "artifacts",
    ];
    fn walk(from: &Path, to: &Path, skip: &[&str]) -> Result<(), VerifyError> {
        for entry in
            std::fs::read_dir(from).map_err(|e| VerifyError(format!("{}: {e}", from.display())))?
        {
            let entry = entry.map_err(|e| VerifyError(e.to_string()))?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if skip.contains(&name.as_ref()) || name.starts_with('.') && name != ".gitignore" {
                continue;
            }
            let dest = to.join(entry.file_name());
            let ty = entry.file_type().map_err(|e| VerifyError(e.to_string()))?;
            if ty.is_dir() {
                std::fs::create_dir_all(&dest)
                    .map_err(|e| VerifyError(format!("{}: {e}", dest.display())))?;
                walk(&entry.path(), &dest, skip)?;
            } else if ty.is_file() {
                std::fs::copy(entry.path(), &dest)
                    .map_err(|e| VerifyError(format!("{}: {e}", dest.display())))?;
            }
        }
        Ok(())
    }
    walk(from, to, &skip)
}

fn first_lines(text: &str, n: usize) -> String {
    text.lines().take(n).collect::<Vec<_>>().join("\n")
}

fn unique() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, body).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&path).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&path, perms).unwrap();
        }
        path
    }

    #[test]
    fn a_failing_poc_means_the_fix_holds() {
        let dir = std::env::temp_dir().join(format!("rg-vf-{}", unique()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let forge = script(
            &dir,
            "forge",
            "#!/bin/sh\nprintf '%s\\n' '{\"suite\":{\"test_a\":{\"success\":false}}}'\nexit 1\n",
        );
        let poc_dir = dir.join("patched");
        std::fs::create_dir_all(&poc_dir).unwrap();
        let out = verify(&Request {
            poc: PathBuf::from("test/Poc.t.sol"),
            patch: None,
            patched_dir: Some(poc_dir),
            match_test: None,
            forge,
            root: dir.clone(),
        })
        .unwrap();
        assert!(matches!(out, Outcome::FixHolds { .. }), "{out:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_passing_poc_means_the_fix_does_not_hold() {
        let dir = std::env::temp_dir().join(format!("rg-vp-{}", unique()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let forge = script(
            &dir,
            "forge",
            "#!/bin/sh\nprintf '%s\\n' '{\"suite\":{\"test_a\":{\"success\": true}}}'\nexit 0\n",
        );
        let poc_dir = dir.join("patched");
        std::fs::create_dir_all(&poc_dir).unwrap();
        let out = verify(&Request {
            poc: PathBuf::from("test/Poc.t.sol"),
            patch: None,
            patched_dir: Some(poc_dir),
            match_test: None,
            forge,
            root: dir.clone(),
        })
        .unwrap();
        assert!(matches!(out, Outcome::FixDoesNotHold { .. }), "{out:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_compile_error_is_not_a_failed_poc() {
        let dir = std::env::temp_dir().join(format!("rg-vc-{}", unique()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let forge = script(
            &dir,
            "forge",
            "#!/bin/sh\necho 'Compiler run failed' >&2\nexit 1\n",
        );
        let poc_dir = dir.join("patched");
        std::fs::create_dir_all(&poc_dir).unwrap();
        let err = verify(&Request {
            poc: PathBuf::from("test/Poc.t.sol"),
            patch: None,
            patched_dir: Some(poc_dir),
            match_test: None,
            forge,
            root: dir.clone(),
        })
        .unwrap_err();
        assert!(err.0.contains("compile"), "{}", err.0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_patch_is_applied_before_forge_runs() {
        let dir = std::env::temp_dir().join(format!("rg-vpatch-{}", unique()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("src/A.sol"), "contract A { uint x = 1; }\n").unwrap();
        let patch = dir.join("fix.diff");
        std::fs::write(
            &patch,
            "--- a/src/A.sol\n+++ b/src/A.sol\n@@ -1 +1 @@\n-contract A { uint x = 1; }\n+contract A { uint x = 2; }\n",
        )
        .unwrap();
        let forge = script(
            &dir,
            "forge",
            "#!/bin/sh\nif grep -q 'x = 2' \"$PWD/src/A.sol\"; then\n  printf '%s\\n' '{\"t\":{\"test_a\":{\"success\":false}}}'\n  exit 1\nfi\nprintf '%s\\n' '{\"t\":{\"test_a\":{\"success\":true}}}'\nexit 0\n",
        );
        let out = verify(&Request {
            poc: PathBuf::from("test/Poc.t.sol"),
            patch: Some(patch),
            patched_dir: None,
            match_test: Some("test_a".into()),
            forge,
            root: dir.clone(),
        })
        .unwrap();
        assert!(matches!(out, Outcome::FixHolds { .. }), "{out:?}");
        // The copy was patched; the tree the user has was not.
        let original = std::fs::read_to_string(dir.join("src/A.sol")).unwrap();
        assert!(original.contains("x = 1"), "{original}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
