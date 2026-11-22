//! The `ffi` and `fs_permissions` guard end to end: every tree forge could run
//! in is parsed as TOML, keys Foundry normalizes to either setting are checked
//! under that normalization, and a refusal happens before forge is started.
//!
//! The forge here is a stand-in that records that it ran. A refused case must
//! leave the marker absent; the control case must create it, so a guard that
//! refused everything would not pass this file.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Spellings TOML accepts for `ffi = true`, and the key aliases Foundry
/// resolves to `ffi` after normalizing keys (case and non-alphanumerics are
/// ignored). The 1.0.3 heuristic missed the tabbed, quoted and dotted forms;
/// the exact-key scan this replaces missed every alias.
const SPELLINGS: &[&str] = &[
    "[profile.default]\nffi = true\n",
    "[profile.default]\nffi\t=\ttrue\n",
    "[profile.default]\n\"ffi\" = true\n",
    "[profile.default]\n'ffi' = true\n",
    "[profile.default]\nffi=true\n",
    "profile.default.ffi = true\n",
    "ffi = true\n",
    "[profile.ci]\nffi = true\n",
    "[profile.default]\nFFI = true\n",
    "[profile.default]\nFfi = true\n",
    "[profile.default]\nffi_ = true\n",
    "[profile.default]\nffi__ = true\n",
    "[profile.default]\nf_f_i = true\n",
    "[profile.default]\nf-f-i = true\n",
    "[profile.default]\n\"f.f.i\" = true\n",
    "[profile.default]\n\"f f i\" = true\n",
    "FFI = true\n",
];

/// Key spellings Foundry normalizes to `fs_permissions`. Verified against
/// forge 1.5.1: every one of these populated `fs_permissions` in
/// `forge config`.
const FS_ALIASES: &[&str] = &[
    "FS_PERMISSIONS",
    "FS-PERMISSIONS",
    "fsPermissions",
    "fs_Permissions",
    "Fs-Permissions",
    "fs__permissions",
    "\"fs.permissions\"",
    "\"fs permissions\"",
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

fn fs_alias_config(key: &str, access: &str) -> String {
    format!("[profile.default]\n{key} = [{{ access = \"{access}\", path = \".\" }}]\n")
}

fn global_config(dir: &Path, text: &str) {
    let home = dir.join("home/.foundry");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(home.join("foundry.toml"), text).unwrap();
}

fn refused_fs(case: &str, key: &str, out: &Output, marker: &Path) {
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(2), "{case} {key:?}: {stderr}");
    assert!(
        stderr.contains("fs_permissions"),
        "{case} {key:?}: the refusal does not name fs_permissions: {stderr}"
    );
    assert!(
        stderr.contains(key.trim_matches('"')),
        "{case} {key:?}: the refusal does not name the alias key: {stderr}"
    );
    assert!(
        !marker.exists(),
        "{case} {key:?}: forge ran despite the fs_permissions alias"
    );
}

/// A forge stand-in that appends to a counter and, on that run, writes an
/// ffi alias to the fake global Foundry config.
fn rewriting_forge(dir: &Path, home: &Path) -> (PathBuf, PathBuf) {
    let counter = dir.join("forge-runs.txt");
    let foundry_dir = home.join(".foundry");
    let global = foundry_dir.join("foundry.toml");
    #[cfg(unix)]
    let path = {
        use std::os::unix::fs::PermissionsExt;
        let p = dir.join("forge-rewrite");
        std::fs::write(
            &p,
            format!(
                "#!/bin/sh\nprintf 'ran\\n' >> '{}'\nmkdir -p '{}'\nprintf '[profile.default]\\nFFI = true\\n' > '{}'\nprintf '%s\\n' '{{\"t\":{{\"test_a\":{{\"success\":true}}}}}}'\nexit 0\n",
                counter.display(),
                foundry_dir.display(),
                global.display()
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
        let p = dir.join("forge-rewrite.bat");
        std::fs::write(
            &p,
            format!(
                "@echo off\r\n>> \"{}\" echo ran\r\nif not exist \"{}\" mkdir \"{}\"\r\n> \"{}\" echo [profile.default]\r\n>> \"{}\" echo FFI = true\r\necho {{\"t\":{{\"test_a\":{{\"success\":true}}}}}}\r\nexit /b 0\r\n",
                counter.display(),
                foundry_dir.display(),
                foundry_dir.display(),
                global.display(),
                global.display()
            ),
        )
        .unwrap();
        p
    };
    (path, counter)
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
    // verify-fix reads the global config under USERPROFILE (then HOME) on
    // Windows and HOME elsewhere; the fake home makes it read this file. On
    // Windows, Foundry may resolve the OS known profile folder instead of
    // these variables; docs/threat-model.md H5 records that residual.
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

fn fs_config(access: &str) -> String {
    format!("[profile.default]\nfs_permissions = [{{ access = \"{access}\", path = \".\" }}]\n")
}

#[test]
fn write_fs_permissions_are_refused_in_every_config_the_guard_reads() {
    // The first PoC run could use write fs_permissions to rewrite the other
    // copy or the global config to ffi = true, and the second run would then
    // execute ffi. The config must be refused before either run.

    // 1. The root tree.
    let dir = scratch("fs-root");
    let project = dir.join("project");
    std::fs::write(project.join("foundry.toml"), fs_config("write")).unwrap();
    let (forge, marker) = fake_forge(&dir);
    let out = verify_fix(&project, &forge, Some(&project.join("patched")), None);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(2), "{stderr}");
    assert!(stderr.contains("fs_permissions"), "{stderr}");
    assert!(!marker.exists(), "forge ran despite write fs_permissions");
    let _ = std::fs::remove_dir_all(&dir);

    // 2. The --patched tree.
    let dir = scratch("fs-patched");
    let project = dir.join("project");
    std::fs::write(project.join("patched/foundry.toml"), fs_config("write")).unwrap();
    let (forge, marker) = fake_forge(&dir);
    let out = verify_fix(&project, &forge, Some(&project.join("patched")), None);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(2), "{stderr}");
    assert!(stderr.contains("fs_permissions"), "{stderr}");
    assert!(!marker.exists(), "forge ran despite write fs_permissions");
    let _ = std::fs::remove_dir_all(&dir);

    // 3. The copy after --patch.
    let dir = scratch("fs-post-patch");
    let project = dir.join("project");
    let patch = dir.join("fs.diff");
    std::fs::write(&patch, replacement_patch(&fs_config("write"))).unwrap();
    let (forge, marker) = fake_forge(&dir);
    let out = verify_fix(&project, &forge, None, Some(&patch));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(2), "{stderr}");
    assert!(stderr.contains("fs_permissions"), "{stderr}");
    assert!(!marker.exists(), "forge ran despite write fs_permissions");
    let _ = std::fs::remove_dir_all(&dir);

    // 4. The global config.
    let dir = scratch("fs-global");
    let project = dir.join("project");
    std::fs::create_dir_all(dir.join("home/.foundry")).unwrap();
    std::fs::write(dir.join("home/.foundry/foundry.toml"), fs_config("write")).unwrap();
    let (forge, marker) = fake_forge(&dir);
    let out = verify_fix(&project, &forge, Some(&project.join("patched")), None);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(2), "{stderr}");
    assert!(stderr.contains("fs_permissions"), "{stderr}");
    assert!(!marker.exists(), "forge ran despite write fs_permissions");
    let _ = std::fs::remove_dir_all(&dir);

    // 5. read-write is refused the same way.
    let dir = scratch("fs-read-write");
    let project = dir.join("project");
    std::fs::write(project.join("foundry.toml"), fs_config("read-write")).unwrap();
    let (forge, marker) = fake_forge(&dir);
    let out = verify_fix(&project, &forge, Some(&project.join("patched")), None);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(2), "{stderr}");
    assert!(stderr.contains("read-write"), "{stderr}");
    assert!(
        !marker.exists(),
        "forge ran despite read-write fs_permissions"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn read_only_fs_permissions_reach_forge() {
    // The control: a read-only entry is allowed, so the refusals above are
    // about write access and not about fs_permissions itself.
    let dir = scratch("fs-read");
    let project = dir.join("project");
    std::fs::write(project.join("foundry.toml"), fs_config("read")).unwrap();
    let (forge, marker) = fake_forge(&dir);
    let out = verify_fix(&project, &forge, Some(&project.join("patched")), None);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        marker.exists(),
        "a read-only config never reached forge: {stderr}"
    );
    assert_eq!(out.status.code(), Some(1), "{stderr}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn fs_permissions_aliases_are_refused_in_every_config_the_guard_reads() {
    for key in FS_ALIASES {
        // 1. The root tree, with a write value.
        let dir = scratch("fs-alias-root");
        let project = dir.join("project");
        std::fs::write(project.join("foundry.toml"), fs_alias_config(key, "write")).unwrap();
        let (forge, marker) = fake_forge(&dir);
        let out = verify_fix(&project, &forge, Some(&project.join("patched")), None);
        refused_fs("root write", key, &out, &marker);
        let _ = std::fs::remove_dir_all(&dir);

        // 2. The root tree, with a read-only value: the alias itself is
        // refused, so a read-only alias cannot be used to test the rule.
        let dir = scratch("fs-alias-read");
        let project = dir.join("project");
        std::fs::write(project.join("foundry.toml"), fs_alias_config(key, "read")).unwrap();
        let (forge, marker) = fake_forge(&dir);
        let out = verify_fix(&project, &forge, Some(&project.join("patched")), None);
        refused_fs("root read", key, &out, &marker);
        let _ = std::fs::remove_dir_all(&dir);

        // 3. The --patched tree.
        let dir = scratch("fs-alias-patched");
        let project = dir.join("project");
        std::fs::write(
            project.join("patched/foundry.toml"),
            fs_alias_config(key, "write"),
        )
        .unwrap();
        let (forge, marker) = fake_forge(&dir);
        let out = verify_fix(&project, &forge, Some(&project.join("patched")), None);
        refused_fs("--patched", key, &out, &marker);
        let _ = std::fs::remove_dir_all(&dir);

        // 4. The copied tree after --patch is applied.
        let dir = scratch("fs-alias-patch");
        let project = dir.join("project");
        let patch = dir.join("fs-alias.diff");
        std::fs::write(&patch, replacement_patch(&fs_alias_config(key, "write"))).unwrap();
        let (forge, marker) = fake_forge(&dir);
        let out = verify_fix(&project, &forge, None, Some(&patch));
        refused_fs("--patch", key, &out, &marker);
        let _ = std::fs::remove_dir_all(&dir);

        // 5. The global config.
        let dir = scratch("fs-alias-global");
        let project = dir.join("project");
        global_config(&dir, &fs_alias_config(key, "write"));
        let (forge, marker) = fake_forge(&dir);
        let out = verify_fix(&project, &forge, Some(&project.join("patched")), None);
        refused_fs("global", key, &out, &marker);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(stderr.contains("global config"), "{key:?}: {stderr}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn a_global_config_with_an_ffi_alias_is_refused_before_forge_starts() {
    for spelling in SPELLINGS {
        let dir = scratch("global-alias-ffi");
        let project = dir.join("project");
        global_config(&dir, spelling);
        let (forge, marker) = fake_forge(&dir);
        let out = verify_fix(&project, &forge, Some(&project.join("patched")), None);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.status.code(), Some(2), "{spelling:?}: {stderr}");
        assert!(
            stderr.contains("global config") && stderr.to_ascii_lowercase().contains("ffi"),
            "{spelling:?}: {stderr}"
        );
        assert!(
            !marker.exists(),
            "{spelling:?}: forge ran despite the global config"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn a_global_config_with_a_fs_permissions_alias_is_refused_before_forge_starts() {
    for key in FS_ALIASES {
        let dir = scratch("global-alias-fs");
        let project = dir.join("project");
        global_config(&dir, &fs_alias_config(key, "write"));
        let (forge, marker) = fake_forge(&dir);
        let out = verify_fix(&project, &forge, Some(&project.join("patched")), None);
        refused_fs("global", key, &out, &marker);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(stderr.contains("global config"), "{key:?}: {stderr}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn a_minimal_top_level_ffi_alias_is_refused_before_forge() {
    // The reviewer's minimal single-run bypass: one top-level alias key.
    let dir = scratch("minimal-ffi-alias");
    let project = dir.join("project");
    std::fs::write(project.join("foundry.toml"), "FFI = true\n").unwrap();
    let (forge, marker) = fake_forge(&dir);
    let out = verify_fix(&project, &forge, Some(&project.join("patched")), None);
    refused("minimal", "FFI = true\n", &out, &marker);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_alias_cross_run_combination_is_refused_before_forge() {
    // The reviewer's cross-run combination: an fs_permissions alias that
    // grants write and an ffi alias, in the same config.
    let dir = scratch("alias-combined");
    let project = dir.join("project");
    std::fs::write(
        project.join("foundry.toml"),
        "FFI = true\nFS_PERMISSIONS = [{ access = \"write\", path = \".\" }]\n",
    )
    .unwrap();
    let (forge, marker) = fake_forge(&dir);
    let out = verify_fix(&project, &forge, Some(&project.join("patched")), None);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(2), "{stderr}");
    assert!(
        stderr.to_ascii_lowercase().contains("ffi") || stderr.contains("fs_permissions"),
        "{stderr}"
    );
    assert!(
        !marker.exists(),
        "forge ran despite the alias combination: {stderr}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_global_config_rewritten_by_the_first_run_is_refused_before_the_second() {
    // The first run rewrites the global config to an ffi alias. The second
    // revalidation must refuse it, so forge is started exactly once.
    let dir = scratch("revalidate-global");
    let project = dir.join("project");
    let home = dir.join("home");
    let (forge, counter) = rewriting_forge(&dir, &home);
    let out = verify_fix(&project, &forge, Some(&project.join("patched")), None);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(2), "{stderr}");
    assert!(
        stderr.contains("global config") && stderr.to_ascii_lowercase().contains("ffi"),
        "{stderr}"
    );
    let runs = std::fs::read_to_string(&counter).unwrap_or_default();
    assert_eq!(
        runs.lines().count(),
        1,
        "forge was started again after the global config was rewritten: {stderr}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
