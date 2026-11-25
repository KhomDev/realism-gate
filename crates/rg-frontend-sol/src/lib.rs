#![forbid(unsafe_code)]
//! The Solidity frontend: lowers `solar`'s AST into [`rg_ir::ir`].
//!
//! `solar` is the Solidity parser written for the Foundry toolchain. Using a
//! real parser instead of regular expressions is what lets the rules tell a
//! cheatcode call from a comment that mentions one, follow an argument into a
//! nested call, and point at an exact line and column.

pub mod parse;

pub use parse::parse_source;
pub use rg_ir::ir;
