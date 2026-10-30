//! An owned, parser-independent model of a Solidity source file.
//!
//! The parser's AST borrows from an arena that lives only as long as a parse
//! session. Rules need to look at several files at once (a test suite and the
//! base contracts it inherits), so each file is lowered into this model once
//! and the arena is dropped. The model keeps only what the rules read: names,
//! call structure, assignments, literals, source text and positions.

/// A position in a source file. Lines are 1-based, columns are 1-based.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Pos {
    pub line: u32,
    pub col: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    /// The expression exactly as written in the source.
    pub text: String,
    pub pos: Pos,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    Ident(String),
    Member(Box<Expr>, String),
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
        /// `{value: 1, gas: 2}` call options, by name.
        options: Vec<(String, Expr)>,
    },
    /// `new T`. Calling it (`new T(args)`) is a `Call` whose callee is `New`.
    New(String),
    Lit(Lit),
    Index(Box<Expr>, Vec<Expr>),
    Binary(Box<Expr>, String, Box<Expr>),
    Unary(String, Box<Expr>),
    Assign(Box<Expr>, Box<Expr>),
    Ternary(Box<Expr>, Box<Expr>, Box<Expr>),
    Tuple(Vec<Expr>),
    Array(Vec<Expr>),
    /// An elementary or user-defined type used as an expression, e.g. the
    /// callee of the cast `address(x)` or the argument of `type(uint256)`.
    Type(String),
    /// Anything the rules have no use for. Sub-expressions are kept so that
    /// calls nested inside it are still visited.
    Other(Vec<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Lit {
    Number,
    Bool(bool),
    Str(String),
    Address,
    Other,
}

impl Expr {
    /// Direct sub-expressions, in evaluation order.
    pub fn children(&self) -> Vec<&Expr> {
        match &self.kind {
            ExprKind::Ident(_) | ExprKind::New(_) | ExprKind::Lit(_) | ExprKind::Type(_) => vec![],
            ExprKind::Member(base, _) => vec![base],
            ExprKind::Call {
                callee,
                args,
                options,
            } => {
                let mut out: Vec<&Expr> = vec![callee];
                out.extend(options.iter().map(|(_, e)| e));
                out.extend(args.iter());
                out
            }
            ExprKind::Index(base, idx) => {
                let mut out: Vec<&Expr> = vec![base];
                out.extend(idx.iter());
                out
            }
            ExprKind::Binary(l, _, r) | ExprKind::Assign(l, r) => vec![l, r],
            ExprKind::Unary(_, e) => vec![e],
            ExprKind::Ternary(c, a, b) => vec![c, a, b],
            ExprKind::Tuple(items) | ExprKind::Array(items) | ExprKind::Other(items) => {
                items.iter().collect()
            }
        }
    }

    /// The identifier, if this expression is a bare identifier.
    pub fn as_ident(&self) -> Option<&str> {
        match &self.kind {
            ExprKind::Ident(name) => Some(name),
            _ => None,
        }
    }

    pub fn is_lit(&self) -> bool {
        matches!(self.kind, ExprKind::Lit(_))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stmt {
    pub kind: StmtKind,
    pub pos: Pos,
}

/// Statements are flattened: the bodies of `if`, `for`, `while`, `try` and
/// nested blocks appear inline, in source order. The rules reason about what a
/// test does, not about which branch runs, so the flattening is deliberate.
#[derive(Debug, Clone, PartialEq)]
pub enum StmtKind {
    Expr(Expr),
    /// A local declaration. `vars` holds `(name, type)`; a tuple declaration
    /// has several entries and one initializer.
    Decl {
        vars: Vec<(String, String)>,
        init: Option<Expr>,
    },
    Emit {
        event: String,
        args: Vec<Expr>,
    },
    Return(Option<Expr>),
    Revert,
    Assembly,
    /// `_;` in a modifier: where the modified function's body runs.
    Placeholder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FnKind {
    Constructor,
    Function,
    Modifier,
    Fallback,
    Receive,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub name: String,
    pub kind: FnKind,
    pub visibility: Option<String>,
    pub params: Vec<(String, String)>,
    /// Modifier invocations, in written order (for a constructor this also
    /// holds base-constructor calls).
    pub modifiers: Vec<String>,
    pub body: Option<Vec<Stmt>>,
    pub pos: Pos,
}

impl Function {
    pub fn is_externally_callable(&self) -> bool {
        matches!(
            self.visibility.as_deref(),
            Some("public") | Some("external")
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct StateVar {
    pub name: String,
    pub ty: String,
    pub constant: bool,
    pub init: Option<Expr>,
    pub pos: Pos,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractKind {
    Contract,
    Abstract,
    Interface,
    Library,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Contract {
    pub name: String,
    pub kind: ContractKind,
    pub bases: Vec<String>,
    pub vars: Vec<StateVar>,
    pub functions: Vec<Function>,
    pub pos: Pos,
}

impl Contract {
    pub fn function(&self, name: &str) -> Option<&Function> {
        self.functions
            .iter()
            .find(|f| f.name == name && f.body.is_some())
    }
}

/// One parsed file.
#[derive(Debug, Clone)]
pub struct Unit {
    /// The path as it is displayed: relative to the working directory where
    /// possible, always with forward slashes.
    pub path: String,
    pub source: String,
    pub contracts: Vec<Contract>,
    /// Parser diagnostics. A file with errors may still yield contracts: the
    /// parser recovers, and whatever it recovered is checked.
    pub errors: Vec<String>,
}
