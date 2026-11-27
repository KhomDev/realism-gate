//! `mcp::run_bounded`: the deadline, the output cap, the exit status and the
//! null stdin, against a real child process. The child is this test binary
//! itself, re-run on `helper_child` with a mode in `RG_MCP_CHILD`.

use realism_gate::mcp::run_bounded;
use std::io::{Read, Write};
use std::process::Command;
use std::time::{Duration, Instant};

/// Not a test of its own: with `RG_MCP_CHILD` unset it does nothing.
#[test]
fn helper_child() {
    match std::env::var("RG_MCP_CHILD").as_deref() {
        Ok("sleep") => std::thread::sleep(Duration::from_secs(120)),
        Ok("flood") => {
            let chunk = vec![b'x'; 1 << 20];
            let mut out = std::io::stdout().lock();
            for _ in 0..6 {
                if out.write_all(&chunk).is_err() {
                    break;
                }
            }
            let _ = out.flush();
        }
        Ok("flood-stderr") => {
            let chunk = vec![b'y'; 1 << 20];
            let mut err = std::io::stderr().lock();
            for _ in 0..6 {
                if err.write_all(&chunk).is_err() {
                    break;
                }
            }
        }
        Ok("stdin") => {
            let mut s = Vec::new();
            let _ = std::io::stdin().read_to_end(&mut s);
            println!("RG_STDIN_BYTES={}", s.len());
        }
        Ok("exit3") => std::process::exit(3),
        Ok("both") => {
            println!("RG_OUT");
            eprintln!("RG_ERR");
        }
        _ => {}
    }
}

fn child(mode: &str) -> Command {
    let mut c = Command::new(std::env::current_exe().unwrap());
    c.args(["helper_child", "--exact", "--nocapture", "--test-threads=1"])
        .env("RG_MCP_CHILD", mode);
    c
}

const CAP: usize = 4 * 1024 * 1024;

#[test]
fn a_run_over_the_deadline_is_killed_and_reported() {
    let started = Instant::now();
    let err = run_bounded(&mut child("sleep"), Duration::from_secs(1), CAP)
        .err()
        .expect("the sleeper times out");
    assert!(err.contains("killed"), "{err}");
    assert!(err.contains("1 s"), "{err}");
    assert!(
        started.elapsed() < Duration::from_secs(30),
        "the deadline was not enforced: {:?}",
        started.elapsed()
    );
}

#[test]
fn output_over_the_cap_is_refused_not_cropped() {
    for (mode, stream) in [("flood", "stdout"), ("flood-stderr", "stderr")] {
        let err = run_bounded(&mut child(mode), Duration::from_secs(60), CAP)
            .err()
            .expect("6 MiB is over a 4 MiB cap");
        assert!(err.contains("tool-output limit"), "{err}");
        assert!(err.contains(stream), "{err}");
    }
}

#[test]
fn output_under_the_cap_is_kept_whole() {
    let ran = run_bounded(
        &mut child("flood"),
        Duration::from_secs(60),
        64 * 1024 * 1024,
    )
    .unwrap();
    assert!(
        ran.stdout.matches('x').count() == 6 << 20,
        "every byte kept"
    );
}

#[test]
fn the_child_reads_a_closed_stdin_not_the_mcp_stream() {
    let ran = run_bounded(&mut child("stdin"), Duration::from_secs(60), CAP).unwrap();
    assert!(ran.stdout.contains("RG_STDIN_BYTES=0"), "{}", ran.stdout);
}

#[test]
fn the_exit_status_and_both_streams_come_back() {
    let ran = run_bounded(&mut child("exit3"), Duration::from_secs(60), CAP).unwrap();
    assert_eq!(ran.code, 3);
    let ran = run_bounded(&mut child("both"), Duration::from_secs(60), CAP).unwrap();
    assert_eq!(ran.code, 0);
    assert!(ran.stdout.contains("RG_OUT"), "{}", ran.stdout);
    assert!(ran.stderr.contains("RG_ERR"), "{}", ran.stderr);
}

#[test]
fn a_program_that_does_not_exist_is_an_error_naming_it() {
    let err = run_bounded(
        &mut Command::new("realism-gate-definitely-not-installed-xyz"),
        Duration::from_secs(5),
        CAP,
    )
    .err()
    .unwrap();
    assert!(
        err.contains("realism-gate-definitely-not-installed-xyz"),
        "{err}"
    );
}
