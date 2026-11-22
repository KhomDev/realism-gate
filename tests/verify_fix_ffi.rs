//! The `ffi` guard end to end: every tree forge could run in is parsed as
//! TOML, and a refusal happens before forge is started.
//!
//! The forge here is a stand-in that records that it ran. A refused case must
//! leave the marker absent; the control case must create it, so a guard that
//! refused everything would not pass this file.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Spellings TOML accepts for `ffi = true`. The 1.0.3 heuristic (strip
/// comments, compare normalized words) missed the tabbed, quoted and dotted
/// forms; a parse cannot.
const SPELLINGS: &[&str] = &[
    "[profile.default]\nffi = true\n",
    "[profile.default]\nffi\t=\ttrue\n",
    "[profile.default]\n\"ffi\" = true\n",
    "[profile.default]\n'ffi' = true\n",
    "[profile.default]\nffi=true\n",
    "profile.default.ffi = true\n",
    "ffi = true\n",
    "[profile.ci]\nffi = true\n",
];

const BENIGN: &str = "[profile.default]\nffi = false\n";

fn scratch(name: &str) -> PathBuf {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("rg-vf-ffi-{name}-{}-{nanos}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("project/test")).unwrap();
    std::fs::create_dir_all(dir.join("project/patched")).unwrap();
    std::fs::write(dir.join("project/test/Poc.t.sol"), "contract Poc {}\n").unwrap();
    std::fs::write(dir.join("project/foundry.toml"), BENIGN).unwrap();
    dir
}

/// A forge stand-in that records that it ran and prints one passing test.
fn fake_forge(dir: &Path) -> (PathBuf, PathBuf) {
    let marker = dir.join("forge-ran.txt");
    #[cfg(unix)]
    let path = {
        use std::os::unix::fs::PermissionsExt;
        let p = dir.join("forge-stand-in");
        std::fs::write(
            &p,
            format!(
                "#!/bin/sh\nprintf 'ran\\n' > '{}'\nprintf '%s\\n' '{{\"t\":{{\"test_a\":{{\"success\":true}}}}}}'\nexit 0\n",
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
                "@echo off\r\n> \"{}\" echo ran\r\necho {{\"t\":{{\"test_a\":{{\"success\":true}}}}}}\r\nexit /b 0\r\n",
                marker.display()
            ),
        )
        .unwrap();
        p
    };
    (path, marker)
}

fn verify_fix(
    project: &Path,
    forge: &Path,
    patched: Option<&Path>,
    patch: Option<&Path>,
) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_realism-gate"));
    cmd.arg("verify-fix")
        .arg("--sandbox")
        .arg("--root")
        .arg(project)
        .arg("--poc")
        .arg("test/Poc.t.sol")
        .arg("--forge")
        .arg(forge)
        .env_remove("REALISM_GATE_SANDBOX");
    if let Some(p) = patched {
        cmd.arg("--patched").arg(p);
    }
    if let Some(p) = patch {
        cmd.arg("--patch").arg(p);
    }
    cmd.output().expect("binary runs")
}

/// A patch that replaces the two-line benign `foundry.toml` with `spelling`.
fn replacement_patch(spelling: &str) -> String {
    let old = BENIGN;
    let mut patch = String::from("--- a/foundry.toml\n+++ b/foundry.toml\n");
    patch.push_str(&format!(
        "@@ -1,{} +1,{} @@\n",
        old.lines().count(),
        spelling.lines().count()
    ));
    for line in old.lines() {
        patch.push('-');
        patch.push_str(line);
        patch.push('\n');
    }
    for line in spelling.lines() {
        patch.push('+');
        patch.push_str(line);
        patch.push('\n');
    }
    patch
}

fn refused(case: &str, spelling: &str, out: &Output, marker: &Path) {
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(2), "{case} {spelling:?}: {stderr}");
    assert!(
        stderr.to_ascii_lowercase().contains("ffi"),
        "{case} {spelling:?}: the refusal does not name ffi: {stderr}"
    );
    assert!(
        !marker.exists(),
        "{case} {spelling:?}: forge was started despite ffi = true"
    );
}

#[test]
fn ffi_is_refused_in_the_root_the_patched_tree_and_after_a_patch() {
    for spelling in SPELLINGS {
        // 1. The root tree.
        let dir = scratch("root");
        let project = dir.join("project");
        std::fs::write(project.join("foundry.toml"), spelling).unwrap();
        let (forge, marker) = fake_forge(&dir);
        let out = verify_fix(&project, &forge, Some(&project.join("patched")), None);
        refused("root", spelling, &out, &marker);
        let _ = std::fs::remove_dir_all(&dir);

        // 2. The --patched tree.
        let dir = scratch("patched");
        let project = dir.join("project");
        std::fs::write(project.join("patched/foundry.toml"), spelling).unwrap();
        let (forge, marker) = fake_forge(&dir);
        let out = verify_fix(&project, &forge, Some(&project.join("patched")), None);
        refused("--patched", spelling, &out, &marker);
        let _ = std::fs::remove_dir_all(&dir);

        // 3. The copied tree after --patch is applied.
        let dir = scratch("patch");
        let project = dir.join("project");
        let patch = dir.join("fix.diff");
        std::fs::write(&patch, replacement_patch(spelling)).unwrap();
        let (forge, marker) = fake_forge(&dir);
        let out = verify_fix(&project, &forge, None, Some(&patch));
        refused("--patch", spelling, &out, &marker);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn the_guard_does_not_refuse_a_project_with_ffi_off() {
    // A control: a benign tree reaches forge, so the refusals above are about
    // ffi and not about the command always failing.
    let dir = scratch("control");
    let project = dir.join("project");
    let (forge, marker) = fake_forge(&dir);
    let out = verify_fix(&project, &forge, Some(&project.join("patched")), None);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        marker.exists(),
        "a benign project never reached forge: {stderr}"
    );
    // Both sides passed, which is "the fix does not hold", not a usage error.
    assert_eq!(out.status.code(), Some(1), "{stderr}");
    let _ = std::fs::remove_dir_all(&dir);
}
