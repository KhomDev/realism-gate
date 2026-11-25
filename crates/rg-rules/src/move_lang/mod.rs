//! Sui and Aptos Move tests.
//!
//! There is no Move compiler in this workspace. A test module is read far
//! enough to find `#[test]` functions, their signer bindings, and the calls in
//! the body. That is enough for the subset the rules check: a privileged
//! signer or an admin cap, a test-only mint paid to the attacker, a missing
//! assertion, a missing baseline, and an `expected_failure` with no impact.
//!
//! Spec blocks, types and control flow are not reconstructed. A call written
//! inside a branch counts, the same way a Solidity `if` does.
//!
//! The reader lives in `rg-frontend-move` and the owned types in
//! `rg-ir::move_lang`; this module re-exports them under the paths the
//! pipeline used before the workspace split.

pub mod rules;

pub use rg_frontend_move::parse_source;
pub use rg_ir::move_lang::{Call, File, TestFn};
pub use rules::check_test;
