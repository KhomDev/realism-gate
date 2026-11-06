//! Lowers an Anchor TypeScript (or JavaScript) test file, parsed with oxc,
//! into the model in [`super::ir`].
//!
//! oxc is the parser of the Oxc JavaScript toolchain. It reads TypeScript in
//! full, recovers from errors, and gives every node a byte range, so a
//! finding points at the exact call and quotes it as written.
//!
//! Only files that import a Solana package are read (`@coral-xyz/anchor`,
//! anything under `@solana/`, bankrun, LiteSVM). Other JavaScript in the tree
//! (a Hardhat suite, a bundler config) is not a Solana test and is skipped.
//!
//! The lowering spells TypeScript the way [`super::parse_rust`] spells Rust,
//! so the trace, the identities and the rules read both alike:
//!
//! - `describe`, `it` and the mocha hooks are structure, not calls. Each
//!   `it(..)` / `test(..)` callback is a test. What runs before it is recorded
//!   in [`Function::before`]: the file's top level, the enclosing `describe`
//!   bodies and `before` / `beforeEach` hooks (all of which build the world),
//!   and every test above it, because mocha runs them first and the test
//!   inherits their state. An earlier test builds the world when its title
//!   says so (`it("initializes the vault")`).
//! - Every called name is in snake case: `svm.setAccount(..)` is
//!   `set_account`, `getAssociatedTokenAddressSync(..)` is
//!   `get_associated_token_address_sync`. Variables and fields keep their
//!   names.
//! - A call on an imported name or a class is a path call:
//!   `anchor.web3.Keypair.generate()` is `anchor::web3::Keypair::generate()`.
//!   `new T(..)` is `T::new(..)`, `this` is `self`.
//! - `kp.publicKey` is `kp.pubkey()`; `new PublicKey("..")` is
//!   `PublicKey::from_str("..")`.
//! - Conversions are their operand: `await x`, `x as T`, `x!`, `new BN(x)`,
//!   `BigInt(x)`, `Number(x)`, `x.toNumber()`, `x.toString()`.
//! - An object literal is a struct literal with no type name; `===` is `==`;
//!   BN and key comparisons (`a.gt(b)`, `a.equals(b)`) are binary operators.
//! - `console.log(..)` and the other `console` methods are `println!`.
//! - chai, node and jest assertions are `assert!`, `assert_eq!` and
//!   `assert_ne!` when they compare, and `assert_that!` otherwise.
//!   `if (c) throw ..` is `assert!(!c)`.
//! - `getAssociatedTokenAddress(mint, owner)` takes the owner first, as Rust's
//!   `get_associated_token_address(owner, mint)` does.
//! - The programs and accounts handed to bankrun's `startAnchor` and `start`
//!   are `add_program` and `set_account` calls, one per entry.

use super::ir::{Dialect, File, Function, Prior};
use crate::ir::{Expr, ExprKind, Lit, Pos, Stmt, StmtKind, Unit};
use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType, Span};
use std::collections::BTreeMap;

/// Text that a Solana test file must contain somewhere; a cheap filter before
/// parsing every JavaScript file in a tree.
const MARKERS: &[&str] = &["@solana/", "anchor", "bankrun", "litesvm"];

/// Whether an import source is a Solana package.
fn is_solana_package(source: &str) -> bool {
    source.starts_with("@solana/")
        || [
            "@coral-xyz/anchor",
            "@project-serum/anchor",
            "solana-bankrun",
            "anchor-bankrun",
            "litesvm",
            "anchor-litesvm",
        ]
        .iter()
        .any(|p| source == *p || source.strip_prefix(p).is_some_and(|r| r.starts_with('/')))
}

fn is_bankrun(source: &str) -> bool {
    matches!(source, "solana-bankrun" | "anchor-bankrun")
}

/// Whether a path names a JavaScript or TypeScript source this frontend reads.
pub fn is_script_path(path: &str) -> bool {
    const EXTENSIONS: &[&str] = &[".ts", ".mts", ".cts", ".js", ".mjs", ".cjs"];
    !path.ends_with(".d.ts") && EXTENSIONS.iter().any(|e| path.ends_with(e))
}

/// Parses one script. `None` when it is not a Solana test file: it imports no
/// Solana package. Never panics on malformed input: a file that does not
/// parse yields what oxc recovered and its errors in [`Unit::errors`].
pub fn parse_source(path: &str, source: &str) -> Option<Unit> {
    let text = source.strip_prefix('\u{feff}').unwrap_or(source);
    if !MARKERS.iter().any(|m| text.contains(m)) {
        return None;
    }
    let source_type = SourceType::from_path(path).unwrap_or_else(|_| SourceType::ts());
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, text, source_type).parse();
    let mut lower = Lower::new(text);
    let errors: Vec<String> = parsed
        .diagnostics
        .errors()
        .take(3)
        .map(|d| {
            let at = lower.pos_at(d.labels.first().map_or(0, |l| l.offset()));
            format!("error: {} (line {}, column {})", d.message, at.line, at.col)
        })
        .collect();
    let imports_solana = lower.imports(&parsed.program.body);
    if !imports_solana && errors.is_empty() {
        return None;
    }
    let file = lower.program(&parsed.program.body);
    Some(Unit {
        path: path.to_string(),
        source: source.to_string(),
        contracts: Vec::new(),
        solana: Some(file),
        errors,
    })
}

/// A called name in snake case: `setAccount` is `set_account`,
/// `getATA` is `get_ata`. Names without lower-case letters are unchanged.
pub fn snake(name: &str) -> String {
    if !name.chars().any(|c| c.is_ascii_uppercase())
        || !name.chars().any(|c| c.is_ascii_lowercase())
    {
        return name.to_string();
    }
    let chars: Vec<char> = name.chars().collect();
    let mut out = String::with_capacity(name.len() + 4);
    for (i, &c) in chars.iter().enumerate() {
        if c.is_ascii_uppercase() {
            let boundary = i > 0 && {
                let prev = chars[i - 1];
                let next_lower = chars.get(i + 1).is_some_and(|n| n.is_ascii_lowercase());
                prev.is_ascii_lowercase()
                    || prev.is_ascii_digit()
                    || (prev.is_ascii_uppercase() && next_lower)
            };
            if boundary && !out.ends_with('_') {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// What a test-framework call is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mocha {
    Describe,
    Test,
    /// `before`, `beforeAll`: once, before the block's first test.
    Before,
    /// `beforeEach`: before every test of the block.
    BeforeEach,
    /// `after*` hooks, and skipped or pending blocks and tests.
    Ignored,
}

fn mocha_kind(name: &str) -> Option<Mocha> {
    Some(match name {
        "describe" | "context" | "suite" => Mocha::Describe,
        "it" | "test" | "specify" => Mocha::Test,
        "before" | "beforeAll" => Mocha::Before,
        "beforeEach" => Mocha::BeforeEach,
        "after" | "afterAll" | "afterEach" | "xit" | "xtest" | "xdescribe" | "xcontext" => {
            Mocha::Ignored
        }
        _ => return None,
    })
}

/// A `describe` block, or the file's top level.
#[derive(Debug, Default)]
struct Block {
    parent: Option<usize>,
    /// The `describe` titles from the outermost down: `vault > withdraw`.
    path: String,
    /// The statements of the block that are not hooks or tests.
    body: Option<usize>,
    before: Vec<usize>,
    before_each: Vec<usize>,
}

struct TestRef {
    function: usize,
    block: usize,
    setup: bool,
}

struct Lower<'s> {
    src: &'s str,
    /// Byte offset of the start of each line.
    lines: Vec<usize>,
    /// Imported names: the local binding and the module it comes from.
    imported: BTreeMap<String, String>,
    functions: Vec<Function>,
    blocks: Vec<Block>,
    tests: Vec<TestRef>,
    /// Statements of closures met inside the expression being lowered. They
    /// run before the statement that contains them.
    pending: Vec<Stmt>,
}

/// An expression that stands for nothing: a closure, a class expression.
fn nothing(pos: Pos) -> Expr {
    Expr {
        kind: ExprKind::Other(Vec::new()),
        text: String::new(),
        pos,
    }
}

fn is_nothing(e: &Expr) -> bool {
    matches!(&e.kind, ExprKind::Other(items) if items.is_empty()) && e.text.is_empty()
}

/// A function's body: statements, or an arrow function's expression.
#[derive(Clone, Copy)]
enum Body<'a, 'b> {
    Stmts(&'b [Statement<'a>]),
    Expr(&'b Expression<'a>),
}

fn arrow_body<'a, 'b>(b: &'b ArrowFunctionBody<'a>) -> Body<'a, 'b> {
    match b {
        ArrowFunctionBody::FunctionBody(f) => Body::Stmts(&f.statements),
        other => other.as_expression().map_or(Body::Stmts(&[]), Body::Expr),
    }
}

fn fn_body<'a, 'b>(f: Option<&'b FunctionBody<'a>>) -> Body<'a, 'b> {
    Body::Stmts(f.map_or(&[][..], |b| &b.statements[..]))
}

/// The callback of a mocha call: its last function argument.
fn callback<'a, 'b>(call: &'b CallExpression<'a>) -> Option<Body<'a, 'b>> {
    call.arguments
        .iter()
        .rev()
        .find_map(|a| match a.as_expression()? {
            Expression::ArrowFunctionExpression(f) => Some(arrow_body(&f.body)),
            Expression::FunctionExpression(f) => Some(fn_body(f.body.as_deref())),
            _ => None,
        })
}

/// Whether a test's title says it builds the world: its leading verb is a
/// setup word in any inflection. `it("initializes the pool")`,
/// `it("is initialized")`, `it("funds the vault")` and `it("sets up")` do;
/// `it("steals funds")` and `it("attacker initializes a fake pool")` do not.
pub fn is_setup_title(title: &str) -> bool {
    const STEMS: &[&str] = &[
        "setup",
        "init",
        "initialize",
        "initialise",
        "create",
        "deploy",
        "bootstrap",
        "prepare",
        "fund",
        "configure",
        "mint",
        "airdrop",
    ];
    const LEADING: &[&str] = &["it", "is", "are", "should", "can", "will", "the", "a", "we"];
    let words = crate::identity::words(title);
    let mut rest = words.iter().skip_while(|w| LEADING.contains(&w.as_str()));
    let Some(verb) = rest.next() else {
        return false;
    };
    let inflected = |stem: &str| {
        ["", "s", "d", "ed", "es"]
            .iter()
            .any(|suffix| *verb == format!("{stem}{suffix}"))
    };
    STEMS.iter().any(|s| inflected(s))
        || (verb.starts_with("set") && rest.next().is_some_and(|w| w == "up"))
}

/// Assertion methods that compare two values, and the operator they assert.
fn comparison(name: &str) -> Option<&'static str> {
    Some(match name {
        "equal" | "equals" | "eq" | "eql" | "strictEqual" | "deepEqual" | "deepStrictEqual"
        | "toBe" | "toEqual" | "toStrictEqual" => "==",
        "notEqual" | "notStrictEqual" | "notDeepEqual" | "notDeepStrictEqual" => "!=",
        "above" | "gt" | "greaterThan" | "isAbove" | "toBeGreaterThan" => ">",
        "least" | "gte" | "greaterThanOrEqual" | "isAtLeast" | "toBeGreaterThanOrEqual" => ">=",
        "below" | "lt" | "lessThan" | "isBelow" | "toBeLessThan" => "<",
        "most" | "lte" | "lessThanOrEqual" | "isAtMost" | "toBeLessThanOrEqual" => "<=",
        _ => return None,
    })
}

/// Assertion methods that assert their subject is truthy (`true`) or
/// falsy (`false`).
fn truthiness(name: &str) -> Option<bool> {
    match name {
        "ok" | "isOk" | "isTrue" | "true" | "toBeTruthy" => Some(true),
        "notOk" | "isNotOk" | "isFalse" | "false" | "toBeFalsy" => Some(false),
        _ => None,
    }
}

fn negate(op: &str) -> &'static str {
    match op {
        "==" => "!=",
        "!=" => "==",
        ">" => "<=",
        ">=" => "<",
        "<" => ">=",
        _ => ">",
    }
}

/// BN, bigint and key comparison methods, as operators.
fn comparison_method(name: &str) -> Option<&'static str> {
    Some(match name {
        "gt" | "gtn" => ">",
        "gte" | "gten" => ">=",
        "lt" | "ltn" => "<",
        "lte" | "lten" => "<=",
        "eq" | "eqn" | "equals" => "==",
        _ => return None,
    })
}

/// Methods that convert a value without changing what it is.
const CONVERSIONS: &[&str] = &[
    "toNumber", "toString", "toBase58", "toBigInt", "toJSON", "valueOf",
];

impl<'s> Lower<'s> {
    fn new(src: &'s str) -> Lower<'s> {
        let lines = std::iter::once(0)
            .chain(src.match_indices('\n').map(|(i, _)| i + 1))
            .collect();
        Lower {
            src,
            lines,
            imported: BTreeMap::new(),
            functions: Vec::new(),
            blocks: Vec::new(),
            tests: Vec::new(),
            pending: Vec::new(),
        }
    }

    fn pos_at(&self, offset: u32) -> Pos {
        let offset = (offset as usize).min(self.src.len());
        let line = self.lines.partition_point(|&s| s <= offset).max(1);
        let start = self.lines[line - 1];
        let col = self.src.get(start..offset).map_or(0, |s| s.chars().count());
        Pos {
            line: line as u32,
            col: col as u32 + 1,
        }
    }

    fn pos(&self, span: Span) -> Pos {
        self.pos_at(span.start)
    }

    fn text(&self, span: Span) -> String {
        self.src
            .get(span.start as usize..span.end as usize)
            .unwrap_or_default()
            .to_string()
    }

    fn expr_of(&self, kind: ExprKind, span: Span) -> Expr {
        Expr {
            kind,
            text: self.text(span),
            pos: self.pos(span),
        }
    }

    // ----- imports -----------------------------------------------------

    /// Records every imported binding. Returns whether any comes from a
    /// Solana package.
    fn imports(&mut self, body: &[Statement<'_>]) -> bool {
        let mut solana = false;
        for s in body {
            match s {
                Statement::ImportDeclaration(d) => {
                    let source = d.source.value.as_str();
                    solana |= is_solana_package(source);
                    for spec in d.specifiers.iter().flatten() {
                        let local = match spec {
                            ImportDeclarationSpecifier::ImportSpecifier(s) => &s.local,
                            ImportDeclarationSpecifier::ImportDefaultSpecifier(s) => &s.local,
                            ImportDeclarationSpecifier::ImportNamespaceSpecifier(s) => &s.local,
                        };
                        self.imported
                            .insert(local.name.as_str().to_string(), source.to_string());
                    }
                }
                Statement::VariableDeclaration(d) => {
                    for decl in &d.declarations {
                        if let Some(source) = decl.init.as_ref().and_then(required_module) {
                            solana |= is_solana_package(source);
                            for name in binding_names(&decl.id) {
                                self.imported.insert(name, source.to_string());
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        solana
    }

    /// The path a member chain names when it starts at an imported binding or
    /// a capitalised name (a class): `anchor.web3.Keypair`, `PublicKey`.
    fn path_of(&self, e: &Expression<'_>) -> Option<String> {
        match e.without_parentheses() {
            Expression::Identifier(id) => {
                let name = id.name.as_str();
                (self.imported.contains_key(name)
                    || name.starts_with(|c: char| c.is_ascii_uppercase()))
                .then(|| name.to_string())
            }
            Expression::StaticMemberExpression(m) => self
                .path_of(&m.object)
                .map(|p| format!("{p}::{}", m.property.name.as_str())),
            _ => None,
        }
    }

    fn imported_from(&self, name: &str) -> Option<&str> {
        self.imported.get(name).map(String::as_str)
    }

    // ----- structure ---------------------------------------------------

    fn program(mut self, body: &[Statement<'_>]) -> File {
        self.blocks.push(Block::default());
        let mut top = Vec::new();
        for s in body {
            self.structural(s, 0, &mut top);
        }
        self.finish_block(0, top, "");
        let before_lists: Vec<(usize, Vec<Prior>)> = self
            .tests
            .iter()
            .enumerate()
            .map(|(i, t)| (t.function, self.before(i)))
            .collect();
        for (f, before) in before_lists {
            self.functions[f].before = before;
        }
        File {
            dialect: Dialect::TypeScript,
            functions: self.functions,
            consts: Vec::new(),
        }
    }

    /// The blocks from the file's top level down to `block`.
    fn ancestors(&self, block: usize) -> Vec<usize> {
        let mut chain = vec![block];
        while let Some(p) = self.blocks[*chain.last().expect("non-empty")].parent {
            chain.push(p);
        }
        chain.reverse();
        chain
    }

    fn before(&self, test: usize) -> Vec<Prior> {
        let chain = self.ancestors(self.tests[test].block);
        let setup = |function: usize| Prior {
            function,
            setup: true,
        };
        let mut out: Vec<Prior> = Vec::new();
        out.extend(chain.iter().filter_map(|&b| self.blocks[b].body).map(setup));
        for &b in &chain {
            out.extend(self.blocks[b].before.iter().copied().map(setup));
        }
        out.extend(self.tests[..test].iter().map(|t| Prior {
            function: t.function,
            setup: t.setup,
        }));
        for &b in &chain {
            out.extend(self.blocks[b].before_each.iter().copied().map(setup));
        }
        out
    }

    fn finish_block(&mut self, block: usize, body: Vec<Stmt>, header: &str) {
        if body.is_empty() {
            return;
        }
        let f = self.push_function(Function {
            name: header.to_string(),
            module: self.blocks[block].path.clone(),
            hook: true,
            header: header.to_string(),
            pos: body[0].pos,
            body,
            ..Function::default()
        });
        self.blocks[block].body = Some(f);
    }

    fn push_function(&mut self, f: Function) -> usize {
        self.functions.push(f);
        self.functions.len() - 1
    }

    /// A statement at the level of a file or a `describe` body, where mocha
    /// calls and declarations are structure.
    fn structural(&mut self, s: &Statement<'_>, block: usize, out: &mut Vec<Stmt>) {
        if let Statement::ExpressionStatement(es) = s
            && let Expression::CallExpression(call) = es.expression.without_parentheses()
            && let Some(kind) = self.mocha_call(call)
        {
            self.mocha(kind, call, block);
            return;
        }
        self.stmt(s, out);
    }

    fn mocha_call(&self, call: &CallExpression<'_>) -> Option<Mocha> {
        let kind = match call.callee.without_parentheses() {
            Expression::Identifier(id) => mocha_kind(id.name.as_str())?,
            Expression::StaticMemberExpression(m) => {
                let Expression::Identifier(id) = &m.object else {
                    return None;
                };
                let base = mocha_kind(id.name.as_str())?;
                match m.property.name.as_str() {
                    "only" => base,
                    "skip" | "todo" => Mocha::Ignored,
                    _ => return None,
                }
            }
            _ => return None,
        };
        callback(call).is_some().then_some(kind)
    }

    fn title(&self, call: &CallExpression<'_>) -> String {
        match call.arguments.first().and_then(Argument::as_expression) {
            Some(Expression::StringLiteral(s)) => s.value.as_str().to_string(),
            Some(Expression::TemplateLiteral(t)) if t.expressions.is_empty() => t
                .quasis
                .first()
                .map(|q| q.value.raw.as_str().to_string())
                .unwrap_or_default(),
            Some(e) => self.text(e.span()),
            None => String::new(),
        }
    }

    fn mocha(&mut self, kind: Mocha, call: &CallExpression<'_>, block: usize) {
        let Some(cb) = callback(call) else { return };
        let title = self.title(call);
        let callee = self.text(call.callee.span());
        let path = self.blocks[block].path.clone();
        match kind {
            Mocha::Ignored => {}
            Mocha::Describe => {
                let child = self.blocks.len();
                self.blocks.push(Block {
                    parent: Some(block),
                    path: if path.is_empty() {
                        title.clone()
                    } else {
                        format!("{path} > {title}")
                    },
                    ..Block::default()
                });
                let mut body = Vec::new();
                match cb {
                    Body::Stmts(stmts) => {
                        for s in stmts {
                            self.structural(s, child, &mut body);
                        }
                    }
                    Body::Expr(e) => self.push_expr(e, &mut body),
                }
                self.finish_block(child, body, &format!("{callee}({title:?})"));
            }
            Mocha::Test | Mocha::Before | Mocha::BeforeEach => {
                let body = self.isolated(cb);
                let test = kind == Mocha::Test;
                let header = if test {
                    format!("{callee}({title:?})")
                } else {
                    format!("{callee}(..)")
                };
                let f = self.push_function(Function {
                    name: if test { title.clone() } else { header.clone() },
                    module: path,
                    test,
                    hook: !test,
                    header,
                    body,
                    pos: self.pos(call.span),
                    ..Function::default()
                });
                match kind {
                    Mocha::Test => self.tests.push(TestRef {
                        function: f,
                        block,
                        setup: is_setup_title(&title),
                    }),
                    Mocha::Before => self.blocks[block].before.push(f),
                    _ => self.blocks[block].before_each.push(f),
                }
            }
        }
    }

    fn lower_body(&mut self, body: Body<'_, '_>, out: &mut Vec<Stmt>) {
        match body {
            Body::Stmts(stmts) => self.stmts(stmts, out),
            Body::Expr(e) => self.push_expr(e, out),
        }
    }

    /// A function body lowered on its own, apart from the statement being
    /// lowered around it.
    fn isolated(&mut self, body: Body<'_, '_>) -> Vec<Stmt> {
        let saved = std::mem::take(&mut self.pending);
        let mut out = Vec::new();
        self.lower_body(body, &mut out);
        out.append(&mut self.pending);
        self.pending = saved;
        out
    }

    // ----- helpers -----------------------------------------------------

    /// Registers a helper function: a function declaration, an arrow function
    /// bound to a name, or a class method. Returns its index.
    fn helper(
        &mut self,
        name: &str,
        params: &FormalParameters<'_>,
        body: Body<'_, '_>,
        span: Span,
    ) -> usize {
        let body = self.isolated(body);
        let names: Vec<String> = params
            .items
            .iter()
            .map(|p| match &p.pattern {
                BindingPattern::BindingIdentifier(id) => id.name.as_str().to_string(),
                _ => String::new(),
            })
            .collect();
        let required = params
            .items
            .iter()
            .take_while(|p| p.initializer.is_none() && !p.optional)
            .count();
        let header = self
            .text(span)
            .lines()
            .next()
            .unwrap_or_default()
            .trim()
            .to_string();
        self.push_function(Function {
            name: snake(name),
            params: names,
            required,
            variadic: params.rest.is_some(),
            header,
            body,
            pos: self.pos(span),
            ..Function::default()
        })
    }

    /// A function declaration: `function setup() { .. }`.
    fn function_decl(&mut self, f: &oxc_ast::ast::Function<'_>) {
        let name = f.id.as_ref().map_or("default", |id| id.name.as_str());
        self.helper(name, &f.params, fn_body(f.body.as_deref()), f.span);
    }

    fn class(&mut self, class: &Class<'_>) {
        let Some(name) = class.id.as_ref().map(|id| id.name.as_str().to_string()) else {
            return;
        };
        // Field initializers run when an instance is constructed.
        let mut fields = Vec::new();
        for el in &class.body.body {
            if let ClassElement::PropertyDefinition(p) = el
                && !p.r#static
                && let (Some(key), Some(value)) = (p.key.static_name(), &p.value)
            {
                let target = Expr {
                    kind: ExprKind::Member(
                        Box::new(Expr {
                            kind: ExprKind::Ident("self".to_string()),
                            text: "this".to_string(),
                            pos: self.pos(p.span),
                        }),
                        key.to_string(),
                    ),
                    text: format!("this.{key}"),
                    pos: self.pos(p.span),
                };
                let value = self.expr(value);
                let stmt = Stmt {
                    pos: self.pos(p.span),
                    kind: StmtKind::Expr(
                        self.expr_of(ExprKind::Assign(Box::new(target), Box::new(value)), p.span),
                    ),
                };
                fields.extend(std::mem::take(&mut self.pending));
                fields.push(stmt);
            }
        }
        let mut constructed = false;
        for el in &class.body.body {
            let ClassElement::MethodDefinition(m) = el else {
                continue;
            };
            let Some(key) = m.key.static_name() else {
                continue;
            };
            let ctor = m.kind == MethodDefinitionKind::Constructor;
            let fname = if ctor { "new" } else { key.as_ref() };
            let f = self.helper(
                fname,
                &m.value.params,
                fn_body(m.value.body.as_deref()),
                m.span,
            );
            let method = &mut self.functions[f];
            method.owner = Some(name.clone());
            method.method = !ctor && !m.r#static;
            if ctor {
                constructed = true;
                method.body.splice(0..0, std::mem::take(&mut fields));
            }
        }
        if !constructed && !fields.is_empty() {
            self.functions.push(Function {
                name: "new".to_string(),
                owner: Some(name),
                header: "constructor()".to_string(),
                pos: fields[0].pos,
                body: fields,
                ..Function::default()
            });
        }
    }

    // ----- statements --------------------------------------------------

    fn push(&mut self, kind: StmtKind, span: Span, out: &mut Vec<Stmt>) {
        out.append(&mut self.pending);
        out.push(Stmt {
            kind,
            pos: self.pos(span),
        });
    }

    fn push_expr(&mut self, e: &Expression<'_>, out: &mut Vec<Stmt>) {
        let x = self.expr(e);
        if is_nothing(&x) {
            out.append(&mut self.pending);
        } else {
            self.push(StmtKind::Expr(x), e.span(), out);
        }
    }

    fn stmt(&mut self, s: &Statement<'_>, out: &mut Vec<Stmt>) {
        if let Some(d) = s.as_declaration() {
            self.declaration(d, out);
            return;
        }
        match s {
            Statement::ExpressionStatement(es) => self.push_expr(&es.expression, out),
            Statement::ExportNamedDeclaration(_)
            | Statement::ExportAllDeclaration(_)
            | Statement::ExportFromDeclaration(_)
            | Statement::ImportDeclaration(_) => {}
            Statement::ExportDeclaration(e) => {
                self.declaration(&e.declaration, out);
            }
            Statement::ExportDefaultDeclaration(e) => match &e.declaration {
                ExportDefaultDeclarationKind::FunctionDeclaration(f) => self.function_decl(f),
                ExportDefaultDeclarationKind::ClassDeclaration(c) => self.class(c),
                _ => {}
            },
            Statement::BlockStatement(b) => self.stmts(&b.body, out),
            Statement::IfStatement(i) => {
                if i.alternate.is_none() && always_fails(&i.consequent) {
                    let cond = self.expr(&i.test);
                    let negated = Expr {
                        kind: ExprKind::Unary("!".to_string(), Box::new(cond)),
                        text: format!("!({})", self.text(i.test.span())),
                        pos: self.pos(i.test.span()),
                    };
                    let assert = self.macro_call("assert!", vec![negated], i.span);
                    self.push(StmtKind::Expr(assert), i.span, out);
                    return;
                }
                self.push_expr(&i.test, out);
                self.stmt(&i.consequent, out);
                if let Some(alt) = &i.alternate {
                    self.stmt(alt, out);
                }
            }
            Statement::ForStatement(f) => {
                match &f.init {
                    Some(ForStatementInit::VariableDeclaration(d)) => self.var_decl(d, out),
                    Some(init) => {
                        if let Some(e) = init.as_expression() {
                            self.push_expr(e, out);
                        }
                    }
                    None => {}
                }
                if let Some(t) = &f.test {
                    self.push_expr(t, out);
                }
                self.stmt(&f.body, out);
                if let Some(u) = &f.update {
                    self.push_expr(u, out);
                }
            }
            Statement::ForOfStatement(f) => {
                self.push_expr(&f.right, out);
                self.stmt(&f.body, out);
            }
            Statement::ForInStatement(f) => {
                self.push_expr(&f.right, out);
                self.stmt(&f.body, out);
            }
            Statement::WhileStatement(w) => {
                self.push_expr(&w.test, out);
                self.stmt(&w.body, out);
            }
            Statement::DoWhileStatement(w) => {
                self.stmt(&w.body, out);
                self.push_expr(&w.test, out);
            }
            Statement::TryStatement(t) => {
                self.stmts(&t.block.body, out);
                if let Some(h) = &t.handler {
                    self.stmts(&h.body.body, out);
                }
                if let Some(f) = &t.finalizer {
                    self.stmts(&f.body, out);
                }
            }
            Statement::SwitchStatement(sw) => {
                self.push_expr(&sw.discriminant, out);
                for case in &sw.cases {
                    self.stmts(&case.consequent, out);
                }
            }
            Statement::LabeledStatement(l) => self.stmt(&l.body, out),
            Statement::ReturnStatement(r) => {
                let value = r.argument.as_ref().map(|a| self.expr(a));
                self.push(StmtKind::Return(value), r.span, out);
            }
            Statement::ThrowStatement(t) => self.push(StmtKind::Revert, t.span, out),
            _ => {}
        }
    }

    fn stmts(&mut self, stmts: &[Statement<'_>], out: &mut Vec<Stmt>) {
        for s in stmts {
            self.stmt(s, out);
        }
    }

    fn declaration(&mut self, d: &Declaration<'_>, out: &mut Vec<Stmt>) {
        match d {
            Declaration::VariableDeclaration(v) => self.var_decl(v, out),
            Declaration::FunctionDeclaration(f) => self.function_decl(f),
            Declaration::ClassDeclaration(c) => self.class(c),
            _ => {}
        }
    }

    fn var_decl(&mut self, d: &VariableDeclaration<'_>, out: &mut Vec<Stmt>) {
        for decl in &d.declarations {
            let Some(init) = &decl.init else {
                // `let vault: PublicKey;`, assigned later in a hook.
                continue;
            };
            if required_module(init).is_some() {
                continue;
            }
            // `const { Keypair, SystemProgram } = anchor.web3`: aliases of an
            // imported namespace.
            if let BindingPattern::ObjectPattern(_) = &decl.id
                && let Some(base) = self.path_of(init)
            {
                for name in binding_names(&decl.id) {
                    self.imported.insert(name, base.clone());
                }
                continue;
            }
            // `const send = async (ix, signers) => { .. }` is a helper.
            if let BindingPattern::BindingIdentifier(id) = &decl.id {
                match init.without_parentheses() {
                    Expression::ArrowFunctionExpression(f) => {
                        self.helper(id.name.as_str(), &f.params, arrow_body(&f.body), decl.span);
                        continue;
                    }
                    Expression::FunctionExpression(f) => {
                        self.helper(
                            id.name.as_str(),
                            &f.params,
                            fn_body(f.body.as_deref()),
                            decl.span,
                        );
                        continue;
                    }
                    _ => {}
                }
            }
            let ty = decl
                .type_annotation
                .as_ref()
                .map(|t| self.text(t.type_annotation.span()))
                .unwrap_or_default();
            let vars = binding_names(&decl.id)
                .into_iter()
                .map(|n| (n, ty.clone()))
                .collect();
            let init = self.expr(init);
            self.push(
                StmtKind::Decl {
                    vars,
                    init: Some(init),
                },
                decl.span,
                out,
            );
        }
    }

    // ----- expressions -------------------------------------------------

    fn exprs(&mut self, args: &[Argument<'_>]) -> Vec<Expr> {
        args.iter()
            .map(|a| match a {
                Argument::SpreadElement(s) => self.expr(&s.argument),
                other => self.expr(other.to_expression()),
            })
            .collect()
    }

    fn expr(&mut self, e: &Expression<'_>) -> Expr {
        let span = e.span();
        match e {
            Expression::BooleanLiteral(b) => self.expr_of(ExprKind::Lit(Lit::Bool(b.value)), span),
            Expression::NumericLiteral(_) | Expression::BigIntLiteral(_) => {
                self.expr_of(ExprKind::Lit(Lit::Number), span)
            }
            Expression::StringLiteral(s) => {
                self.expr_of(ExprKind::Lit(Lit::Str(s.value.as_str().to_string())), span)
            }
            Expression::TemplateLiteral(t) if t.expressions.is_empty() => {
                let value = t
                    .quasis
                    .first()
                    .map(|q| q.value.raw.as_str().to_string())
                    .unwrap_or_default();
                self.expr_of(ExprKind::Lit(Lit::Str(value)), span)
            }
            Expression::TemplateLiteral(t) => {
                let parts = t.expressions.iter().map(|x| self.expr(x)).collect();
                self.expr_of(ExprKind::Other(parts), span)
            }
            Expression::NullLiteral(_) | Expression::RegExpLiteral(_) => {
                self.expr_of(ExprKind::Lit(Lit::Other), span)
            }
            Expression::Identifier(id) if id.name.as_str() == "undefined" => {
                self.expr_of(ExprKind::Lit(Lit::Other), span)
            }
            Expression::Identifier(id) => {
                self.expr_of(ExprKind::Ident(id.name.as_str().to_string()), span)
            }
            Expression::ThisExpression(_) => {
                self.expr_of(ExprKind::Ident("self".to_string()), span)
            }
            Expression::Super(_) => self.expr_of(ExprKind::Ident("super".to_string()), span),
            Expression::ParenthesizedExpression(p) => self.expr(&p.expression),
            Expression::AwaitExpression(a) => self.expr(&a.argument),
            Expression::TSAsExpression(x) => self.expr(&x.expression),
            Expression::TSSatisfiesExpression(x) => self.expr(&x.expression),
            Expression::TSTypeAssertion(x) => self.expr(&x.expression),
            Expression::TSNonNullExpression(x) => self.expr(&x.expression),
            Expression::TSInstantiationExpression(x) => self.expr(&x.expression),
            Expression::ChainExpression(c) => match &c.expression {
                ChainElement::CallExpression(call) => self.call(call, span),
                ChainElement::TSNonNullExpression(x) => self.expr(&x.expression),
                other => match other.as_member_expression() {
                    Some(m) => self.member(m),
                    None => nothing(self.pos(span)),
                },
            },
            Expression::StaticMemberExpression(_)
            | Expression::ComputedMemberExpression(_)
            | Expression::PrivateFieldExpression(_) => {
                if let Some(a) = self.assertion(e) {
                    return a;
                }
                match e.as_member_expression() {
                    Some(m) => self.member(m),
                    None => nothing(self.pos(span)),
                }
            }
            Expression::CallExpression(c) => match self.assertion(e) {
                Some(a) => a,
                None => self.call(c, span),
            },
            Expression::NewExpression(n) => self.new_expr(n),
            Expression::ArrayExpression(a) => {
                let items = a
                    .elements
                    .iter()
                    .filter_map(|el| match el {
                        ArrayExpressionElement::SpreadElement(s) => Some(&s.argument),
                        ArrayExpressionElement::Elision(_) => None,
                        other => other.as_expression(),
                    })
                    .collect::<Vec<_>>();
                let items = items.into_iter().map(|x| self.expr(x)).collect();
                self.expr_of(ExprKind::Array(items), span)
            }
            Expression::ObjectExpression(o) => self.object(o),
            Expression::BinaryExpression(b) => {
                let op = match b.operator.as_str() {
                    "===" => "==",
                    "!==" => "!=",
                    other => other,
                };
                let (l, r) = (self.expr(&b.left), self.expr(&b.right));
                self.expr_of(
                    ExprKind::Binary(Box::new(l), op.to_string(), Box::new(r)),
                    span,
                )
            }
            Expression::LogicalExpression(b) => {
                let (l, r) = (self.expr(&b.left), self.expr(&b.right));
                let op = b.operator.as_str().to_string();
                self.expr_of(ExprKind::Binary(Box::new(l), op, Box::new(r)), span)
            }
            Expression::UnaryExpression(u) => {
                let inner = self.expr(&u.argument);
                self.expr_of(
                    ExprKind::Unary(u.operator.as_str().to_string(), Box::new(inner)),
                    span,
                )
            }
            Expression::ConditionalExpression(c) => {
                let (t, a, b) = (
                    self.expr(&c.test),
                    self.expr(&c.consequent),
                    self.expr(&c.alternate),
                );
                self.expr_of(
                    ExprKind::Ternary(Box::new(t), Box::new(a), Box::new(b)),
                    span,
                )
            }
            Expression::AssignmentExpression(a) => {
                let value = self.expr(&a.right);
                let target = match a.left.as_simple_assignment_target() {
                    Some(SimpleAssignmentTarget::AssignmentTargetIdentifier(id)) => {
                        Some(self.expr_of(ExprKind::Ident(id.name.as_str().to_string()), id.span))
                    }
                    Some(t) => t.as_member_expression().map(|m| self.member(m)),
                    None => None,
                };
                match (a.operator.as_str(), target) {
                    ("=", Some(t)) => {
                        self.expr_of(ExprKind::Assign(Box::new(t), Box::new(value)), span)
                    }
                    (_, t) => {
                        let parts = t.into_iter().chain(std::iter::once(value)).collect();
                        self.expr_of(ExprKind::Other(parts), span)
                    }
                }
            }
            Expression::SequenceExpression(s) => {
                let parts = s.expressions.iter().map(|x| self.expr(x)).collect();
                self.expr_of(ExprKind::Other(parts), span)
            }
            Expression::ArrowFunctionExpression(f) => {
                self.closure(arrow_body(&f.body));
                nothing(self.pos(span))
            }
            Expression::FunctionExpression(f) => {
                self.closure(fn_body(f.body.as_deref()));
                nothing(self.pos(span))
            }
            Expression::TaggedTemplateExpression(t) => {
                let parts = t.quasi.expressions.iter().map(|x| self.expr(x)).collect();
                self.expr_of(ExprKind::Other(parts), span)
            }
            Expression::YieldExpression(y) => {
                let parts = y.argument.iter().map(|x| self.expr(x)).collect();
                self.expr_of(ExprKind::Other(parts), span)
            }
            _ => self.expr_of(ExprKind::Other(Vec::new()), span),
        }
    }

    /// A closure's statements run inline, before the statement that holds it.
    fn closure(&mut self, body: Body<'_, '_>) {
        let lowered = self.isolated(body);
        self.pending.extend(lowered);
    }

    fn member(&mut self, m: &MemberExpression<'_>) -> Expr {
        let span = m.span();
        match m {
            MemberExpression::StaticMemberExpression(s) => {
                let prop = s.property.name.as_str();
                if prop == "publicKey" {
                    let base = self.expr(&s.object);
                    let callee =
                        self.expr_of(ExprKind::Member(Box::new(base), "pubkey".to_string()), span);
                    return self.expr_of(
                        ExprKind::Call {
                            callee: Box::new(callee),
                            args: Vec::new(),
                            options: Vec::new(),
                        },
                        span,
                    );
                }
                if let Some(path) = self.path_of(&s.object) {
                    return self.expr_of(ExprKind::Ident(format!("{path}::{prop}")), span);
                }
                let base = self.expr(&s.object);
                self.expr_of(ExprKind::Member(Box::new(base), prop.to_string()), span)
            }
            MemberExpression::ComputedMemberExpression(c) => {
                let (base, index) = (self.expr(&c.object), self.expr(&c.expression));
                self.expr_of(ExprKind::Index(Box::new(base), vec![index]), span)
            }
            MemberExpression::PrivateFieldExpression(p) => {
                let base = self.expr(&p.object);
                let name = format!("#{}", p.field.name.as_str());
                self.expr_of(ExprKind::Member(Box::new(base), name), span)
            }
        }
    }

    fn object(&mut self, o: &ObjectExpression<'_>) -> Expr {
        let mut fields = Vec::new();
        for p in &o.properties {
            if let ObjectPropertyKind::ObjectProperty(p) = p
                && !p.method
                && p.kind == PropertyKind::Init
                && let Some(key) = p.key.static_name()
            {
                fields.push((key.to_string(), self.expr(&p.value)));
            }
        }
        let callee = Expr {
            kind: ExprKind::Type(String::new()),
            text: String::new(),
            pos: self.pos(o.span),
        };
        self.expr_of(
            ExprKind::Call {
                callee: Box::new(callee),
                args: Vec::new(),
                options: fields,
            },
            o.span,
        )
    }

    fn new_expr(&mut self, n: &NewExpression<'_>) -> Expr {
        let span = n.span;
        let path = self
            .path_of(&n.callee)
            .unwrap_or_else(|| self.text(n.callee.span()));
        let last = path.rsplit("::").next().unwrap_or(&path);
        if last == "BN"
            && let Some(first) = n.arguments.first().and_then(Argument::as_expression)
        {
            return self.expr(first);
        }
        let string_arg = matches!(
            n.arguments.first().and_then(Argument::as_expression),
            Some(Expression::StringLiteral(_))
        );
        let ctor = if last == "PublicKey" && string_arg {
            "from_str"
        } else {
            "new"
        };
        let args = self.exprs(&n.arguments);
        let callee = self.expr_of(ExprKind::Ident(format!("{path}::{ctor}")), n.callee.span());
        self.expr_of(
            ExprKind::Call {
                callee: Box::new(callee),
                args,
                options: Vec::new(),
            },
            span,
        )
    }

    /// A call to the macro `name` (`assert!`, `println!`) standing for the
    /// call at `span`.
    fn macro_call(&self, name: &str, args: Vec<Expr>, span: Span) -> Expr {
        let callee = Expr {
            kind: ExprKind::Ident(name.to_string()),
            text: name.trim_end_matches('!').to_string(),
            pos: self.pos(span),
        };
        self.expr_of(
            ExprKind::Call {
                callee: Box::new(callee),
                args,
                options: Vec::new(),
            },
            span,
        )
    }

    /// A chai, node or jest assertion, as the Rust assertion macro it means:
    /// `assert.equal(a, b)` and `expect(a).to.equal(b)` are `assert_eq!(a, b)`,
    /// `expect(a).to.be.greaterThan(b)` is `assert!(a > b)`, `assert(c)` is
    /// `assert!(c)`. Assertions that do not compare are `assert_that!`.
    fn assertion(&mut self, e: &Expression<'_>) -> Option<Expr> {
        let outer = e.without_parentheses();
        let outer_args: &[Argument<'_>] = match outer {
            Expression::CallExpression(c) => &c.arguments,
            _ => &[],
        };
        // The member names from the matcher inward, and what the chain starts
        // at: `expect(subject)`, or `assert` (`None`).
        let mut names: Vec<&str> = Vec::new();
        let mut cur = outer;
        let subject = loop {
            match cur {
                Expression::CallExpression(c) => match c.callee.without_parentheses() {
                    Expression::Identifier(id) if id.name.as_str() == "expect" => {
                        break Some(c.arguments.first().and_then(Argument::as_expression));
                    }
                    Expression::StaticMemberExpression(m)
                        if m.property.name.as_str() == "expect" =>
                    {
                        break Some(c.arguments.first().and_then(Argument::as_expression));
                    }
                    Expression::Identifier(id)
                        if id.name.as_str() == "assert" && names.is_empty() =>
                    {
                        break None;
                    }
                    Expression::StaticMemberExpression(m) => {
                        names.push(m.property.name.as_str());
                        if is_assert_root(&m.object) {
                            break None;
                        }
                        cur = &m.object;
                    }
                    _ => return None,
                },
                Expression::StaticMemberExpression(m) => {
                    names.push(m.property.name.as_str());
                    cur = &m.object;
                }
                _ => return None,
            }
        };
        let mut args = outer_args.iter().filter_map(Argument::as_expression);
        let style_expect = subject.is_some();
        let operands: Vec<&Expression<'_>> = match subject {
            Some(subject) => subject.into_iter().chain(args.next()).collect(),
            // `assert.x(..)` and `assert(..)` are calls; a bare `assert.x`
            // property is not an assertion.
            None if matches!(outer, Expression::CallExpression(_)) => args.take(2).collect(),
            None => return None,
        };
        let span = e.span();
        let method = names.first().copied().unwrap_or("assert");
        let negated = style_expect && names.contains(&"not");
        let lowered: Vec<Expr> = operands.iter().map(|x| self.expr(x)).collect();
        Some(
            match (comparison(method), truthiness(method), lowered.as_slice()) {
                (Some(op), _, [a, b]) => match if negated { negate(op) } else { op } {
                    "==" => self.macro_call("assert_eq!", lowered.clone(), span),
                    "!=" => self.macro_call("assert_ne!", lowered.clone(), span),
                    op => {
                        let cond = self.expr_of(
                            ExprKind::Binary(
                                Box::new(a.clone()),
                                op.to_string(),
                                Box::new(b.clone()),
                            ),
                            span,
                        );
                        self.macro_call("assert!", vec![cond], span)
                    }
                },
                (_, Some(truthy), [subject, ..]) => {
                    let cond = if truthy != negated {
                        subject.clone()
                    } else {
                        self.expr_of(
                            ExprKind::Unary("!".to_string(), Box::new(subject.clone())),
                            span,
                        )
                    };
                    self.macro_call("assert!", vec![cond], span)
                }
                _ if !style_expect && names.is_empty() => self.macro_call("assert!", lowered, span),
                _ => self.macro_call("assert_that!", lowered, span),
            },
        )
    }

    fn call(&mut self, c: &CallExpression<'_>, span: Span) -> Expr {
        let callee_span = c.callee.span();
        match c.callee.without_parentheses() {
            Expression::StaticMemberExpression(m) => {
                let prop = m.property.name.as_str();
                if matches!(&m.object, Expression::Identifier(id) if id.name.as_str() == "console")
                {
                    let args = self.exprs(&c.arguments);
                    return self.macro_call("println!", args, span);
                }
                if c.arguments.is_empty() && CONVERSIONS.contains(&prop) {
                    return self.expr(&m.object);
                }
                if let (Some(op), [arg]) = (comparison_method(prop), &c.arguments[..])
                    && let Some(arg) = arg.as_expression()
                {
                    let (l, r) = (self.expr(&m.object), self.expr(arg));
                    return self.expr_of(
                        ExprKind::Binary(Box::new(l), op.to_string(), Box::new(r)),
                        span,
                    );
                }
                let callee = match self.path_of(&m.object) {
                    Some(path) => self.expr_of(
                        ExprKind::Ident(format!("{path}::{}", snake(prop))),
                        callee_span,
                    ),
                    None => {
                        let base = self.expr(&m.object);
                        self.expr_of(ExprKind::Member(Box::new(base), snake(prop)), callee_span)
                    }
                };
                let args = self.exprs(&c.arguments);
                self.finish_call(callee, args, span)
            }
            Expression::Identifier(id) => {
                let name = id.name.as_str();
                if matches!(name, "BigInt" | "Number" | "String")
                    && let [arg] = &c.arguments[..]
                    && let Some(arg) = arg.as_expression()
                {
                    return self.expr(arg);
                }
                let mut args = self.exprs(&c.arguments);
                let name = snake(name);
                match name.as_str() {
                    "get_associated_token_address" | "get_associated_token_address_sync"
                        if args.len() >= 2 =>
                    {
                        args.swap(0, 1)
                    }
                    "start_anchor" => self.bankrun_entries(&mut args, 1, 2, "startAnchor"),
                    "start" if self.imported_from(id.name.as_str()).is_some_and(is_bankrun) => {
                        self.bankrun_entries(&mut args, 0, 1, "start")
                    }
                    _ => {}
                }
                let callee = self.expr_of(ExprKind::Ident(name), callee_span);
                self.finish_call(callee, args, span)
            }
            other => {
                let callee = self.expr(other);
                let args = self.exprs(&c.arguments);
                self.finish_call(callee, args, span)
            }
        }
    }

    /// Turns the program and account lists of a bankrun `startAnchor` or
    /// `start` call into `add_program` and `set_account` calls, so each entry
    /// is an op of its own.
    fn bankrun_entries(
        &mut self,
        args: &mut [Expr],
        programs: usize,
        accounts: usize,
        callee: &str,
    ) {
        for (index, field_pair, method) in [
            (programs, ("programId", "name"), "add_program"),
            (accounts, ("address", "info"), "set_account"),
        ] {
            let Some(ExprKind::Array(items)) = args.get_mut(index).map(|a| &mut a.kind) else {
                continue;
            };
            for item in items.iter_mut() {
                let ExprKind::Call {
                    callee: ty,
                    options,
                    ..
                } = &item.kind
                else {
                    continue;
                };
                if !matches!(&ty.kind, ExprKind::Type(t) if t.is_empty()) {
                    continue;
                }
                let get = |k: &str| options.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone());
                let (Some(first), Some(second)) = (get(field_pair.0), get(field_pair.1)) else {
                    continue;
                };
                let receiver = Expr {
                    kind: ExprKind::Ident("bankrun".to_string()),
                    text: String::new(),
                    pos: item.pos,
                };
                let member = Expr {
                    kind: ExprKind::Member(Box::new(receiver), method.to_string()),
                    text: callee.to_string(),
                    pos: item.pos,
                };
                item.kind = ExprKind::Call {
                    callee: Box::new(member),
                    args: vec![first, second],
                    options: Vec::new(),
                };
            }
        }
    }

    fn finish_call(&self, callee: Expr, args: Vec<Expr>, span: Span) -> Expr {
        self.expr_of(
            ExprKind::Call {
                callee: Box::new(callee),
                args,
                options: Vec::new(),
            },
            span,
        )
    }
}

/// `assert` or `chai.assert`, the root of an `assert.equal(..)` chain.
fn is_assert_root(e: &Expression<'_>) -> bool {
    match e.without_parentheses() {
        Expression::Identifier(id) => id.name.as_str() == "assert",
        Expression::StaticMemberExpression(m) => m.property.name.as_str() == "assert",
        _ => false,
    }
}

/// The module of `require("x")`.
fn required_module<'a>(e: &Expression<'a>) -> Option<&'a str> {
    let Expression::CallExpression(c) = e.without_parentheses() else {
        return None;
    };
    let Expression::Identifier(id) = &c.callee else {
        return None;
    };
    if id.name.as_str() != "require" {
        return None;
    }
    match c.arguments.first().and_then(Argument::as_expression) {
        Some(Expression::StringLiteral(s)) => Some(s.value.as_str()),
        _ => None,
    }
}

/// The names a binding pattern declares: `vault`, `[vault, bump]`,
/// `{ svm, payer }`.
fn binding_names(p: &BindingPattern<'_>) -> Vec<String> {
    match p {
        BindingPattern::BindingIdentifier(id) => vec![id.name.as_str().to_string()],
        BindingPattern::ArrayPattern(a) => a
            .elements
            .iter()
            .map(|el| match el {
                Some(BindingPattern::BindingIdentifier(id)) => id.name.as_str().to_string(),
                _ => String::new(),
            })
            .collect(),
        BindingPattern::ObjectPattern(o) => o
            .properties
            .iter()
            .flat_map(|prop| binding_names(&prop.value))
            .collect(),
        BindingPattern::AssignmentPattern(a) => binding_names(&a.left),
    }
}

/// Whether running this statement always fails the test: `throw ..`, or a
/// block that throws or calls `assert.fail(..)` / `expect.fail(..)`.
fn always_fails(s: &Statement<'_>) -> bool {
    match s {
        Statement::ThrowStatement(_) => true,
        Statement::BlockStatement(b) => b.body.iter().any(always_fails),
        Statement::ExpressionStatement(es) => match es.expression.without_parentheses() {
            Expression::CallExpression(c) => matches!(
                c.callee.without_parentheses(),
                Expression::StaticMemberExpression(m)
                    if m.property.name.as_str() == "fail" && is_fail_root(&m.object)
            ),
            _ => false,
        },
        _ => false,
    }
}

fn is_fail_root(e: &Expression<'_>) -> bool {
    matches!(e.without_parentheses(), Expression::Identifier(id) if matches!(id.name.as_str(), "assert" | "expect"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const IMPORTS: &str = "import * as anchor from \"@coral-xyz/anchor\";\nimport { Keypair, PublicKey } from \"@solana/web3.js\";\nimport { assert, expect } from \"chai\";\n";

    fn parse(src: &str) -> File {
        let unit = parse_source("t.ts", &format!("{IMPORTS}{src}")).expect("a Solana file");
        assert!(unit.errors.is_empty(), "{:?}", unit.errors);
        unit.solana.unwrap()
    }

    /// Every call in an expression, inner first: `.method`, a path, a macro
    /// name, or `{}` for an object literal.
    fn calls(e: &Expr, out: &mut Vec<String>) {
        for c in e.children() {
            calls(c, out);
        }
        if let ExprKind::Call { callee, .. } = &e.kind {
            out.push(match &callee.kind {
                ExprKind::Member(_, m) => format!(".{m}"),
                ExprKind::Ident(n) => n.clone(),
                ExprKind::Type(n) => format!("{{{n}}}"),
                _ => "?".to_string(),
            });
        }
    }

    fn body_calls(f: &Function) -> Vec<String> {
        let mut out = Vec::new();
        for s in &f.body {
            match &s.kind {
                StmtKind::Expr(e) => calls(e, &mut out),
                StmtKind::Decl { init: Some(e), .. } | StmtKind::Return(Some(e)) => {
                    calls(e, &mut out)
                }
                _ => {}
            }
        }
        out
    }

    fn test<'f>(file: &'f File, name: &str) -> &'f Function {
        file.functions
            .iter()
            .find(|f| f.test && f.name == name)
            .unwrap()
    }

    #[test]
    fn called_names_are_snake_case() {
        for (camel, snake_case) in [
            ("setAccount", "set_account"),
            (
                "getAssociatedTokenAddressSync",
                "get_associated_token_address_sync",
            ),
            ("toBase58", "to_base58"),
            ("getATA", "get_ata"),
            ("rpc", "rpc"),
            ("TOKEN_PROGRAM_ID", "TOKEN_PROGRAM_ID"),
        ] {
            assert_eq!(snake(camel), snake_case);
        }
    }

    #[test]
    fn only_files_that_import_a_solana_package_are_read() {
        let hardhat = "import { ethers } from \"hardhat\";\nit(\"deploys\", async () => {});";
        assert!(parse_source("t.ts", hardhat).is_none());
        let required = "const anchor = require(\"@coral-xyz/anchor\");\nit(\"x\", () => {});";
        assert!(parse_source("t.js", required).is_some());
        assert!(is_script_path("tests/vault.ts") && is_script_path("tests/vault.mjs"));
        assert!(!is_script_path("types/vault.d.ts"));
    }

    #[test]
    fn a_file_that_does_not_parse_reports_where() {
        let src = format!("{IMPORTS}it(\"x\", () => {{ let = ; }});");
        let unit = parse_source("t.ts", &src).unwrap();
        assert_eq!(unit.errors.len(), 1, "{:?}", unit.errors);
        assert!(unit.errors[0].contains("line 4"), "{}", unit.errors[0]);
    }

    #[test]
    fn mocha_runs_hooks_and_earlier_tests_first() {
        let f = parse(
            r#"
            const provider = anchor.AnchorProvider.env();
            describe("vault", () => {
              const admin = Keypair.generate();
              beforeEach(async () => { await reset(); });
              before(async () => { await fund(admin); });
              it("is initialized", async () => { await init(); });
              it.skip("is skipped", async () => {});
              describe("exploit", () => {
                it("drains", async () => { await drain(); });
              });
              after(async () => { await cleanup(); });
            });
        "#,
        );
        let drains = test(&f, "drains");
        assert_eq!(drains.module, "vault > exploit");
        assert_eq!(drains.header, "it(\"drains\")");
        let order: Vec<(Vec<String>, bool)> = drains
            .before
            .iter()
            .map(|p| (body_calls(&f.functions[p.function]), p.setup))
            .collect();
        let calls = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            order,
            vec![
                (calls(&["anchor::AnchorProvider::env"]), true),
                (calls(&["Keypair::generate"]), true),
                (calls(&["fund"]), true),
                (calls(&["init"]), true),
                (calls(&["reset"]), true),
            ]
        );
        assert!(f.functions.iter().all(|x| x.name != "is skipped"));
    }

    #[test]
    fn calls_on_imports_are_paths_and_public_keys_are_pubkey() {
        let f = parse(
            r#"
            it("t", async () => {
              const kp = anchor.web3.Keypair.generate();
              const [vault] = PublicKey.findProgramAddressSync([], program.programId);
              const fixed = new PublicKey("11111111111111111111111111111111");
              await svm.setAccount(kp.publicKey, { lamports: 1n });
              const ata = getAssociatedTokenAddressSync(mint, owner);
              this.env.drainVault();
              console.log(await new BN(5).toNumber());
            });
        "#,
        );
        assert_eq!(
            body_calls(test(&f, "t")),
            vec![
                "anchor::web3::Keypair::generate",
                "PublicKey::find_program_address_sync",
                "PublicKey::from_str",
                ".pubkey",
                "{}",
                ".set_account",
                "get_associated_token_address_sync",
                ".drain_vault",
                "println!",
            ]
        );
        let StmtKind::Decl {
            init: Some(ata), ..
        } = &test(&f, "t").body[4].kind
        else {
            panic!()
        };
        let ExprKind::Call { args, .. } = &ata.kind else {
            panic!()
        };
        assert_eq!(args[0].text, "owner", "the owner comes first, as in Rust");
    }

    #[test]
    fn assertions_become_the_rust_macros() {
        let f = parse(
            r#"
            it("t", async () => {
              assert.equal(a, b);
              expect(a).to.deep.equal(b);
              expect(a).not.to.equal(b);
              assert.isAbove(after, before);
              expect(after).to.be.greaterThan(before);
              expect(after).toBeLessThanOrEqual(cap);
              assert(after.gt(before), "gained");
              expect(done).to.be.true;
              assert.isFalse(paused);
              await expect(tx).to.be.rejectedWith("Unauthorized");
              assert.fail("unreachable");
              if (balance < 1) throw new Error("no gain");
            });
        "#,
        );
        let shapes: Vec<String> = test(&f, "t")
            .body
            .iter()
            .map(|s| match &s.kind {
                StmtKind::Expr(Expr {
                    kind: ExprKind::Call { callee, args, .. },
                    ..
                }) => {
                    let args: Vec<String> = args
                        .iter()
                        .map(|a| match &a.kind {
                            ExprKind::Binary(l, op, r) => format!("{} {op} {}", l.text, r.text),
                            ExprKind::Unary(op, x) => format!("{op}{}", x.text),
                            _ => a.text.clone(),
                        })
                        .collect();
                    format!("{}({})", callee.as_ident().unwrap_or("?"), args.join(", "))
                }
                other => format!("{other:?}"),
            })
            .collect();
        assert_eq!(
            shapes,
            vec![
                "assert_eq!(a, b)",
                "assert_eq!(a, b)",
                "assert_ne!(a, b)",
                "assert!(after > before)",
                "assert!(after > before)",
                "assert!(after <= cap)",
                "assert!(after > before, \"gained\")",
                "assert!(done)",
                "assert!(!paused)",
                "assert_that!(tx, \"Unauthorized\")",
                "assert_that!(\"unreachable\")",
                "assert!(!balance < 1)",
            ]
        );
    }

    #[test]
    fn helpers_classes_and_arrow_functions_are_functions() {
        let f = parse(
            r#"
            async function sendAs(ix, signer, opts = {}) { await provider.sendAndConfirm(ix, [signer]); }
            const fundAll = async (...kps) => { for (const kp of kps) await airdrop(kp); };
            class TestEnv {
              svm = new LiteSVM();
              static async create() { return new TestEnv(); }
              drainVault(amount) { this.svm.sendTransaction(tx); }
            }
            it("t", async () => {});
        "#,
        );
        let got: Vec<String> = f
            .functions
            .iter()
            .filter(|x| !x.test && !x.hook)
            .map(|x| {
                let owner = x.owner.as_deref().unwrap_or("-");
                let kind = if x.method { "method" } else { "fn" };
                let rest = if x.variadic { "+rest" } else { "" };
                format!(
                    "{owner} {kind} {}({}/{}{rest})",
                    x.name,
                    x.required,
                    x.params.len()
                )
            })
            .collect();
        assert_eq!(
            got,
            vec![
                "- fn send_as(2/3)",
                "- fn fund_all(0/0+rest)",
                "TestEnv fn create(0/0)",
                "TestEnv method drain_vault(1/1)",
                "TestEnv fn new(0/0)",
            ]
        );
    }

    #[test]
    fn setup_titles() {
        for t in [
            "is initialized",
            "initializes the pool",
            "creates a market",
            "sets up",
            "funds the vault",
            "airdrop payer",
        ] {
            assert!(is_setup_title(t), "{t}");
        }
        for t in [
            "drains the vault",
            "steals funds via reentrancy",
            "attacker initializes a fake pool",
        ] {
            assert!(!is_setup_title(t), "{t}");
        }
    }
}
