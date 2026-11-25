//! Enough of a Move reader to find tests and the calls inside them.
//!
//! Comments and strings are skipped. Braces and parentheses are matched.
//! Anything else is left as text, which is all the rules look at.

use super::{Call, File, TestFn};
use crate::ir::Pos;

pub fn parse_source(source: &str) -> File {
    let mut scan = Scan::new(source);
    let mut module = String::new();
    let mut tests = Vec::new();
    let mut attrs: Vec<String> = Vec::new();
    let mut depth = 0i32;
    while let Some(c) = scan.peek() {
        if c == '/' && scan.starts_with("//") {
            scan.skip_line();
            continue;
        }
        if c == '/' && scan.starts_with("/*") {
            scan.skip_block_comment();
            continue;
        }
        if c == '"' {
            scan.skip_string();
            continue;
        }
        if depth == 0 && c == '#' {
            attrs.push(scan.read_attribute());
            continue;
        }
        if depth == 0 && scan.starts_with("module") && scan.at_ident_boundary("module") {
            scan.bump_str("module");
            scan.skip_ws();
            module = scan.read_path();
            scan.skip_ws();
            // The module's own braces are not nesting. Tests inside them are
            // still at depth 0, which is also where a bare `#[test] fun` sits.
            if scan.peek() == Some('{') {
                scan.bump();
            }
            continue;
        }
        if depth == 0 && scan.starts_with("fun") && scan.at_ident_boundary("fun") {
            if let Some(fun) = scan.read_fun(&attrs) {
                tests.push(fun);
            }
            attrs.clear();
            continue;
        }
        match c {
            '{' => {
                depth += 1;
                scan.bump();
            }
            '}' => {
                depth = depth.saturating_sub(1);
                scan.bump();
                if depth == 0 {
                    attrs.clear();
                }
            }
            _ => {
                // An identifier that is not `fun` ends a run of attributes
                // that belonged to something else (a struct, a const).
                if depth == 0 && (c.is_ascii_alphabetic() || c == '_') {
                    attrs.clear();
                    scan.skip_ident();
                } else {
                    scan.bump();
                }
            }
        }
    }
    File { module, tests }
}

struct Scan<'a> {
    src: &'a str,
    i: usize,
    line: u32,
    col: u32,
}

impl<'a> Scan<'a> {
    fn new(src: &'a str) -> Self {
        Self {
            src,
            i: 0,
            line: 1,
            col: 1,
        }
    }

    fn peek(&self) -> Option<char> {
        self.src[self.i..].chars().next()
    }

    fn starts_with(&self, s: &str) -> bool {
        self.src[self.i..].starts_with(s)
    }

    fn at_ident_boundary(&self, word: &str) -> bool {
        let rest = &self.src[self.i + word.len()..];
        !rest
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.i += c.len_utf8();
        if c == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Some(c)
    }

    fn bump_str(&mut self, s: &str) {
        for _ in s.chars() {
            self.bump();
        }
    }

    fn pos(&self) -> Pos {
        Pos {
            line: self.line,
            col: self.col,
        }
    }

    fn skip_ws(&mut self) {
        while let Some(c) = self.peek() {
            if c == '/' && self.starts_with("//") {
                self.skip_line();
            } else if c == '/' && self.starts_with("/*") {
                self.skip_block_comment();
            } else if c.is_whitespace() {
                self.bump();
            } else {
                break;
            }
        }
    }

    fn skip_line(&mut self) {
        while let Some(c) = self.bump() {
            if c == '\n' {
                break;
            }
        }
    }

    fn skip_block_comment(&mut self) {
        self.bump_str("/*");
        while self.peek().is_some() && !self.starts_with("*/") {
            self.bump();
        }
        if self.starts_with("*/") {
            self.bump_str("*/");
        }
    }

    fn skip_string(&mut self) {
        self.bump();
        while let Some(c) = self.bump() {
            if c == '\\' {
                self.bump();
            } else if c == '"' {
                break;
            }
        }
    }

    fn skip_ident(&mut self) {
        while self
            .peek()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            self.bump();
        }
    }

    fn read_ident(&mut self) -> String {
        let start = self.i;
        self.skip_ident();
        self.src[start..self.i].to_string()
    }

    fn read_path(&mut self) -> String {
        let mut out = self.read_ident();
        while self.starts_with("::") {
            self.bump_str("::");
            out.push_str("::");
            out.push_str(&self.read_ident());
        }
        out
    }

    /// `#[ ... ]`, with nested brackets. Returned without the leading `#`.
    fn read_attribute(&mut self) -> String {
        let start = self.i;
        self.bump(); // #
        if self.peek() == Some('[') {
            self.read_balanced('[', ']');
        }
        self.src[start..self.i].to_string()
    }

    fn read_balanced(&mut self, open: char, close: char) -> String {
        let start = self.i;
        if self.peek() == Some(open) {
            self.bump();
        }
        let mut depth = 1i32;
        while let Some(c) = self.peek() {
            if c == '"' {
                self.skip_string();
                continue;
            }
            if c == '/' && self.starts_with("//") {
                self.skip_line();
                continue;
            }
            if c == '/' && self.starts_with("/*") {
                self.skip_block_comment();
                continue;
            }
            self.bump();
            if c == open {
                depth += 1;
            } else if c == close {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
        }
        self.src[start..self.i].to_string()
    }

    /// A test function, or `None` for an ordinary one. The body is consumed
    /// either way, so the outer scan does not see it twice.
    fn read_fun(&mut self, attrs: &[String]) -> Option<TestFn> {
        let is_test = attrs.iter().any(|a| {
            let t = a.trim_start_matches('#').trim();
            t.starts_with("[test") || t.starts_with("[expected_failure")
        });
        let expected_failure = attrs.iter().any(|a| a.contains("expected_failure"));
        let signers = attrs
            .iter()
            .filter(|a| a.contains("[test"))
            .flat_map(|a| signer_bindings(a))
            .collect();
        self.bump_str("fun");
        self.skip_ws();
        let pos = self.pos();
        let name = self.read_ident();
        self.skip_ws();
        if self.peek() == Some('<') {
            self.read_balanced('<', '>');
            self.skip_ws();
        }
        let params = if self.peek() == Some('(') {
            let raw = self.read_balanced('(', ')');
            param_names(&raw)
        } else {
            Vec::new()
        };
        self.skip_ws();
        if self.peek() == Some(':') {
            // Return type, up to the body.
            while let Some(c) = self.peek() {
                if c == '{' || c == ';' {
                    break;
                }
                self.bump();
            }
        }
        self.skip_ws();
        let body = if self.peek() == Some('{') {
            let raw = self.read_balanced('{', '}');
            raw.trim_start_matches('{')
                .trim_end_matches('}')
                .to_string()
        } else {
            String::new()
        };
        if !is_test {
            return None;
        }
        let calls = calls_in(&body, pos.line);
        Some(TestFn {
            name,
            pos,
            expected_failure,
            signers,
            params,
            body,
            calls,
        })
    }
}

fn signer_bindings(attr: &str) -> Vec<(String, String)> {
    // #[test(admin = @0x1, attacker = @0xA)]
    let Some(inner) = attr
        .split_once('(')
        .map(|(_, r)| r.trim_end_matches(']').trim_end_matches(')'))
    else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for part in split_top(inner, ',') {
        let Some((name, addr)) = part.split_once('=') else {
            continue;
        };
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        out.push((name.to_string(), addr.trim().to_string()));
    }
    out
}

fn param_names(params: &str) -> Vec<String> {
    let inner = params.trim().trim_start_matches('(').trim_end_matches(')');
    split_top(inner, ',')
        .into_iter()
        .filter_map(|p| {
            let p = p.trim().trim_start_matches("mut ").trim();
            let name = p
                .split(':')
                .next()?
                .trim()
                .trim_start_matches("mut ")
                .trim();
            if name.is_empty() {
                None
            } else {
                Some(name.to_string())
            }
        })
        .collect()
}

fn split_top(text: &str, sep: char) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut depth = 0i32;
    for (i, c) in text.char_indices() {
        match c {
            '(' | '<' | '[' | '{' => depth += 1,
            ')' | '>' | ']' | '}' => depth = depth.saturating_sub(1),
            _ if c == sep && depth == 0 => {
                out.push(&text[start..i]);
                start = i + c.len_utf8();
            }
            _ => {}
        }
    }
    if start <= text.len() {
        let tail = text[start..].trim();
        if !tail.is_empty() {
            out.push(tail);
        }
    }
    out
}

/// Calls in a function body. Line numbers are relative to the `fun` line,
/// which is close enough: the body scanner counts newlines from the start of
/// the body, and the caller passes the line of `fun`. The body's first line
/// is usually the line after `fun ... {`, so counts start at `fun_line`.
fn calls_in(body: &str, fun_line: u32) -> Vec<Call> {
    let mut scan = Scan::new(body);
    // The `{` was on the `fun` line or the next one. Count newlines so a call
    // on the first body line lands on fun_line + newlines seen.
    let mut calls = Vec::new();
    while scan.peek().is_some() {
        if scan.starts_with("//") {
            scan.skip_line();
            continue;
        }
        if scan.starts_with("/*") {
            scan.skip_block_comment();
            continue;
        }
        if scan.peek() == Some('"') {
            scan.skip_string();
            continue;
        }
        if scan
            .peek()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        {
            let pos_in_body = scan.pos();
            let path = scan.read_path();
            scan.skip_ws();
            let type_args = if scan.peek() == Some('<') {
                scan.read_balanced('<', '>')
            } else {
                String::new()
            };
            scan.skip_ws();
            // `assert!(...)` is a macro call. The `!` is not part of the name.
            if scan.peek() == Some('!') {
                scan.bump();
                scan.skip_ws();
            }
            if scan.peek() == Some('(') && !is_keyword(&path) {
                let args = scan.read_balanced('(', ')');
                let name = path.rsplit("::").next().unwrap_or(&path).to_string();
                let text = format!("{path}{type_args}{args}");
                calls.push(Call {
                    name,
                    path,
                    type_args,
                    args,
                    pos: Pos {
                        line: fun_line + pos_in_body.line - 1,
                        col: pos_in_body.col,
                    },
                    text,
                });
            }
            continue;
        }
        scan.bump();
    }
    calls
}

fn is_keyword(path: &str) -> bool {
    matches!(
        path,
        "if" | "while"
            | "loop"
            | "return"
            | "abort"
            | "let"
            | "as"
            | "move"
            | "copy"
            | "else"
            | "match"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_test_and_its_calls_are_found() {
        let src = r#"
        module vault::tests {
            #[test]
            fun test_withdraws() {
                let attacker = @0xA11CE;
                test_scenario::next_tx(&mut scenario, attacker);
                vault::withdraw();
                assert!(coin::value(&c) > 0, 0);
            }

            fun helper() { let x = 1; }
        }
        "#;
        let file = parse_source(src);
        assert_eq!(file.module, "vault::tests");
        assert_eq!(file.tests.len(), 1);
        let t = &file.tests[0];
        assert_eq!(t.name, "test_withdraws");
        let names: Vec<_> = t.calls.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["next_tx", "withdraw", "assert"]);
        assert!(t.calls[2].args.contains("value"));
    }

    #[test]
    fn signer_bindings_and_expected_failure() {
        let src = r#"
            #[expected_failure]
            #[test(admin = @0x1, attacker = @0xA)]
            fun test_aborts(admin: &signer, attacker: &signer) {
                vault::init(admin);
            }
        "#;
        let file = parse_source(src);
        assert_eq!(file.tests.len(), 1);
        let t = &file.tests[0];
        assert!(t.expected_failure);
        assert_eq!(t.signers.len(), 2);
        assert_eq!(t.signers[0].0, "admin");
        assert!(t.signers[0].1.contains("@0x1"));
        assert_eq!(t.params, vec!["admin", "attacker"]);
    }
}
