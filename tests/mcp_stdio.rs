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
fn an_oversized_frame_is_answered_and_the_server_stops_without_serving_a_ping() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_realism-gate-mcp"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the MCP binary runs");
    {
        let mut stdin = child.stdin.take().unwrap();
        // The declared body really follows, and then a well-formed ping. The
        // server must answer the oversized frame from the header, stop with
        // exit 2, and never treat the 9 MB body as a line (which used to make
        // the next read a fatal line-cap error before the ping was served).
        let mut request = b"Content-Length: 9000000\r\n\r\n".to_vec();
        request.extend(std::iter::repeat_n(b'x', 9_000_000));
        request.extend_from_slice(
            b"Content-Length: 40\r\n\r\n{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"ping\"}",
        );
        // The server stops after the header, so this write may hit a closed
        // pipe; that is part of the behavior under test.
        let _ = stdin.write_all(&request);
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
    // One answer only: the ping is not served after the refusal.
    let headers = out
        .stdout
        .windows(b"Content-Length: ".len())
        .filter(|w| *w == b"Content-Length: ")
        .count();
    assert_eq!(headers, 1, "the server kept serving after the refusal");
    assert_eq!(
        out.status.code(),
        Some(2),
        "the framing error must stop the server: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}
