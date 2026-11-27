//! Property tests for the MCP server's request handling (`src/mcp.rs`): any
//! JSON gets a well-formed answer or none, a client string never reaches the
//! checker's command line as a flag, and arbitrary bytes on stdin never
//! panic the reader or the loop.

use proptest::prelude::*;
use realism_gate::mcp::{self, Framing, INVALID_PARAMS, Incoming, Policy, Ran};
use serde_json::{Value, json};

fn arb_json() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        any::<i64>().prop_map(|n| json!(n)),
        any::<f64>()
            .prop_filter("finite", |f| f.is_finite())
            .prop_map(|f| json!(f)),
        ".{0,12}".prop_map(Value::String),
        prop_oneof![
            Just("initialize"),
            Just("tools/list"),
            Just("tools/call"),
            Just("ping"),
            Just("2.0"),
            Just("check_poc"),
            Just("check_claims"),
            Just("dump_program"),
            Just("list_rules"),
            Just("explain_rule"),
        ]
        .prop_map(|s| Value::String(s.into())),
    ];
    leaf.prop_recursive(4, 48, 6, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..5).prop_map(Value::Array),
            prop::collection::btree_map(
                prop_oneof![
                    Just("jsonrpc".to_string()),
                    Just("id".to_string()),
                    Just("method".to_string()),
                    Just("params".to_string()),
                    Just("name".to_string()),
                    Just("arguments".to_string()),
                    Just("path".to_string()),
                    Just("report".to_string()),
                    Just("dir".to_string()),
                    Just("rule".to_string()),
                    Just("fail_on".to_string()),
                    "[a-z_]{1,8}",
                ],
                inner,
                0..6
            )
            .prop_map(|m| Value::Object(m.into_iter().collect())),
        ]
    })
}

/// A request shaped like a real one, with arbitrary arguments.
fn arb_call() -> impl Strategy<Value = Value> {
    (
        prop_oneof![
            Just("check_poc".to_string()),
            Just("check_claims".to_string()),
            Just("dump_program".to_string()),
            Just("list_rules".to_string()),
            Just("explain_rule".to_string()),
            ".{0,8}",
        ],
        arb_json(),
        any::<u32>(),
    )
        .prop_map(|(name, arguments, id)| {
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "tools/call",
                "params": { "name": name, "arguments": arguments }
            })
        })
}

fn record(calls: &mut Vec<Vec<String>>) -> impl FnMut(&[String]) -> Result<Ran, String> + '_ {
    move |args: &[String]| {
        calls.push(args.to_vec());
        Ok(Ran {
            code: 0,
            stdout: "{}".into(),
            stderr: String::new(),
        })
    }
}

/// The only argv elements a client may influence are path values (after
/// `--` or after a flag that takes a path), the fail_on level and a rule
/// id. Everything else is one of these fixed tokens.
const FIXED: &[&str] = &[
    "check",
    "rules",
    "explain",
    "--format",
    "json",
    "--fail-on",
    "--no-config",
    "--no-declaration",
    "--declaration",
    "--report",
    "--claims",
    "--dump-program",
    "--fragment",
    "--",
];
const LEVELS: &[&str] = &["error", "warn", "note", "never"];
const TAKES_VALUE: &[&str] = &[
    "--declaration",
    "--report",
    "--claims",
    "--dump-program",
    "--fail-on",
    "--format",
];

fn assert_argv_is_safe(argv: &[String]) -> Result<(), TestCaseError> {
    let dash = argv.iter().position(|a| a == "--");
    let mut i = 0;
    while i < argv.len() {
        let a = &argv[i];
        if Some(i) == dash {
            // At most one value follows `--`, and it is the last element.
            prop_assert!(argv.len() - i <= 2, "{argv:?}");
            break;
        }
        if TAKES_VALUE.contains(&a.as_str()) {
            prop_assert!(i + 1 < argv.len(), "{argv:?}");
            let v = &argv[i + 1];
            prop_assert!(!v.starts_with('-'), "a flag value is a flag: {argv:?}");
            if a == "--fail-on" {
                prop_assert!(LEVELS.contains(&v.as_str()), "{argv:?}");
            }
            if a == "--format" {
                prop_assert_eq!(v.as_str(), "json");
            }
            i += 2;
            continue;
        }
        prop_assert!(
            FIXED.contains(&a.as_str()),
            "a client token outside a value slot: {argv:?}"
        );
        i += 1;
    }
    for v in argv {
        prop_assert!(
            !v.contains('\0') && !v.contains('\n') && !v.contains('\r'),
            "{argv:?}"
        );
        prop_assert!(v.len() <= mcp::MAX_ARG_BYTES, "{argv:?}");
    }
    Ok(())
}

fn has_float(v: &Value) -> bool {
    match v {
        Value::Number(n) => n.is_f64(),
        Value::Array(a) => a.iter().any(has_float),
        Value::Object(o) => o.values().any(has_float),
        _ => false,
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 512, ..ProptestConfig::default() })]

    #[test]
    fn any_json_gets_a_well_formed_answer_or_none(msg in arb_json()) {
        let mut calls = Vec::new();
        let mut run = record(&mut calls);
        if let Some(out) = mcp::dispatch(&msg, &mut run) {
            prop_assert_eq!(&out["jsonrpc"], "2.0");
            let has_result = out.get("result").is_some();
            let has_error = out.get("error").is_some();
            prop_assert!(has_result != has_error, "{out}");
            if has_error {
                prop_assert!(out["error"]["code"].is_i64(), "{out}");
                prop_assert!(out["error"]["message"].is_string(), "{out}");
            }
            if !out["id"].is_null() {
                prop_assert_eq!(&out["id"], &msg["id"]);
            }
        } else {
            prop_assert!(msg.get("id").is_none(), "only a notification goes unanswered: {msg}");
        }
    }

    #[test]
    fn a_tool_call_never_puts_a_client_string_in_a_flag_slot(msg in arb_call()) {
        let mut calls = Vec::new();
        let out = {
            let mut run = record(&mut calls);
            mcp::dispatch(&msg, &mut run).unwrap()
        };
        if out.get("error").is_some() {
            prop_assert!(calls.is_empty(), "a refused call ran: {msg}");
        }
        for argv in &calls {
            assert_argv_is_safe(argv)?;
        }
    }

    #[test]
    fn check_poc_runs_exactly_when_the_path_is_a_path(path in "\\PC{0,40}") {
        let mut calls = Vec::new();
        let out = {
            let mut run = record(&mut calls);
            mcp::dispatch(
                &json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"check_poc","arguments":{"path":path}}}),
                &mut run,
            )
            .unwrap()
        };
        let t = path.trim();
        let valid = !t.is_empty() && !t.starts_with('-') && !t.contains(['\0', '\n', '\r']);
        prop_assert_eq!(calls.len(), usize::from(valid), "{}", out);
        if valid {
            let argv = &calls[0];
            prop_assert_eq!(
                &argv[..argv.len() - 1],
                &["check", "--format", "json", "--fail-on", "warn", "--no-config", "--no-declaration", "--"].map(String::from)[..]
            );
            prop_assert_eq!(argv.last().unwrap(), t);
        } else {
            prop_assert_eq!(&out["error"]["code"], INVALID_PARAMS);
        }
    }

    #[test]
    fn explain_rule_runs_only_an_id_or_slug(rule in "\\PC{0,20}|[A-Za-z0-9_-]{0,70}") {
        let mut calls = Vec::new();
        {
            let mut run = record(&mut calls);
            mcp::dispatch(
                &json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"explain_rule","arguments":{"rule":rule}}}),
                &mut run,
            )
            .unwrap();
        }
        let t = rule.trim();
        let valid = !t.is_empty()
            && t.len() <= 64
            && t.as_bytes()[0].is_ascii_alphanumeric()
            && t.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
        prop_assert_eq!(calls.len(), usize::from(valid));
        if valid {
            prop_assert_eq!(&calls[0], &vec!["explain".to_string(), "--".into(), t.to_string()]);
        }
    }

    #[test]
    fn an_argument_outside_the_schema_is_refused(key in "[a-z_]{1,10}") {
        let known = ["path", "declaration", "fail_on", "fragment"];
        prop_assume!(!known.contains(&key.as_str()));
        let mut calls = Vec::new();
        let out = {
            let mut run = record(&mut calls);
            mcp::dispatch(
                &json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"check_poc","arguments":{"path":"t", key: "x"}}}),
                &mut run,
            )
            .unwrap()
        };
        prop_assert_eq!(&out["error"]["code"], INVALID_PARAMS);
        prop_assert!(calls.is_empty());
    }

    #[test]
    fn a_line_round_trips_any_message(msg in arb_json()) {
        // serde_json without `float_roundtrip` may read a float back one ulp
        // off; the server never round-trips a float (ids are integers, tool
        // text is a string), so floats are out of this property.
        prop_assume!(!has_float(&msg));
        // serde_json writes every newline inside a string as an escape, so
        // the written line is one message, and reading it gives it back.
        let mut buf = Vec::new();
        mcp::write_line(&mut buf, &msg).unwrap();
        prop_assert_eq!(buf.iter().filter(|b| **b == b'\n').count(), 1);
        let got = mcp::read_message(&mut std::io::Cursor::new(buf)).unwrap().unwrap();
        match got {
            (Incoming::Message(v), Framing::Line) => {
                prop_assert_eq!(v, msg);
            }
            other => prop_assert!(false, "{other:?}"),
        }
    }

    #[test]
    fn arbitrary_bytes_never_panic_the_reader(bytes in prop::collection::vec(any::<u8>(), 0..4096)) {
        let mut input = std::io::Cursor::new(bytes);
        // Read until the stream ends or a framing error stops it.
        for _ in 0..10_000 {
            match mcp::read_message(&mut input) {
                Ok(Some(_)) => continue,
                Ok(None) | Err(_) => break,
            }
        }
    }

    #[test]
    fn arbitrary_bytes_never_panic_the_server_loop_and_every_line_answer_is_json(
        bytes in prop::collection::vec(prop_oneof![any::<u8>(), Just(b'\n'), Just(b'{'), Just(b'}')], 0..2048)
    ) {
        let mut out = Vec::new();
        let mut calls = Vec::new();
        {
            let mut run = record(&mut calls);
            let _ = mcp::serve(&mut std::io::Cursor::new(bytes.clone()), &mut out, &Policy::default(), &mut run);
        }
        // With no Content-Length header in the input, every answer is a line of JSON.
        let lower = String::from_utf8_lossy(&bytes).to_ascii_lowercase();
        if !lower.contains("content-length") {
            for line in String::from_utf8(out).unwrap().lines() {
                let v: Value = serde_json::from_str(line).unwrap();
                prop_assert_eq!(&v["jsonrpc"], "2.0");
            }
        }
    }

    #[test]
    fn a_framed_message_of_any_length_under_the_cap_round_trips(s in "\\PC{0,200}", id in any::<u32>()) {
        let msg = json!({"jsonrpc":"2.0","id":id,"method":"ping","params":{"s":s}});
        let mut buf = Vec::new();
        mcp::write_frame(&mut buf, &msg).unwrap();
        let got = mcp::read_message(&mut std::io::Cursor::new(buf)).unwrap().unwrap();
        prop_assert_eq!(got, (Incoming::Message(msg), Framing::ContentLength));
    }
}
