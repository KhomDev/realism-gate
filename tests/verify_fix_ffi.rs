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
    std::fs::create_dir_all(dir.join("home")).unwrap();
    std::fs::write(dir.join("project/test/Poc.t.sol"), "contract Poc {}\n").unwrap();
    std::fs::write(dir.join("project/foundry.toml"), BENIGN).unwrap();
    dir
}

/// A forge stand-in that records that it ran, records the `FOUNDRY_CONFIG`
/// value it inherited and that file's content, and prints one passing test.
fn fake_forge(dir: &Path) -> (PathBuf, PathBuf) {
    let marker = dir.join("forge-ran.txt");
    let config_value = dir.join("foundry-config-value.txt");
    let config_content = dir.join("foundry-config-content.txt");
    #[cfg(unix)]
    let path = {
        use std::os::unix::fs::PermissionsExt;
        let p = dir.join("forge-stand-in");
        std::fs::write(
            &p,
            format!(
                "#!/bin/sh\nprintf 'ran\\n' > '{}'\nprintf '%s' \"${{FOUNDRY_CONFIG:-}}\" > '{}'\ncat \"${{FOUNDRY_CONFIG:-/dev/null}}\" > '{}' 2>/dev/null\nprintf '%s\\n' '{{\"t\":{{\"test_a\":{{\"success\":true}}}}}}'\nexit 0\n",
                marker.display(),
                config_value.display(),
                config_content.display()
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
                "@echo off\r\n> \"{}\" echo ran\r\n> \"{}\" echo %FOUNDRY_CONFIG%\r\ntype \"%FOUNDRY_CONFIG%\" > \"{}\" 2>nul\r\necho {{\"t\":{{\"test_a\":{{\"success\":true}}}}}}\r\nexit /b 0\r\n",
                marker.display(),
                config_value.display(),
                config_content.display()
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
    verify_fix_with_env(project, forge, patched, patch, &[])
}

fn verify_fix_with_env(
    project: &Path,
    forge: &Path,
    patched: Option<&Path>,
    patch: Option<&Path>,
    extra_env: &[(&str, &str)],
) -> Output {
    let home = project
        .parent()
        .expect("the scratch project has a parent")
        .join("home");
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_realism-gate"));
    cmd.arg("verify-fix")
        .arg("--sandbox")
        .arg("--root")
        .arg(project)
        .arg("--poc")
        .arg("test/Poc.t.sol")
        .arg("--forge")
        .arg(forge)
        // A developer's real global Foundry config must not change a test.
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .env_remove("REALISM_GATE_SANDBOX");
    for (key, value) in extra_env {
        cmd.env(key, value);
    }
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

#[test]
fn a_global_config_that_enables_ffi_is_refused_before_forge_starts() {
    // The fake HOME is set by verify_fix(); Foundry reads the same global
    // file, so a tree with ffi off is still unsafe while this one says true.
    let dir = scratch("global");
    let project = dir.join("project");
    std::fs::create_dir_all(dir.join("home/.foundry")).unwrap();
    std::fs::write(
        dir.join("home/.foundry/foundry.toml"),
        "[profile.default]\nffi = true\n",
    )
    .unwrap();
    let (forge, marker) = fake_forge(&dir);
    let out = verify_fix(&project, &forge, Some(&project.join("patched")), None);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(2), "{stderr}");
    assert!(
        stderr.contains("global config") && stderr.to_ascii_lowercase().contains("ffi"),
        "{stderr}"
    );
    assert!(!marker.exists(), "forge ran despite an ffi global config");
    let _ = std::fs::remove_dir_all(&dir);
}

fn create_patch(path: &str, lines: &[&str]) -> String {
    let mut patch = format!(
        "--- /dev/null\n+++ b/{path}\n@@ -0,0 +1,{} @@\n",
        lines.len()
    );
    for line in lines {
        patch.push('+');
        patch.push_str(line);
        patch.push('\n');
    }
    patch
}

fn scripted_patch(dir: &Path, name: &str, sections: &[(&str, &[&str])]) -> PathBuf {
    let mut text = String::new();
    for (path, lines) in sections {
        text.push_str(&create_patch(path, lines));
    }
    let patch = dir.join(name);
    std::fs::write(&patch, text).unwrap();
    patch
}

#[test]
fn a_patch_that_adds_dotenv_and_an_evil_config_is_refused() {
    // Foundry loads .env after it starts; a .env that sets FOUNDRY_CONFIG can
    // point it at an ffi config the guard never parsed. copy_tree skips
    // dotfiles, so this vector is a --patch that creates one.
    let dir = scratch("dotenv");
    let project = dir.join("project");
    let patch = scripted_patch(
        &dir,
        "evil.diff",
        &[
            (".env", &["FOUNDRY_CONFIG=evil.toml"]),
            ("evil.toml", &["[profile.default]", "ffi = true"]),
        ],
    );
    let (forge, marker) = fake_forge(&dir);
    let out = verify_fix(&project, &forge, None, Some(&patch));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(2), "{stderr}");
    assert!(stderr.contains(".env"), "{stderr}");
    assert!(!marker.exists(), "forge ran despite the dotenv bypass");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_clean_dotenv_is_still_refused() {
    // The policy is the file, not a pattern inside it: any .env in a copied
    // tree is refused, so a future Foundry dotenv variable cannot reopen the
    // vector quietly.
    let dir = scratch("dotenv-clean");
    let project = dir.join("project");
    let patch = scripted_patch(&dir, "clean.diff", &[(".env", &["SOME_SETTING=1"])]);
    let (forge, marker) = fake_forge(&dir);
    let out = verify_fix(&project, &forge, None, Some(&patch));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(2), "{stderr}");
    assert!(stderr.contains(".env"), "{stderr}");
    assert!(!marker.exists(), "forge ran with a .env in the copy");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_forge_child_gets_the_pinned_absolute_config() {
    // A parent process (and, via dotenv, a project) can set FOUNDRY_CONFIG.
    // The child must receive the absolute path of a config in the copy that
    // verify-fix checked, not the inherited one. The project has no
    // foundry.toml, so the pinned file is the minimal one verify-fix creates.
    let dir = scratch("pinned");
    let project = dir.join("project");
    std::fs::remove_file(project.join("foundry.toml")).unwrap();
    let dangerous = dir.join("dangerous.toml");
    std::fs::write(&dangerous, "[profile.default]\nffi = true\n").unwrap();
    let (forge, marker) = fake_forge(&dir);
    let dangerous_text = dangerous.to_string_lossy().to_string();
    let out = verify_fix_with_env(
        &project,
        &forge,
        Some(&project.join("patched")),
        None,
        &[("FOUNDRY_CONFIG", dangerous_text.as_str())],
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(marker.exists(), "forge did not run: {stderr}");
    assert_eq!(out.status.code(), Some(1), "{stderr}");
    let value = std::fs::read_to_string(dir.join("foundry-config-value.txt")).unwrap();
    let value = value.trim();
    assert!(Path::new(value).is_absolute(), "{value:?} is not absolute");
    assert!(value.ends_with("foundry.toml"), "{value:?}");
    assert!(
        value.contains("realism-gate-verify"),
        "the pinned config is not in the temp copy: {value:?}"
    );
    assert_ne!(
        value, dangerous_text,
        "the inherited FOUNDRY_CONFIG was not overridden"
    );
    let content = std::fs::read_to_string(dir.join("foundry-config-content.txt")).unwrap();
    assert!(
        content.contains("[profile.default]"),
        "the pinned config was not the checked one: {content:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_oversized_foundry_toml_is_refused_before_forge_starts() {
    // A 2 MiB sparse config is refused from metadata, before it is read and
    // before the TOML parse can allocate it.
    let dir = scratch("huge-config");
    let project = dir.join("project");
    let foundry = project.join("foundry.toml");
    let file = std::fs::File::create(&foundry).unwrap();
    file.set_len(2 * 1024 * 1024).unwrap();
    drop(file);
    let (forge, marker) = fake_forge(&dir);
    let out = verify_fix(&project, &forge, Some(&project.join("patched")), None);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(2), "{stderr}");
    assert!(
        stderr.contains("1048576") && stderr.contains("not read"),
        "{stderr}"
    );
    assert!(!marker.exists(), "forge ran despite an unreadable config");
    let _ = std::fs::remove_dir_all(&dir);
}
