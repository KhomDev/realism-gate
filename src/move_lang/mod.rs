//! Sui and Aptos Move tests.
//!
//! There is no Move compiler in this crate. A test module is read far enough
//! to find `#[test]` functions, their signer bindings, and the calls in the
//! body. That is enough for the subset the rules check: a privileged signer
//! or an admin cap, a test-only mint paid to the attacker, a missing
//! assertion, a missing baseline, and an `expected_failure` with no impact.
//!
//! Spec blocks, types and control flow are not reconstructed. A call written
//! inside a branch counts, the same way a Solidity `if` does.

mod parse;
mod rules;

use crate::ir::Pos;
pub use parse::parse_source;
pub use rules::check_test;

/// One `#[test]` function.
#[derive(Debug, Clone)]
pub struct TestFn {
    pub name: String,
    pub pos: Pos,
    /// `#[expected_failure]`: the test passes when it aborts.
    pub expected_failure: bool,
    /// Bindings from `#[test(admin = @0x1, user = @0x2)]`.
    pub signers: Vec<(String, String)>,
    /// Function parameters, names only.
    pub params: Vec<String>,
    pub body: String,
    pub calls: Vec<Call>,
}

/// A call in a test body. `path` is `coin::mint_for_testing`, `name` is the
/// last segment.
#[derive(Debug, Clone)]
pub struct Call {
    pub name: String,
    pub path: String,
    pub type_args: String,
    pub args: String,
    pub pos: Pos,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct File {
    pub module: String,
    pub tests: Vec<TestFn>,
}
