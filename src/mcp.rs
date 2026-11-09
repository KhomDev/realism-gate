//! A small MCP server: `check_poc` and `check_finding`.
//!
//! The server speaks MCP over stdio (Content-Length framing) and shells out
//! to `realism-gate` and `vg`. It does not embed either checker. A test
//! passes a runner so the protocol can be checked without those binaries.

use serde_json::{Value, json};
use std::io::{BufRead, Write};

pub struct Ran {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

/// What `check_poc` and `check_finding` shell out through.
type Runner<'a> = dyn FnMut(&str, &[String]) -> Result<Ran, String> + 'a;

/// Handle one JSON-RPC message. `None` for a notification, which has no id
/// and gets no response.
pub fn dispatch(msg: &Value, run: &mut Runner<'_>) -> Option<Value> {
    let id = msg.get("id").cloned()?;
    let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
    let params = msg.get("params").cloned().unwrap_or(Value::Null);
    let result = match method {
        "initialize" => Ok(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": { "listChanged": false } },
            "serverInfo": { "name": "realism-gate", "version": env!("CARGO_PKG_VERSION") }
        })),
        "tools/list" => Ok(json!({ "tools": tools() })),
        "tools/call" => call(&params, run),
        "ping" => Ok(json!({})),
        other => Err(format!("unknown method `{other}`")),
    };
    Some(match result {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err(message) => json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32601, "message": message }
        }),
    })
}

fn tools() -> Vec<Value> {
    vec![
        json!({
            "name": "check_poc",
            "description": "Run realism-gate on a PoC test. Returns the JSON report. isError when a finding is at or above fail_on (default warn).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "A test file or directory." },
                    "declaration": { "type": "string", "description": "Path to realism.yaml." },
                    "fail_on": { "type": "string", "enum": ["error", "warn", "note", "never"], "description": "Default warn." }
                },
                "required": ["path"]
            }
        }),
        json!({
            "name": "check_finding",
            "description": "Run viability-gate (`vg check`) on a draft finding, and realism-gate on a PoC when one is given. isError on a KILL or HOLD outcome, or on a PoC finding at or above warn.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "finding": { "type": "string", "description": "Path to the draft finding (markdown)." },
                    "poc": { "type": "string", "description": "Optional PoC test to check with realism-gate." },
                    "declaration": { "type": "string" }
                },
                "required": ["finding"]
            }
        }),
    ]
}

fn call(params: &Value, run: &mut Runner<'_>) -> Result<Value, String> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or("tools/call needs a name")?;
    let args = params.get("arguments").cloned().unwrap_or(json!({}));
    let (text, is_error) = match name {
        "check_poc" => check_poc(&args, run)?,
        "check_finding" => check_finding(&args, run)?,
        other => return Err(format!("unknown tool `{other}`")),
    };
    Ok(json!({
        "content": [{ "type": "text", "text": text }],
        "isError": is_error
    }))
}

fn check_poc(args: &Value, run: &mut Runner<'_>) -> Result<(String, bool), String> {
    let path = args
        .get("path")
        .and_then(Value::as_str)
        .ok_or("check_poc needs path")?;
    let fail_on = args
        .get("fail_on")
        .and_then(Value::as_str)
        .unwrap_or("warn");
    let mut argv = vec![
        "check".into(),
        path.into(),
        "--format".into(),
        "json".into(),
        "--fail-on".into(),
        fail_on.into(),
    ];
    if let Some(d) = args.get("declaration").and_then(Value::as_str) {
        argv.push("--declaration".into());
        argv.push(d.into());
    }
    let ran = run("realism-gate", &argv)?;
    let text = if ran.stdout.trim().is_empty() {
        ran.stderr.clone()
    } else {
        ran.stdout.clone()
    };
    Ok((text, ran.code != 0))
}

fn check_finding(args: &Value, run: &mut Runner<'_>) -> Result<(String, bool), String> {
    let finding = args
        .get("finding")
        .and_then(Value::as_str)
        .ok_or("check_finding needs finding")?;
    let vg = run("vg", &["check".into(), finding.into(), "--json".into()])?;
    let mut text = format!("# viability-gate\n{}", vg.stdout);
    if !vg.stderr.trim().is_empty() {
        text.push('\n');
        text.push_str(&vg.stderr);
    }
    let mut is_error = vg.code != 0 || outcome_blocks(&vg.stdout);
    if let Some(poc) = args.get("poc").and_then(Value::as_str) {
        let mut argv = vec![
            "check".into(),
            poc.into(),
            "--format".into(),
            "json".into(),
            "--fail-on".into(),
            "warn".into(),
        ];
        if let Some(d) = args.get("declaration").and_then(Value::as_str) {
            argv.push("--declaration".into());
            argv.push(d.into());
        }
        let rg = run("realism-gate", &argv)?;
        text.push_str("\n# realism-gate\n");
        text.push_str(&rg.stdout);
        if rg.code != 0 {
            is_error = true;
        }
    }
    Ok((text, is_error))
}

/// KILL and HOLD are the outcomes that should stop a report. CLEAR and
/// INCOMPLETE are not, by themselves, a failed tool call: INCOMPLETE means a
/// gate had no input, which the text already says.
fn outcome_blocks(stdout: &str) -> bool {
    let Ok(v) = serde_json::from_str::<Value>(stdout.trim()) else {
        return stdout.contains("\"outcome\": \"KILL\"")
            || stdout.contains("\"outcome\": \"HOLD\"");
    };
    let reports = if v.is_array() {
        v.as_array().cloned().unwrap()
    } else {
        vec![v]
    };
    reports.iter().any(|r| {
        matches!(
            r.get("outcome").and_then(Value::as_str),
            Some("KILL" | "HOLD")
        )
    })
}

/// Read one MCP frame from `input`. Also accepts a bare JSON line, which is
/// what the unit tests write.
pub fn read_frame(input: &mut dyn BufRead) -> Result<Option<Value>, String> {
    let mut line = String::new();
    let n = input
        .read_line(&mut line)
        .map_err(|e| format!("stdin: {e}"))?;
    if n == 0 {
        return Ok(None);
    }
    if line.to_ascii_lowercase().starts_with("content-length:") {
        let len: usize = line
            .split(':')
            .nth(1)
            .unwrap_or("")
            .trim()
            .parse()
            .map_err(|_| format!("bad content-length: {line}"))?;
        // The blank line that ends the headers.
        let mut blank = String::new();
        loop {
            blank.clear();
            let n = input
                .read_line(&mut blank)
                .map_err(|e| format!("stdin: {e}"))?;
            if n == 0 {
                return Err("stdin ended in the headers".into());
            }
            if blank.trim().is_empty() {
                break;
            }
        }
        let mut buf = vec![0u8; len];
        input
            .read_exact(&mut buf)
            .map_err(|e| format!("stdin body: {e}"))?;
        let v = serde_json::from_slice(&buf).map_err(|e| format!("request: {e}"))?;
        return Ok(Some(v));
    }
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return read_frame(input);
    }
    serde_json::from_str(trimmed)
        .map(Some)
        .map_err(|e| format!("request: {e}"))
}

pub fn write_frame(output: &mut dyn Write, msg: &Value) -> Result<(), String> {
    let body = serde_json::to_vec(msg).map_err(|e| format!("response: {e}"))?;
    write!(output, "Content-Length: {}\r\n\r\n", body.len()).map_err(|e| e.to_string())?;
    output.write_all(&body).map_err(|e| e.to_string())?;
    output.flush().map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scripted(program: &str) -> Ran {
        let stdout = if program == "vg" {
            r#"{"outcome":"KILL","viability_score":40}"#.to_string()
        } else {
            r#"{"summary":{"errors":1}}"#.to_string()
        };
        Ran {
            code: 1,
            stdout,
            stderr: String::new(),
        }
    }

    #[test]
    fn initialize_and_list() {
        let calls = std::cell::RefCell::new(Vec::new());
        let mut run = |program: &str, args: &[String]| {
            calls
                .borrow_mut()
                .push((program.to_string(), args.to_vec()));
            Ok(scripted(program))
        };
        let msg = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}});
        let out = dispatch(&msg, &mut run).unwrap();
        assert_eq!(out["result"]["serverInfo"]["name"], "realism-gate");
        let msg = json!({"jsonrpc":"2.0","id":2,"method":"tools/list"});
        let out = dispatch(&msg, &mut run).unwrap();
        let names: Vec<&str> = out["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, vec!["check_poc", "check_finding"]);
    }

    #[test]
    fn check_poc_shells_out_and_a_finding_is_an_error() {
        let calls = std::cell::RefCell::new(Vec::new());
        let mut run = |program: &str, args: &[String]| {
            calls
                .borrow_mut()
                .push((program.to_string(), args.to_vec()));
            Ok(scripted(program))
        };
        let msg = json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": { "name": "check_poc", "arguments": { "path": "test/Poc.t.sol" } }
        });
        let out = dispatch(&msg, &mut run).unwrap();
        assert_eq!(out["result"]["isError"], true);
        let calls = calls.borrow();
        assert_eq!(calls[0].0, "realism-gate");
        assert!(calls[0].1.iter().any(|a| a == "test/Poc.t.sol"));
        assert!(calls[0].1.iter().any(|a| a == "warn"));
    }

    #[test]
    fn check_finding_runs_both_and_kill_is_an_error() {
        let calls = std::cell::RefCell::new(Vec::new());
        let mut run = |program: &str, args: &[String]| {
            calls
                .borrow_mut()
                .push((program.to_string(), args.to_vec()));
            Ok(scripted(program))
        };
        let msg = json!({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "tools/call",
            "params": {
                "name": "check_finding",
                "arguments": { "finding": "finding.md", "poc": "test/Poc.t.sol" }
            }
        });
        let out = dispatch(&msg, &mut run).unwrap();
        assert_eq!(out["result"]["isError"], true);
        let calls = calls.borrow();
        assert_eq!(calls[0].0, "vg");
        assert_eq!(calls[1].0, "realism-gate");
        let text = out["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("KILL"), "{text}");
    }

    #[test]
    fn a_notification_gets_no_response() {
        let mut run = |_program: &str, _args: &[String]| Ok(scripted("realism-gate"));
        let msg = json!({"jsonrpc":"2.0","method":"notifications/initialized"});
        assert!(dispatch(&msg, &mut run).is_none());
    }

    #[test]
    fn frames_round_trip() {
        let msg = json!({"jsonrpc":"2.0","id":1,"method":"ping"});
        let mut buf = Vec::new();
        write_frame(&mut buf, &msg).unwrap();
        let got = read_frame(&mut std::io::Cursor::new(buf)).unwrap().unwrap();
        assert_eq!(got, msg);
    }
}
