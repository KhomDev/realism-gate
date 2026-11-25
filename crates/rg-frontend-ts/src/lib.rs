#![forbid(unsafe_code)]
//! The Anchor TypeScript frontend for Solana tests: lowers `oxc`'s AST into
//! [`rg_ir::solana::ir`] and the shared expressions in [`rg_ir::ir`].
//!
//! oxc is the parser of the Oxc JavaScript toolchain. It reads TypeScript in
//! full, recovers from errors, and gives every node a byte range, so a finding
//! points at the exact call and quotes it as written. Only files that import a
//! Solana package are read; other JavaScript in the tree is skipped.

pub mod parse_ts;

pub use rg_ir::ir;
pub use rg_ir::solana::ir as solana_ir;
