#![forbid(unsafe_code)]
//! realism-gate: a static checker for proof-of-concept tests, written for
//! Foundry (Solidity) or for Solana programs (Rust, or Anchor's TypeScript).
//!
//! It answers one question about a PoC: does it prove the claim from the
//! position of an unprivileged attacker? It does not run the test. It parses
//! the source, rebuilds the order in which the test's code runs (state
//! initializers, `setUp()`, the test, inlined helpers, callbacks), works out
//! who sends or signs each call, and reports the patterns that make a passing
//! PoC prove less than it appears to: privileged pranks and signers, state
//! written with cheatcodes or `set_account`, assertions that cannot fail, and
//! measurements without a baseline. [`solana`] holds the Solana frontends,
//! Rust and TypeScript, and the rules as they apply to them.
//!
//! The analysis lives in workspace crates: `rg-ir` (the parsed model), the
//! `rg-frontend-*` parsers, `rg-model` (project, trace, identities, claims and
//! the report), `rg-rules`, `rg-exec` and `rg-render`. This crate is the
//! binary: the CLI, the engine that runs the rules, the declaration and
//! configuration readers, the claim binder and the MCP server. The moved
//! modules are re-exported here, so `realism_gate::project` and the rest keep
//! the paths the published library exposed.
//!
//! The library is what the `realism-gate` and `rgate` binaries run. Its API is
//! not yet stable; the JSON and SARIF outputs are the supported interface.

pub mod bind;
pub mod changed;
pub mod cite;
pub mod cli;
pub mod config;
pub mod declaration;
pub mod engine;
mod git;
pub mod mcp;
pub mod suppress;

pub use rg_exec as verify_fix;
pub use rg_frontend_sol::parse;
pub use rg_ir::finding::{Finding, Severity};
pub use rg_ir::{finding, ir};
pub use rg_model::project::Project;
pub use rg_model::{claims, identity, project, trace};
pub use rg_render as render;
pub use rg_rules::move_lang;
pub use rg_rules::rules;
pub use rg_rules::solana;

pub use engine::{Options, Report, analyze};
