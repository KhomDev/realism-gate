//! The owned model of one Solana test file, Rust or TypeScript.
//!
//! Expressions and statements reuse the Solidity model in [`crate::ir`], so
//! the same helpers (keys, compact text, positions) apply to every language.
//! Both frontends lower into the same shapes, spelled the way the Rust
//! harnesses spell them, so the trace, the identities and the rules do not
//! care which language a test is written in. The mapping from Rust is:
//!
//! - a path (`spl_token::ID`, `attacker`) is an [`ExprKind::Ident`] holding
//!   the path without generics;
//! - `a.f(x)` is a call whose callee is `Member(a, "f")`, and `f(x)` or
//!   `T::f(x)` is a call whose callee is the path;
//! - a macro (`assert_eq!(a, b)`) is a call whose callee is `Ident("assert_eq!")`
//!   and whose arguments are the macro's comma-separated expressions;
//! - a struct literal (`Account { lamports, data, .. }`) is a call whose callee
//!   is [`ExprKind::Type`] and whose named options are the fields;
//! - `&x`, `*x` and `!x` are unary; `x?`, `x.await`, `(x)` and `x as T` are
//!   the inner expression.
//!
//! TypeScript is lowered onto the same shapes; [`super::parse_ts`] lists how.
//!
//! Blocks are flattened the same way Solidity's are: the statements inside an
//! `if`, `match`, loop, closure or nested block run inline, in source order,
//! before the statement that contains them.
//!
//! [`ExprKind::Ident`]: crate::ir::ExprKind::Ident
//! [`ExprKind::Type`]: crate::ir::ExprKind::Type

use crate::ir::{Expr, Pos, Stmt};

/// The language a Solana test file is written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Dialect {
    /// `solana-program-test`, LiteSVM, Mollusk or anchor-client tests.
    #[default]
    Rust,
    /// Anchor's TypeScript tests: mocha or jest, with `@coral-xyz/anchor`,
    /// web3.js, bankrun or LiteSVM.
    TypeScript,
}

/// A function that runs before a test: a `describe` block's body, a
/// `before` hook, or an earlier test whose state the test inherits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Prior {
    /// Index into [`File::functions`].
    pub function: usize,
    /// Whether it builds the world rather than attacks it.
    pub setup: bool,
}

/// A function, free or in an `impl` block or class.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Function {
    pub name: String,
    /// The `impl` block's type or the class, for a method or an associated
    /// function.
    pub owner: Option<String>,
    /// Whether the first parameter is `self` (an instance method).
    pub method: bool,
    /// Where the function sits in the file: the module path in Rust (`""` at
    /// the top level, else `tests` or `tests::helpers`), the `describe` titles
    /// in TypeScript (`vault > withdraw`).
    pub module: String,
    /// A test: marked with an attribute whose last segment is `test`
    /// (`#[test]`, `#[tokio::test]`), or an `it(..)` / `test(..)` callback.
    pub test: bool,
    /// A `describe` body or a mocha hook: it runs only as a test's [`Prior`],
    /// never as a helper a call resolves to.
    pub hook: bool,
    /// Marked `#[should_panic]`: the test passes only if it fails.
    pub should_panic: bool,
    /// Parameter names, excluding `self`, in order. A pattern parameter that is
    /// not a plain name is recorded as an empty string.
    pub params: Vec<String>,
    /// How many arguments a call must pass. Rust has no default arguments, so
    /// this is every parameter; in TypeScript a parameter with a default or a
    /// `?` may be left out.
    pub required: usize,
    /// Whether a rest parameter takes any number of further arguments.
    pub variadic: bool,
    /// What runs before a test, in order: for a TypeScript test, the file's
    /// top level, the enclosing `describe` bodies and `before` hooks, and the
    /// tests above it, which mocha runs first and whose state it shares.
    pub before: Vec<Prior>,
    /// How the function is introduced in the source, for quoting it:
    /// `fn test_drain()`, `it("drains the vault")`.
    pub header: String,
    pub body: Vec<Stmt>,
    pub pos: Pos,
}

impl Function {
    /// Whether a call passing `nargs` arguments can be a call to this
    /// function. Rust has neither overloading nor default arguments, so a
    /// function with a different number of parameters is not the one called.
    pub fn accepts(&self, nargs: usize) -> bool {
        nargs >= self.required && (self.variadic || nargs <= self.params.len())
    }
}

/// One parsed Solana test file.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct File {
    pub dialect: Dialect,
    pub functions: Vec<Function>,
    /// `const` and `static` items, anywhere in the file, with their value.
    pub consts: Vec<(String, Expr)>,
}
