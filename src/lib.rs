//! realism-gate: a static checker for Foundry proof-of-concept tests.
//!
//! It answers one question about a PoC: does it prove the claim from the
//! position of an unprivileged attacker? It does not run the test. It parses
//! the Solidity, rebuilds the order in which the test's code runs (state
//! initializers, `setUp()`, the test, inlined helpers, callbacks), works out
//! who sends each call, and reports the patterns that make a passing PoC prove
//! less than it appears to: privileged pranks, state written with cheatcodes,
//! assertions that cannot fail, and measurements without a baseline.
//!
//! The library is what the `realism-gate` and `rgate` binaries run. Its API is
//! not yet stable; the JSON and SARIF outputs are the supported interface.

pub mod changed;
pub mod cli;
pub mod config;
pub mod declaration;
pub mod engine;
pub mod finding;
pub mod identity;
pub mod ir;
pub mod parse;
pub mod project;
pub mod render;
pub mod rules;
pub mod suppress;
pub mod trace;

pub use engine::{Options, Report, analyze};
pub use finding::{Finding, Severity};
pub use project::Project;
