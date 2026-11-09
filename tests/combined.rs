//! The combined-report script.

use std::process::Command;

#[test]
fn the_merge_script_accepts_its_own_sample() {
    let out = Command::new("python3")
        .args(["combined/merge_reports.py", "--self-test"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("python3");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
