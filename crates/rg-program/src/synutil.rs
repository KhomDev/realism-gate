//! Reading spans out of Rust source parsed with `syn`.
//!
//! `syn` borrows its tokens from `proc-macro2`, whose spans report a 1-based
//! line and a 0-based column in UTF-8 characters once the `span-locations`
//! feature is on. The workspace enables it. A span's text is cut back out of
//! the original source by those coordinates: the model reports the line and
//! column *and* the text at them, and does not depend on any pretty-printer.

use crate::Span;
use proc_macro2::Span as PmSpan;
use syn::spanned::Spanned;

/// The longest text a [`Span`] keeps. A whole instruction body would bloat the
/// serialised model; the coordinates point at the rest.
const MAX_SPAN_TEXT: usize = 160;

/// One source file with its line starts, so a `syn` span can be turned into a
/// line, a column and the exact text without rescanning the file per span.
pub(crate) struct SourceIndex<'a> {
    source: &'a str,
    line_starts: Vec<usize>,
}

impl<'a> SourceIndex<'a> {
    pub(crate) fn new(source: &'a str) -> Self {
        let mut line_starts = vec![0];
        for (offset, byte) in source.bytes().enumerate() {
            if byte == b'\n' {
                line_starts.push(offset + 1);
            }
        }
        Self {
            source,
            line_starts,
        }
    }

    /// The model's [`Span`] for a `syn`/`proc-macro2` span.
    pub(crate) fn span(&self, file: &str, span: PmSpan) -> Span {
        let start = span.start();
        let end = span.end();
        Span {
            file: file.to_string(),
            line: saturating_u32(start.line),
            column: saturating_u32(start.column) + 1,
            text: short(&self.slice(start.line, start.column, end.line, end.column)),
        }
    }

    /// The span of any `syn` node.
    pub(crate) fn spanned<T: Spanned>(&self, file: &str, node: &T) -> Span {
        self.span(file, node.span())
    }

    /// The exact source text of a `syn` span.
    pub(crate) fn text(&self, span: PmSpan) -> String {
        let start = span.start();
        let end = span.end();
        self.slice(start.line, start.column, end.line, end.column)
    }

    fn slice(
        &self,
        start_line: usize,
        start_col: usize,
        end_line: usize,
        end_col: usize,
    ) -> String {
        let Some(start) = self.byte_at(start_line, start_col) else {
            return String::new();
        };
        // `proc-macro2` reports the end position as the offset just past the
        // last character (verified against the spans a `syn::parse_file` of a
        // string assigns), so the end byte is used as-is.
        let end = self.byte_at(end_line, end_col).unwrap_or(start);
        if end < start || end > self.source.len() {
            return String::new();
        }
        self.source.get(start..end).unwrap_or_default().to_string()
    }

    /// The byte offset of the 0-based character `column` of the 1-based
    /// `line`.
    fn byte_at(&self, line: usize, column: usize) -> Option<usize> {
        let start = *self.line_starts.get(line.checked_sub(1)?)?;
        let line_text = &self.source[start..];
        let line_text = line_text.split('\n').next().unwrap_or(line_text);
        match line_text.char_indices().nth(column) {
            Some((offset, _)) => Some(start + offset),
            // A span can point one past the end of a line (for example at the
            // newline); clamp it to the line end.
            None => Some(start + line_text.len()),
        }
    }
}

/// Collapses runs of whitespace to one space and caps the length, so a span's
/// text stays on one line in the serialised model.
pub(crate) fn short(text: &str) -> String {
    let one_line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.chars().count() <= MAX_SPAN_TEXT {
        one_line
    } else {
        let cut: String = one_line.chars().take(MAX_SPAN_TEXT - 1).collect();
        format!("{cut}…")
    }
}

fn saturating_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

/// Whether `name` appears in `text` as a whole identifier: not as part of a
/// longer identifier. `user` matches `user.key()` and `&user`, not `user_data`.
pub(crate) fn mentions_ident(text: &str, name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let bytes = text.as_bytes();
    let mut from = 0;
    while let Some(found) = text[from..].find(name) {
        let start = from + found;
        let end = start + name.len();
        let before_ok = start == 0 || !is_ident_byte(bytes[start - 1]);
        let after_ok = end >= bytes.len() || !is_ident_byte(bytes[end]);
        if before_ok && after_ok {
            return true;
        }
        from = end;
    }
    false
}

fn is_ident_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spans_report_line_column_and_text() {
        let source = "pub fn a() {}\npub fn b() { let x = foo(1, 2); }\n";
        let index = SourceIndex::new(source);
        let file: syn::File = syn::parse_file(source).unwrap();
        let syn::Item::Fn(function) = &file.items[0] else {
            panic!("a function");
        };
        let span = index.spanned("lib.rs", function);
        assert_eq!(span.file, "lib.rs");
        assert_eq!(span.line, 1);
        assert_eq!(span.column, 1);
        assert_eq!(span.text, "pub fn a() {}");
        let second = index.spanned("lib.rs", &file.items[1]);
        assert_eq!(second.line, 2);
        assert_eq!(second.text, "pub fn b() { let x = foo(1, 2); }");
    }

    #[test]
    fn a_multibyte_line_keeps_char_columns() {
        let source = "// é comment\nconst X: u8 = 1;\n";
        let index = SourceIndex::new(source);
        let file: syn::File = syn::parse_file(source).unwrap();
        let span = index.spanned("lib.rs", &file.items[0]);
        assert_eq!(span.line, 2);
        assert_eq!(span.column, 1);
        assert_eq!(span.text, "const X: u8 = 1;");
    }

    #[test]
    fn identifier_mentions_respect_boundaries() {
        assert!(mentions_ident("user.key().as_ref()", "user"));
        assert!(mentions_ident("&user", "user"));
        assert!(!mentions_ident("user_data.key()", "user"));
        assert!(!mentions_ident("superuser", "user"));
    }

    #[test]
    fn text_is_collapsed_and_capped() {
        assert_eq!(short("a(\n  b,\n  c)"), "a( b, c)");
        let long = "x".repeat(500);
        assert_eq!(short(&long).chars().count(), MAX_SPAN_TEXT);
    }
}
