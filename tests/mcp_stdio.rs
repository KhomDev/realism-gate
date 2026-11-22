//! Drives the stdio MCP binary. The framing is client-controlled, so an
//! oversized `Content-Length` must produce a JSON-RPC error and leave the
//! process in control of itself, not an allocation of the requested size.

use std::io::{Read, Write};
use std::process::{Command, Stdio};

/// Reads one `Content-Length` frame from `input`.
fn read_frame(input: &mut dyn Read) -> Vec<u8> {
    let mut header = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        input.read_exact(&mut byte).unwrap();
        header.push(byte[0]);
        if header.ends_with(b"\r\n\r\n") {
            break;
        }
        assert!(header.len() < 8192, "the header did not end");
    }
    let text = String::from_utf8_lossy(&header);
    let len: usize = text
        .lines()
        .next()
        .and_then(|l| l.strip_prefix("Content-Length: "))
        .expect("a Content-Length header")
        .trim()
        .parse()
        .expect("a numeric Content-Length");
    let mut body = vec![0u8; len];
    input.read_exact(&mut body).unwrap();
    body
}

#[test]
fn an_oversized_content_length_gets_a_jsonrpc_error_and_a_clean_exit() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_realism-gate-mcp"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the MCP binary runs");
    {
        let mut stdin = child.stdin.take().unwrap();
        // No body follows: closing the pipe is how a client disappears. The
        // server must answer from the header alone rather than wait for the
        // declared gigabyte.
        stdin
            .write_all(b"Content-Length: 999999999\r\n\r\n")
            .unwrap();
    }
    let out = child.wait_with_output().expect("the server exits");

    let body = read_frame(&mut &out.stdout[..]);
    let value: serde_json::Value = serde_json::from_slice(&body).expect("a JSON-RPC response");
    assert!(value["id"].is_null(), "{value}");
    assert_eq!(value["error"]["code"], -32600, "{value}");
    assert!(
        value["error"]["message"]
            .as_str()
            .is_some_and(|m| m.contains("frame limit")),
        "{value}"
    );
    assert!(
        out.status.success(),
        "the server did not stay in control: {:?}: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
}
