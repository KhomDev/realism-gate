//! The copy `verify-fix` makes is attacker-controlled. File count, total
//! bytes and directory depth are capped before forge runs, and a forge stand-in
//! proves the refusal happens before any process is started.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const MAX_COPY_BYTES: u64 = 512 * 1024 * 1024;

fn scratch(name: &str) -> PathBuf {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir =
        std::env::temp_dir().join(format!("rg-vf-copy-{name}-{}-{nanos}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let project = dir.join("project");
    std::fs::create_dir_all(project.join("test")).unwrap();
    std::fs::create_dir_all(project.join("patched")).unwrap();
    std::fs::create_dir_all(dir.join("home")).unwrap();
    std::fs::write(project.join("test/Poc.t.sol"), "contract Poc {}\n").unwrap();
    std::fs::write(
        project.join("foundry.toml"),
        "[profile.default]\nffi = false\n",
    )
    .unwrap();
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

fn verify_fix(project: &Path, forge: &Path) -> Output {
    let home = project
        .parent()
        .expect("the scratch project has a parent")
        .join("home");
    Command::new(env!("CARGO_BIN_EXE_realism-gate"))
        .arg("verify-fix")
        .arg("--sandbox")
        .arg("--root")
        .arg(project)
        .arg("--poc")
        .arg("test/Poc.t.sol")
        .arg("--patched")
        .arg(project.join("patched"))
        .arg("--forge")
        .arg(forge)
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .env_remove("REALISM_GATE_SANDBOX")
        .output()
        .expect("binary runs")
}

#[test]
fn a_tree_larger_than_the_copy_limit_is_refused_before_forge_starts() {
    let dir = scratch("bytes");
    let project = dir.join("project");
    let huge = project.join("huge.bin");
    let file = std::fs::File::create(&huge).unwrap();
    file.set_len(MAX_COPY_BYTES + 1).unwrap();
    drop(file);
    let (forge, marker) = fake_forge(&dir);
    let out = verify_fix(&project, &forge);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(2), "{stderr}");
    assert!(
        stderr.contains(&MAX_COPY_BYTES.to_string()) && stderr.contains("will not copy"),
        "{stderr}"
    );
    assert!(!marker.exists(), "forge ran despite the copy cap");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_tree_deeper_than_the_copy_limit_is_refused_before_forge_starts() {
    let dir = scratch("depth");
    let project = dir.join("project");
    let mut deep = project.clone();
    for _ in 0..70 {
        deep.push("d");
        std::fs::create_dir(&deep).unwrap();
    }
    let (forge, marker) = fake_forge(&dir);
    let out = verify_fix(&project, &forge);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(2), "{stderr}");
    assert!(
        stderr.contains("64-directory limit") && stderr.contains("will not copy"),
        "{stderr}"
    );
    assert!(!marker.exists(), "forge ran despite the depth cap");
    let _ = std::fs::remove_dir_all(&dir);
}
