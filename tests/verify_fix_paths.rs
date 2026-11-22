//! `verify-fix` path arguments are resolved from the caller's current
//! directory, not from the temp copy's. A relative `--patch` outside the
//! project and a relative `--forge` with a separator both used to be
//! reinterpreted after the copy became the working directory.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const PATCH: &str = "--- a/src/A.sol\n+++ b/src/A.sol\n@@ -1 +1 @@\n-contract A { uint x = 1; }\n+contract A { uint x = 2; }\n";

fn scratch(name: &str) -> PathBuf {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir =
        std::env::temp_dir().join(format!("rg-vf-paths-{name}-{}-{nanos}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let project = dir.join("project");
    std::fs::create_dir_all(project.join("test")).unwrap();
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::create_dir_all(project.join("patched")).unwrap();
    std::fs::create_dir_all(dir.join("home")).unwrap();
    std::fs::write(project.join("test/Poc.t.sol"), "contract Poc {}\n").unwrap();
    std::fs::write(project.join("src/A.sol"), "contract A { uint x = 1; }\n").unwrap();
    dir
}

/// A forge stand-in that records that it ran. It passes until the patch has
/// changed `x = 1` to `x = 2`, so the before/after rule is observable.
fn fake_forge(dir: &Path) -> PathBuf {
    let marker = dir.join("forge-ran.txt");
    #[cfg(unix)]
    let path = {
        use std::os::unix::fs::PermissionsExt;
        let p = dir.join("forge-stand-in");
        std::fs::write(
            &p,
            format!(
                "#!/bin/sh\nprintf 'ran\\n' > '{}'\nif grep -q 'x = 2' \"$PWD/src/A.sol\" 2>/dev/null; then\n  printf '%s\\n' '{{\"t\":{{\"test_a\":{{\"success\":false}}}}}}'\n  exit 1\nfi\nprintf '%s\\n' '{{\"t\":{{\"test_a\":{{\"success\":true}}}}}}'\nexit 0\n",
                marker.display()
            ),
        )
        .unwrap();
        let mut perms = std::fs::metadata(&p).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&p, perms).unwrap();
        p
    };
    #[cfg(windows)]
    let path = {
        let p = dir.join("forge-stand-in.bat");
        std::fs::write(
            &p,
            format!(
                "@echo off\r\n> \"{}\" echo ran\r\nfindstr /C:\"x = 2\" \"src\\A.sol\" >nul 2>&1\r\nif %errorlevel%==0 (\r\n  echo {{\"t\":{{\"test_a\":{{\"success\":false}}}}}}\r\n  exit /b 1\r\n)\r\necho {{\"t\":{{\"test_a\":{{\"success\":true}}}}}}\r\nexit /b 0\r\n",
                marker.display()
            ),
        )
        .unwrap();
        p
    };
    path
}

/// Runs the binary with the project as the current directory.
fn run(project: &Path, args: &[&str]) -> Output {
    let home = project
        .parent()
        .expect("the scratch project has a parent")
        .join("home");
    Command::new(env!("CARGO_BIN_EXE_realism-gate"))
        .args(args)
        .current_dir(project)
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .env_remove("REALISM_GATE_SANDBOX")
        .output()
        .expect("binary runs")
}

#[test]
fn a_relative_patch_outside_the_root_is_resolved_from_the_caller_cwd() {
    let dir = scratch("patch");
    let project = dir.join("project");
    let patch = dir.join("fix.diff");
    std::fs::write(&patch, PATCH).unwrap();
    let forge = fake_forge(&dir);
    let forge_arg = forge.to_string_lossy().to_string();
    let out = run(
        &project,
        &[
            "verify-fix",
            "--sandbox",
            "--root",
            ".",
            "--poc",
            "test/Poc.t.sol",
            "--patch",
            "../fix.diff",
            "--forge",
            forge_arg.as_str(),
        ],
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    // The PoC passes before the patch and fails after it: exit 0.
    assert_eq!(out.status.code(), Some(0), "{stderr}");
    assert!(
        dir.join("forge-ran.txt").exists(),
        "forge never ran: {stderr}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_relative_forge_with_a_separator_is_resolved_from_the_caller_cwd() {
    let dir = scratch("forge");
    let project = dir.join("project");
    let forge = fake_forge(&dir);
    let name = forge.file_name().unwrap().to_string_lossy().to_string();
    let forge_arg = format!("../{name}");
    let out = run(
        &project,
        &[
            "verify-fix",
            "--sandbox",
            "--root",
            ".",
            "--poc",
            "test/Poc.t.sol",
            "--patched",
            "patched",
            "--forge",
            forge_arg.as_str(),
        ],
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    // Both sides pass, so the fix does not hold; the point is that the
    // relative forge path was found after the temp copy became the cwd.
    assert_eq!(out.status.code(), Some(1), "{stderr}");
    assert!(
        dir.join("forge-ran.txt").exists(),
        "the relative forge was not found: {stderr}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_patch_that_git_apply_check_refuses_never_starts_forge_or_touches_the_project() {
    let dir = scratch("bad-patch");
    let project = dir.join("project");
    let bad = dir.join("bad.diff");
    // The old-side context does not match `src/A.sol`, so `git apply --check`
    // must refuse before any file is written, and forge must never start.
    std::fs::write(
        &bad,
        "--- a/src/A.sol\n+++ b/src/A.sol\n@@ -1 +1 @@\n-contract A { uint x = 99; }\n+contract A { uint x = 2; }\n",
    )
    .unwrap();
    let forge = fake_forge(&dir);
    let forge_arg = forge.to_string_lossy().to_string();
    let bad_arg = bad.to_string_lossy().to_string();
    let out = run(
        &project,
        &[
            "verify-fix",
            "--sandbox",
            "--root",
            ".",
            "--poc",
            "test/Poc.t.sol",
            "--patch",
            bad_arg.as_str(),
            "--forge",
            forge_arg.as_str(),
        ],
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(2), "{stderr}");
    assert!(stderr.contains("git apply"), "{stderr}");
    assert!(
        !dir.join("forge-ran.txt").exists(),
        "forge ran after a patch that did not apply"
    );
    let original = std::fs::read_to_string(project.join("src/A.sol")).unwrap();
    assert!(
        original.contains("x = 1"),
        "the project was modified: {original}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
