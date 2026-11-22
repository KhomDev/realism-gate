//! `verify-fix` on the host: what the decision trusts. The sandbox attestation
//! is read from the environment (`REALISM_GATE_SANDBOX=1`) and from two marker
//! files, so an agent with a shell can satisfy it. Changing that policy is a
//! separate decision (see docs/threat-model.md, H1). These tests pin what the
//! current policy does, so a change to it is a visible change.
//!
//! The forge binary named here does not exist. The host decision comes before
//! forge runs, so the error text shows which branch was taken: the refusal, or
//! the attempt to run forge.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn scratch(name: &str) -> PathBuf {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "rg-vf-limits-{name}-{}-{nanos}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("patched")).unwrap();
    std::fs::create_dir_all(dir.join("test")).unwrap();
    std::fs::create_dir_all(dir.join("home")).unwrap();
    std::fs::write(dir.join("test/Poc.t.sol"), "contract Poc {}\n").unwrap();
    dir
}

/// True when this machine is a container, where the marker files exist and
/// the no-attestation case cannot be tested.
fn in_a_container() -> bool {
    Path::new("/.dockerenv").is_file() || Path::new("/run/.containerenv").is_file()
}

fn verify_fix(dir: &Path, sandbox_env: Option<&str>) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_realism-gate"));
    cmd.arg("verify-fix")
        .arg("--root")
        .arg(dir)
        .arg("--poc")
        .arg("test/Poc.t.sol")
        .arg("--patched")
        .arg(dir.join("patched"))
        .arg("--forge")
        .arg(dir.join("no-such-forge"))
        .current_dir(dir)
        // A developer's real global Foundry config must not change a test.
        .env("HOME", dir.join("home"))
        .env("USERPROFILE", dir.join("home"))
        .env_remove("REALISM_GATE_SANDBOX");
    if let Some(v) = sandbox_env {
        cmd.env("REALISM_GATE_SANDBOX", v);
    }
    cmd.output().expect("binary runs")
}

const REFUSED: &str = "will not run an untrusted forge project on the host";
const RAN: &str = "could not run";

#[test]
fn without_an_attestation_the_host_run_is_refused() {
    if in_a_container() {
        eprintln!("running in a container; the no-attestation case is skipped");
        return;
    }
    let dir = scratch("refused");
    let o = verify_fix(&dir, None);
    let err = String::from_utf8_lossy(&o.stderr);
    assert_eq!(o.status.code(), Some(2), "{err}");
    assert!(err.contains(REFUSED), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_environment_variable_alone_is_taken_as_the_attestation() {
    // Pinned, not endorsed: the variable is not a proof that the process is
    // confined. An agent that can set it reaches the host branch.
    let dir = scratch("env-one");
    let o = verify_fix(&dir, Some("1"));
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(err.contains(RAN), "the host branch was not taken: {err}");
    assert!(!err.contains(REFUSED), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn only_the_value_one_counts_as_the_environment_attestation() {
    if in_a_container() {
        eprintln!("running in a container; the refusal branch is not reachable");
        return;
    }
    for value in ["0", "true", "yes", ""] {
        let dir = scratch("env-other");
        let o = verify_fix(&dir, Some(value));
        let err = String::from_utf8_lossy(&o.stderr);
        assert!(
            err.contains(REFUSED),
            "REALISM_GATE_SANDBOX={value:?} was taken as an attestation: {err}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn the_sandbox_flag_is_the_explicit_attestation() {
    let dir = scratch("flag");
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_realism-gate"));
    let o = cmd
        .arg("verify-fix")
        .arg("--sandbox")
        .arg("--root")
        .arg(&dir)
        .arg("--poc")
        .arg("test/Poc.t.sol")
        .arg("--patched")
        .arg(dir.join("patched"))
        .arg("--forge")
        .arg(dir.join("no-such-forge"))
        .env("HOME", dir.join("home"))
        .env("USERPROFILE", dir.join("home"))
        .env_remove("REALISM_GATE_SANDBOX")
        .output()
        .expect("binary runs");
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(err.contains(RAN), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}
