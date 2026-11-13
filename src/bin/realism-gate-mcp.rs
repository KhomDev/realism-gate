//! MCP stdio server. See `src/mcp.rs` for the tools.
//!
//! The binary is `REALISM_GATE_BIN` when set, else a sibling of this
//! executable, else `realism-gate` on `PATH`. There is no second tool to
//! find: the two questions (does the proof prove the claim, is it new) are
//! separate programs with no pin between them.

use realism_gate::mcp::{self, Ran};
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let mut stdin = std::io::stdin().lock();
    let mut stdout = std::io::stdout().lock();
    loop {
        let msg = match mcp::read_frame(&mut stdin as &mut dyn BufRead) {
            Ok(Some(v)) => v,
            Ok(None) => break,
            Err(e) => {
                let _ = writeln!(std::io::stderr(), "realism-gate-mcp: {e}");
                std::process::exit(2);
            }
        };
        if let Some(response) = mcp::dispatch(&msg, &mut shell_out)
            && let Err(e) = mcp::write_frame(&mut stdout as &mut dyn Write, &response)
        {
            let _ = writeln!(std::io::stderr(), "realism-gate-mcp: {e}");
            std::process::exit(2);
        }
    }
}

fn shell_out(args: &[String]) -> Result<Ran, String> {
    let bin = resolve();
    let output = Command::new(&bin)
        .args(args)
        .output()
        .map_err(|e| format!("could not run {}: {e}", bin.display()))?;
    Ok(Ran {
        code: output.status.code().unwrap_or(1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

fn resolve() -> PathBuf {
    if let Some(p) = std::env::var_os("REALISM_GATE_BIN") {
        return PathBuf::from(p);
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        let sibling = dir.join("realism-gate");
        if sibling.is_file() {
            return sibling;
        }
        let sibling = dir.join("realism-gate.exe");
        if sibling.is_file() {
            return sibling;
        }
    }
    PathBuf::from("realism-gate")
}
