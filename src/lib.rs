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
//! The library is what the `realism-gate` and `rgate` binaries run. Its API is
//! not yet stable; the JSON and SARIF outputs are the supported interface.

pub mod changed;
pub mod cite;
pub mod cli;
pub mod config;
pub mod declaration;
pub mod engine;
pub mod finding;
pub mod identity;
pub mod ir;
pub mod mcp;
pub mod move_lang;
pub mod parse;
pub mod project;
pub mod render;
pub mod rules;
pub mod solana;
pub mod suppress;
pub mod trace;
pub mod verify_fix;

pub use engine::{Options, Report, analyze};
pub use finding::{Finding, Severity};
pub use project::Project;
