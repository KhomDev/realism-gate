#![forbid(unsafe_code)]
//! MCP stdio server (`io.github.khomdev/rgate`). See `src/mcp.rs` for the
//! tools and the transport.
//!
//! The checker is `REALISM_GATE_BIN` when set, else a sibling of this
//! executable, else `realism-gate` on `PATH`. The operator, not the client,
//! sets two limits: `REALISM_GATE_MCP_ROOT` (every path argument must resolve
//! inside it) and `REALISM_GATE_MCP_TIMEOUT_SECS` (default 120, 1 to 3600).

use realism_gate::mcp::{self, Policy, Ran};
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

const HELP: &str =
    "realism-gate-mcp: the realism-gate MCP server (MCP Registry: io.github.khomdev/rgate)

Speaks MCP over stdio: one JSON-RPC message per line on stdin and stdout.
Content-Length framed requests are also read and answered in kind.
Tools: check_poc, check_claims, dump_program, list_rules, explain_rule.

Environment:
  REALISM_GATE_BIN               the checker binary (default: next to this binary, then PATH)
  REALISM_GATE_MCP_ROOT          refuse any path argument that resolves outside this directory
  REALISM_GATE_MCP_TIMEOUT_SECS  kill a checker run after this many seconds (default 120)

Configure it in your MCP client with the command `realism-gate-mcp`.";

fn main() {
    if let Some(arg) = std::env::args().nth(1) {
        match arg.as_str() {
            "--version" | "-V" => {
                println!("realism-gate-mcp {}", env!("CARGO_PKG_VERSION"));
                return;
            }
            "--help" | "-h" => {
                println!("{HELP}");
                return;
            }
            other => {
                eprintln!("realism-gate-mcp: unknown argument `{other}`; see --help");
                std::process::exit(2);
            }
        }
    }
    let policy = match std::env::var_os("REALISM_GATE_MCP_ROOT") {
        Some(root) if !root.is_empty() => match Policy::with_root(&PathBuf::from(root)) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("realism-gate-mcp: {e}");
                std::process::exit(2);
            }
        },
        _ => Policy::default(),
    };
    let timeout = match timeout_from_env() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("realism-gate-mcp: {e}");
            std::process::exit(2);
        }
    };
    let mut stdin = std::io::stdin().lock();
    let mut stdout = std::io::stdout().lock();
    let mut run = |args: &[String]| -> Result<Ran, String> {
        let bin = resolve();
        mcp::run_bounded(
            Command::new(&bin).args(args),
            timeout,
            mcp::MAX_TOOL_OUTPUT_BYTES,
        )
    };
    // Every framing error stops the server. An oversized frame is answered
    // with a JSON-RPC error first, because the client gets a reason for the
    // stop; the unread body is not trusted enough to resynchronise past it.
    if let Err(e) = mcp::serve(
        &mut stdin as &mut dyn BufRead,
        &mut stdout as &mut dyn Write,
        &policy,
        &mut run,
    ) {
        let _ = writeln!(std::io::stderr(), "realism-gate-mcp: {e}");
        std::process::exit(2);
    }
}

fn timeout_from_env() -> Result<Duration, String> {
    let Some(raw) = std::env::var_os("REALISM_GATE_MCP_TIMEOUT_SECS") else {
        return Ok(mcp::DEFAULT_TIMEOUT);
    };
    let raw = raw.to_string_lossy();
    match raw.trim().parse::<u64>() {
        Ok(s) if (1..=3600).contains(&s) => Ok(Duration::from_secs(s)),
        _ => Err(format!(
            "REALISM_GATE_MCP_TIMEOUT_SECS must be a whole number of seconds from 1 to 3600, not `{raw}`"
        )),
    }
}

fn resolve() -> PathBuf {
    if let Some(p) = std::env::var_os("REALISM_GATE_BIN") {
        return PathBuf::from(p);
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        for name in ["realism-gate", "realism-gate.exe"] {
            let sibling = dir.join(name);
            if sibling.is_file() {
                return sibling;
            }
        }
    }
    PathBuf::from("realism-gate")
}
