#![forbid(unsafe_code)]
//! The Sui and Aptos Move frontend: enough of a reader to find `#[test]`
//! functions and the calls inside them.
//!
//! Comments and strings are skipped. Braces and parentheses are matched.
//! Anything else is left as text, which is all the rules look at. The owned
//! types it produces live in [`rg_ir::move_lang`].

pub mod parse;

pub use parse::parse_source;
pub use rg_ir::ir;
pub use rg_ir::move_lang::{Call, File, TestFn};
