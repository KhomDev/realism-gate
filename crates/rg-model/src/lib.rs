#![forbid(unsafe_code)]
//! The assembled model of a run: the loaded project, the execution trace, the
//! identity model, the claim table and the report types.
//!
//! This crate is what the rules and the renderers share. It reads source (via
//! the language frontends) but never runs it:
//!
//! - [`project`] discovers and loads the files under review and resolves
//!   Solidity inheritance.
//! - [`trace`] rebuilds the order a Foundry test runs in, with helpers
//!   inlined.
//! - [`identity`] decides who each key, address and contract is.
//! - [`solana`] holds the Solana-side environment, harness reader and trace,
//!   and re-exports the Solana frontends and IR.
//! - [`claims`] reads a finding's claims and binds them to evidence.
//! - [`report`] is the JSON report the engine fills in.
//!
//! The module paths are re-exported so that [`crate::ir`] and
//! [`crate::move_lang`] resolve the way they did when this was one crate.

pub mod claims;
pub mod exploit;
pub mod identity;
pub mod project;
pub mod report;
pub mod solana;
pub mod trace;

pub use rg_ir::{finding, ir, snippet};

/// The Move frontend's entry point, under the path the project loader used
/// before the workspace split.
pub use rg_frontend_move as move_lang;

/// The Solidity frontend, under the path the project loader used before the
/// workspace split.
pub use rg_frontend_sol::parse;

pub use report::{ChangedSince, ParseError, Report, Summary};
