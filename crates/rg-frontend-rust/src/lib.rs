#![forbid(unsafe_code)]
//! The Rust frontend for Solana tests: lowers `syn`'s AST into
//! [`rg_ir::solana::ir`] and the shared expressions in [`rg_ir::ir`].
//!
//! `syn` is the parser the Rust ecosystem's procedural macros are built on. It
//! reads a whole file into a full syntax tree, and with `proc-macro2`'s span
//! locations every node carries its line, column and byte range, so a finding
//! points at the exact call and quotes it as written.

pub mod parse_rust;

pub use rg_ir::ir;
pub use rg_ir::solana::ir as solana_ir;
