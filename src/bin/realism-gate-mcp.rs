//! MCP stdio server. See `src/mcp.rs` for the tools.
//!
//! Binaries: `REALISM_GATE_BIN` and `VG_BIN`, else a sibling of this
//! executable, else `realism-gate` and `vg` on `PATH`.

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

fn shell_out(program: &str, args: &[String]) -> Result<Ran, String> {
    let bin = resolve(program);
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

fn resolve(program: &str) -> PathBuf {
    let env_key = match program {
        "vg" => "VG_BIN",
        _ => "REALISM_GATE_BIN",
    };
    if let Some(p) = std::env::var_os(env_key) {
        return PathBuf::from(p);
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        let sibling = dir.join(program);
        if sibling.is_file() {
            return sibling;
        }
    }
    PathBuf::from(program)
}
