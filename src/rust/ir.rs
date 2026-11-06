//! The owned model of one Rust source file.
//!
//! Expressions and statements reuse the Solidity model in [`crate::ir`], so
//! the same helpers (keys, compact text, positions) apply to both languages.
//! The mapping from Rust is:
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
//! Blocks are flattened the same way Solidity's are: the statements inside an
//! `if`, `match`, loop, closure or nested block run inline, in source order,
//! before the statement that contains them.
//!
//! [`ExprKind::Ident`]: crate::ir::ExprKind::Ident
//! [`ExprKind::Type`]: crate::ir::ExprKind::Type

use crate::ir::{Expr, Pos, Stmt};

/// A function, free or in an `impl` block.
#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub name: String,
    /// The `impl` block's type, for a method or an associated function.
    pub owner: Option<String>,
    /// Whether the first parameter is `self`.
    pub method: bool,
    /// The module path inside the file: `""` at the top level, else `tests` or
    /// `tests::helpers`.
    pub module: String,
    /// Marked with an attribute whose last segment is `test`: `#[test]`,
    /// `#[tokio::test]`, `#[test_log::test]`.
    pub test: bool,
    /// Marked `#[should_panic]`: the test passes only if it fails.
    pub should_panic: bool,
    /// Parameter names, excluding `self`, in order. A pattern parameter that is
    /// not a plain name is recorded as an empty string.
    pub params: Vec<String>,
    pub body: Vec<Stmt>,
    pub pos: Pos,
}

/// One parsed Rust file.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct File {
    pub functions: Vec<Function>,
    /// `const` and `static` items, anywhere in the file, with their value.
    pub consts: Vec<(String, Expr)>,
}
