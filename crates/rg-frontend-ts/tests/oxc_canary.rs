//! A canary for the oxc bug that `parse_ts::neutralize_annotations` hides.
//!
//! `oxc_parser` 0.146.0 fails `debug_assert!(comment.is_pure())` at
//! `lexer/trivia_builder.rs:62` on a pure annotation before a label or a stray
//! colon. This test calls oxc directly, with no guard, and expects the panic.
//! When an oxc upgrade fixes the bug, this test fails: the guard can then go.
//! The assertion is live only in a debug build.

#![cfg(debug_assertions)]

use oxc_allocator::Allocator;
use oxc_parser::Parser;
use oxc_span::SourceType;

#[test]
#[should_panic(expected = "comment.is_pure()")]
fn oxc_0_146_still_panics_on_a_pure_comment_before_a_colon() {
    let allocator = Allocator::default();
    let _ = Parser::new(&allocator, "//#__PURE__\n:", SourceType::ts()).parse();
}

#[test]
fn the_guarded_text_does_not_panic() {
    let guarded = rg_frontend_ts::parse_ts::neutralize_annotations("//#__PURE__\n:");
    let allocator = Allocator::default();
    let _ = Parser::new(&allocator, &guarded, SourceType::ts()).parse();
}
