//! The execution trace of one Solana test: every call it makes, in order,
//! with the helpers it calls inlined at the call site.
//!
//! A Rust test has no `setUp()`. The world is built by the test itself or by
//! helpers it calls (`setup()`, `TestEnv::new()`), so the trace is one flat
//! sequence, and the rules decide later where the exploit begins. A
//! TypeScript test is preceded by what mocha runs first (its `describe`
//! bodies, `before` hooks and the tests above it), which the trace walks
//! before the test. Each op records whether it runs inside a helper or hook
//! that builds the world.
//!
//! When a helper is inlined, its parameters are bound to the caller's
//! arguments: inside `fn send(svm: &mut LiteSVM, ix: Instruction, signer:
//! &Keypair)`, `signer` stands for whatever the call site passed, so a
//! transaction signed by `signer` is seen as signed by `admin` when the test
//! calls `send(&mut svm, ix, &admin)`.

use super::harness;
use super::ir::{Dialect, Function};
use crate::identity::{compact, words};
use crate::ir::{Expr, ExprKind, Pos, Stmt, StmtKind};
use crate::project::Project;
use std::collections::BTreeMap;
use std::rc::Rc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallClass {
    /// Anything that fails the test when its condition does not hold:
    /// `assert!`, `assert_eq!`, `.unwrap_err()`, a validating Mollusk run, an
    /// `assert_*` helper method (`svm.assert_token_balance(..)`), or a
    /// TypeScript assertion (lowered to the same macros).
    Assert,
    /// `println!`, `dbg!`, `msg!`, `log::info!`.
    Log,
    /// A function of the test's own code. Resolved ones are inlined.
    Internal,
    /// Everything else: harness calls, library calls, constructors.
    Other,
}

#[derive(Debug, Clone)]
pub struct Call<'p> {
    pub expr: &'p Expr,
    pub class: CallClass,
    pub receiver: Option<&'p Expr>,
    /// The method name, the last segment of the called path, or the macro
    /// name without `!`.
    pub name: String,
    /// The called path for a path call (`Transaction::new_signed_with_payer`);
    /// empty for a method call.
    pub path: String,
    pub args: &'p [Expr],
    /// The fields of a struct literal.
    pub fields: &'p [(String, Expr)],
}

#[derive(Debug, Clone)]
pub enum OpKind<'p> {
    Call(Call<'p>),
    /// `let target = value`, `target = value`, or a helper's parameter bound
    /// to its argument.
    Assign {
        target: String,
        value: Option<&'p Expr>,
    },
}

/// What an inlined helper's parameter stands for: the argument the caller
/// passed, read in the caller's own scope.
#[derive(Debug)]
pub struct Binding<'p> {
    pub arg: &'p Expr,
    pub scope: Rc<Scope<'p>>,
}

/// Parameter name -> its binding, for the helper being walked.
pub type Scope<'p> = BTreeMap<String, Binding<'p>>;

/// Follows a parameter reference to the caller's argument, as many levels up
/// as it was passed through: `signer` -> `&ctx.admin`.
fn resolve<'p>(scope: &Rc<Scope<'p>>, e: &'p Expr) -> (&'p Expr, Rc<Scope<'p>>) {
    if is_identity_shaped(e)
        && let Some(b) = scope.get(&key(e))
    {
        return resolve(&b.scope, b.arg);
    }
    (e, Rc::clone(scope))
}

/// The elements of a list argument, each with the scope it is read in:
/// `&[&payer, &attacker]`, `vec![..]`, or a parameter bound to one of those.
fn items<'p>(scope: &Rc<Scope<'p>>, e: &'p Expr, out: &mut Vec<(&'p Expr, Rc<Scope<'p>>)>) {
    let (e, scope) = resolve(scope, e);
    match &e.kind {
        ExprKind::Unary(op, inner) if op == "&" => items(&scope, inner, out),
        ExprKind::Array(xs) | ExprKind::Tuple(xs) => xs.iter().for_each(|x| items(&scope, x, out)),
        ExprKind::Call { callee, args, .. } if callee.as_ident() == Some("vec!") => {
            args.iter().for_each(|x| items(&scope, x, out))
        }
        _ => out.push((e, scope)),
    }
}

/// Where a helper was called from: the call expression and its file.
#[derive(Debug, Clone, Copy)]
pub struct Site<'p> {
    pub unit: usize,
    pub expr: &'p Expr,
}

#[derive(Debug, Clone)]
pub struct Op<'p> {
    pub kind: OpKind<'p>,
    /// Index of the file the op is written in.
    pub unit: usize,
    pub pos: Pos,
    /// Whether the op runs inside a helper whose name says it builds the
    /// world: `setup`, `init_pool`, `TestEnv::new`.
    pub in_setup_helper: bool,
    /// The call that entered the function this op is written in, when that
    /// is a helper. A transaction is usually built inside a generic `send`
    /// wrapper, so the call site is where the reader sees what it does.
    pub site: Option<Site<'p>>,
    scope: Rc<Scope<'p>>,
}

impl<'p> Op<'p> {
    pub fn call(&self) -> Option<&Call<'p>> {
        match &self.kind {
            OpKind::Call(c) => Some(c),
            OpKind::Assign { .. } => None,
        }
    }

    /// The identity an expression stands for at this op, with the enclosing
    /// helpers' parameters replaced by what their callers passed.
    pub fn key(&self, e: &'p Expr) -> String {
        let (e, _) = resolve(&self.scope, e);
        key(e)
    }

    /// The identity a bare name stands for at this op: the caller's argument
    /// for a helper's parameter, else the name itself.
    pub fn name_key(&self, name: &str) -> String {
        match self.scope.get(name) {
            Some(b) => key(resolve(&b.scope, b.arg).0),
            None => name.to_string(),
        }
    }

    /// The expression `e` stands for at this op: a helper's parameter is the
    /// argument its caller passed.
    pub fn resolved(&self, e: &'p Expr) -> &'p Expr {
        resolve(&self.scope, e).0
    }

    /// The identities in a list argument, such as a transaction's signers.
    pub fn keys(&self, list: &'p Expr) -> Vec<String> {
        let mut out = Vec::new();
        items(&self.scope, list, &mut out);
        out.into_iter()
            .map(|(e, scope)| key(resolve(&scope, e).0))
            .collect()
    }

    /// The expressions in a list argument, such as a transaction's
    /// instructions, with parameters replaced by the caller's arguments.
    pub fn exprs(&self, list: &'p Expr) -> Vec<&'p Expr> {
        let mut out = Vec::new();
        items(&self.scope, list, &mut out);
        out.into_iter().map(|(e, _)| e).collect()
    }
}

/// Methods that return the identity of their receiver: `kp.pubkey()`,
/// `kp.insecure_clone()`, `acc.key()`.
const IDENTITY_METHODS: &[&str] = &[
    "pubkey",
    "key",
    "clone",
    "insecure_clone",
    "to_owned",
    "as_ref",
    "borrow",
    "deref",
];

/// The canonical identity of a keypair- or address-valued expression.
/// References, clones and `.pubkey()` are peeled, `Some(x)` is `x`, and a
/// field is named by its last segment: `&ctx.payer.pubkey()` is `payer`.
pub fn key(e: &Expr) -> String {
    match &e.kind {
        ExprKind::Ident(path) => path.clone(),
        ExprKind::Unary(op, inner) if op == "&" || op == "*" => key(inner),
        ExprKind::Member(_, field) => field.clone(),
        ExprKind::Call { callee, args, .. } => match (&callee.kind, args.len()) {
            (ExprKind::Member(base, m), 0) if IDENTITY_METHODS.contains(&m.as_str()) => key(base),
            (ExprKind::Ident(f), 1) if f == "Some" => key(&args[0]),
            _ => compact(&e.text),
        },
        _ => compact(&e.text),
    }
}

/// Whether an expression is a plain reference to an identity, as opposed to
/// a computation: `admin`, `&ctx.payer`, `attacker.pubkey()`.
pub fn is_identity_shaped(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Ident(_) | ExprKind::Member(..) => true,
        ExprKind::Unary(op, inner) if op == "&" || op == "*" => is_identity_shaped(inner),
        ExprKind::Call { callee, args, .. } => match (&callee.kind, args.len()) {
            (ExprKind::Member(base, m), 0) if IDENTITY_METHODS.contains(&m.as_str()) => {
                is_identity_shaped(base)
            }
            (ExprKind::Ident(f), 1) if f == "Some" => is_identity_shaped(&args[0]),
            _ => false,
        },
        _ => false,
    }
}

/// Words that say a helper builds the world rather than attacks it.
const SETUP_WORDS: &[&str] = &[
    "setup",
    "init",
    "initialize",
    "initialise",
    "fixture",
    "deploy",
    "bootstrap",
    "prepare",
    "create",
    "new",
    "fund",
    "configure",
    "genesis",
];

/// Whether a function name says it builds the world: `setup`, `set_up_pool`,
/// `init_market`, `TestEnv::new`.
pub fn is_setup_name(name: &str) -> bool {
    let ws = words(name);
    ws.iter().any(|w| SETUP_WORDS.contains(&w.as_str()))
        || ws.windows(2).any(|p| p[0] == "set" && p[1] == "up")
}

/// An assertion helper method: `svm.assert_token_balance(&ata, 100)`.
/// `assert_success()` only checks that a transaction did not fail, like
/// `.unwrap()`, and is not one.
fn is_assert_method(name: &str) -> bool {
    name.starts_with("assert_") && name != "assert_success"
}

fn is_assert_macro(name: &str) -> bool {
    let n = name.strip_prefix("debug_").unwrap_or(name);
    let n = n.strip_prefix("prop_").unwrap_or(n);
    n.starts_with("assert")
}

const LOG_MACROS: &[&str] = &[
    "println", "print", "eprintln", "eprint", "dbg", "msg", "info", "debug", "trace", "warn",
    "error",
];

/// The full ordered trace of one test.
#[derive(Debug)]
pub struct Trace<'p> {
    pub ops: Vec<Op<'p>>,
    pub test: &'p Function,
    /// The file the test is written in.
    pub unit: usize,
    pub dialect: Dialect,
}

impl<'p> Trace<'p> {
    pub fn calls(&self) -> impl Iterator<Item = (usize, &Op<'p>, &Call<'p>)> {
        self.ops
            .iter()
            .enumerate()
            .filter_map(|(i, op)| op.call().map(|c| (i, op, c)))
    }
}

const MAX_INLINE_DEPTH: usize = 8;

/// The functions a test can call: those in its own file first, then those in
/// shared test code in the same language (any file under a `tests` or `test`
/// directory, such as `tests/common/mod.rs` or `tests/utils.ts`). Program
/// source is never inlined: it is the code under review, and a test reaches it
/// through transactions.
pub struct Scopes<'p> {
    project: &'p Project,
    shared: Vec<usize>,
}

impl<'p> Scopes<'p> {
    pub fn new(project: &'p Project) -> Scopes<'p> {
        let shared = project
            .units
            .iter()
            .enumerate()
            .filter(|(_, u)| {
                u.solana.is_some()
                    && u.path
                        .split('/')
                        .rev()
                        .skip(1)
                        .any(|c| c == "tests" || c == "test")
            })
            .map(|(i, _)| i)
            .collect();
        Scopes { project, shared }
    }

    /// The first function matching `pred`, searching `from` and then the
    /// shared test files.
    fn find(&self, from: usize, pred: impl Fn(&Function) -> bool) -> Option<(usize, &'p Function)> {
        let dialect = self.dialect(from);
        std::iter::once(from)
            .chain(self.shared.iter().copied().filter(|&u| u != from))
            .find_map(|u| {
                let file = self.project.units[u].solana.as_ref()?;
                if file.dialect != dialect {
                    return None;
                }
                file.functions
                    .iter()
                    .find(|f| !f.test && !f.hook && pred(f))
                    .map(|f| (u, f))
            })
    }

    fn dialect(&self, unit: usize) -> Dialect {
        self.project.units[unit]
            .solana
            .as_ref()
            .map(|f| f.dialect)
            .unwrap_or_default()
    }
}

struct Frame<'p> {
    unit: usize,
    function: &'p Function,
    scope: Rc<Scope<'p>>,
    setup: bool,
    /// The call that entered this function; `None` for the test itself.
    site: Option<Site<'p>>,
}

struct Builder<'p, 's> {
    scopes: &'s Scopes<'p>,
    ops: Vec<Op<'p>>,
    stack: Vec<Frame<'p>>,
}

pub fn build<'p>(scopes: &Scopes<'p>, unit: usize, test: &'p Function) -> Trace<'p> {
    let mut b = Builder {
        scopes,
        ops: Vec::new(),
        stack: Vec::new(),
    };
    if let Some(file) = &scopes.project.units[unit].solana {
        for prior in &test.before {
            let f = &file.functions[prior.function];
            b.walk(unit, f, Scope::new(), prior.setup, None);
        }
    }
    b.walk(unit, test, Scope::new(), false, None);
    Trace {
        ops: b.ops,
        test,
        unit,
        dialect: scopes.dialect(unit),
    }
}

impl<'p> Builder<'p, '_> {
    fn frame(&self) -> &Frame<'p> {
        self.stack.last().expect("a function is being walked")
    }

    fn walk(
        &mut self,
        unit: usize,
        f: &'p Function,
        scope: Scope<'p>,
        setup: bool,
        site: Option<Site<'p>>,
    ) {
        if self.stack.len() >= MAX_INLINE_DEPTH
            || self
                .stack
                .iter()
                .any(|fr| fr.unit == unit && std::ptr::eq(fr.function, f))
        {
            return;
        }
        self.stack.push(Frame {
            unit,
            function: f,
            scope: Rc::new(scope),
            setup,
            site,
        });
        for stmt in &f.body {
            self.stmt(stmt);
        }
        self.stack.pop();
    }

    fn push(&mut self, kind: OpKind<'p>, pos: Pos) {
        let fr = self.frame();
        let op = Op {
            kind,
            unit: fr.unit,
            pos,
            in_setup_helper: fr.setup,
            site: fr.site,
            scope: Rc::clone(&fr.scope),
        };
        self.ops.push(op);
    }

    fn stmt(&mut self, stmt: &'p Stmt) {
        match &stmt.kind {
            StmtKind::Expr(e) | StmtKind::Return(Some(e)) => self.visit(e),
            StmtKind::Decl { vars, init } => {
                if let Some(init) = init {
                    self.visit(init);
                }
                for (name, _) in vars {
                    if name.is_empty() || name == "_" {
                        continue;
                    }
                    self.push(
                        OpKind::Assign {
                            target: name.clone(),
                            value: init.as_ref(),
                        },
                        stmt.pos,
                    );
                }
            }
            _ => {}
        }
    }

    /// Post-order: an expression's sub-expressions run before it does.
    fn visit(&mut self, e: &'p Expr) {
        match &e.kind {
            ExprKind::Call {
                callee,
                args,
                options,
            } => {
                for child in callee.children() {
                    self.visit(child);
                }
                for (_, v) in options {
                    self.visit(v);
                }
                for a in args {
                    self.visit(a);
                }
                let call = self.classify(e, callee, args, options);
                let inline = (call.class == CallClass::Internal)
                    .then(|| self.resolve(callee, args.len()))
                    .flatten();
                let pos = e.pos;
                self.push(OpKind::Call(call), pos);
                if let Some((unit, f)) = inline {
                    self.inline(unit, f, args, e);
                }
            }
            ExprKind::Assign(lhs, rhs) => {
                self.visit(rhs);
                for child in lhs.children() {
                    self.visit(child);
                }
                self.push(
                    OpKind::Assign {
                        target: key(lhs),
                        value: Some(rhs),
                    },
                    e.pos,
                );
            }
            _ => {
                for child in e.children() {
                    self.visit(child);
                }
            }
        }
    }

    /// Walks a helper's body with its parameters bound to the call's
    /// arguments. A method's receiver is `self`, which is not bound: fields
    /// are named by their last segment (`self.admin` is `admin`) wherever
    /// they are read.
    fn inline(&mut self, unit: usize, f: &'p Function, args: &'p [Expr], call: &'p Expr) {
        let fr = self.frame();
        let site = Site {
            unit: fr.unit,
            expr: call,
        };
        let scope: Scope<'p> = f
            .params
            .iter()
            .zip(args)
            .filter(|(param, _)| !param.is_empty())
            .map(|(param, arg)| {
                let binding = Binding {
                    arg,
                    scope: Rc::clone(&fr.scope),
                };
                (param.clone(), binding)
            })
            .collect();
        let setup = fr.setup || is_setup_name(&f.name);
        self.walk(unit, f, scope, setup, Some(site));
    }

    fn classify(
        &self,
        e: &'p Expr,
        callee: &'p Expr,
        args: &'p [Expr],
        fields: &'p [(String, Expr)],
    ) -> Call<'p> {
        let mut call = Call {
            expr: e,
            class: CallClass::Other,
            receiver: None,
            name: String::new(),
            path: String::new(),
            args,
            fields,
        };
        match &callee.kind {
            ExprKind::Ident(path) if path.ends_with('!') => {
                let name = path.trim_end_matches('!');
                call.name = name.to_string();
                call.class = if is_assert_macro(name) {
                    CallClass::Assert
                } else if LOG_MACROS.contains(&name) {
                    CallClass::Log
                } else {
                    CallClass::Other
                };
            }
            ExprKind::Ident(path) => {
                call.name = path.rsplit("::").next().unwrap_or(path).to_string();
                call.path = path.clone();
                if self.resolve(callee, args.len()).is_some() {
                    call.class = CallClass::Internal;
                }
            }
            ExprKind::Type(path) => {
                call.name = path.rsplit("::").next().unwrap_or(path).to_string();
                call.path = path.clone();
            }
            ExprKind::Member(receiver, method) => {
                call.name = method.clone();
                call.receiver = Some(receiver);
                call.class = if matches!(method.as_str(), "unwrap_err" | "expect_err")
                    || harness::is_validating_run(method)
                    || is_assert_method(method)
                {
                    CallClass::Assert
                } else if self.resolve(callee, args.len()).is_some() {
                    CallClass::Internal
                } else {
                    CallClass::Other
                };
            }
            _ => {}
        }
        call
    }

    /// The test-code function a call goes to, if any. A function that cannot
    /// take the call's number of arguments is not the one called:
    /// `liquidity.borrow(amount)` is not a fixture's
    /// `borrow(&self, banks, payer, amount)`.
    fn resolve(&self, callee: &Expr, nargs: usize) -> Option<(usize, &'p Function)> {
        let from = self.frame().unit;
        let current_owner = self.frame().function.owner.clone();
        match &callee.kind {
            ExprKind::Ident(path) if !path.ends_with('!') => {
                let segs: Vec<&str> = path.split("::").collect();
                let name = *segs.last()?;
                let owner = match segs.len() {
                    1 => None,
                    _ => {
                        let prev = segs[segs.len() - 2];
                        if prev == "Self" {
                            current_owner
                        } else if prev.starts_with(|c: char| c.is_ascii_uppercase()) {
                            Some(prev.to_string())
                        } else {
                            None
                        }
                    }
                };
                self.scopes.find(from, |f| {
                    f.name == name && f.owner == owner && !f.method && f.accepts(nargs)
                })
            }
            ExprKind::Member(receiver, method) => {
                let on_self = receiver.as_ident() == Some("self");
                if !on_self && harness::METHODS.contains(&method.as_str()) {
                    return None;
                }
                self.scopes.find(from, |f| {
                    f.method
                        && f.name == *method
                        && f.accepts(nargs)
                        && (!on_self || current_owner.is_none() || f.owner == current_owner)
                })
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(sources: &[(&str, &str)]) -> Project {
        Project::from_sources(sources)
    }

    fn trace_names(p: &Project, unit: usize) -> Vec<String> {
        let scopes = Scopes::new(p);
        let file = p.units[unit].solana.as_ref().unwrap();
        let test = file.functions.iter().find(|f| f.test).unwrap();
        let t = build(&scopes, unit, test);
        t.ops
            .iter()
            .filter_map(|op| {
                op.call().map(|c| {
                    let setup = if op.in_setup_helper { "setup:" } else { "" };
                    format!("{setup}{:?}:{}", c.class, c.name)
                })
            })
            .collect()
    }

    #[test]
    fn helpers_are_inlined_with_their_setup_flag() {
        let p = project(&[(
            "tests/t.rs",
            r#"
            fn setup() -> LiteSVM { let svm = LiteSVM::new(); svm }
            fn attack(svm: &mut LiteSVM) { svm.send_transaction(tx); }
            #[test]
            fn test_a() {
                let mut svm = setup();
                attack(&mut svm);
                assert!(svm.get_balance(&a).unwrap() > 0);
            }
        "#,
        )]);
        assert_eq!(
            trace_names(&p, 0),
            vec![
                "Internal:setup",
                "setup:Other:new",
                "Internal:attack",
                "Other:send_transaction",
                "Other:get_balance",
                "Other:unwrap",
                "Assert:assert",
            ]
        );
    }

    #[test]
    fn parameters_stand_for_the_callers_arguments() {
        let p = project(&[(
            "tests/t.rs",
            r#"
            fn send(svm: &mut LiteSVM, ix: Instruction, signer: &Keypair) {
                let tx = Transaction::new_signed_with_payer(&[ix], Some(&signer.pubkey()), &[signer], h);
            }
            #[test]
            fn test_a() { send(&mut svm, ix, &ctx.admin); }
        "#,
        )]);
        let scopes = Scopes::new(&p);
        let file = p.units[0].solana.as_ref().unwrap();
        let test = file.functions.iter().find(|f| f.test).unwrap();
        let t = build(&scopes, 0, test);
        let (_, op, c) = t
            .calls()
            .find(|(_, _, c)| c.name == "new_signed_with_payer")
            .unwrap();
        let Some(harness::Event::Transaction { signers, .. }) = harness::event(c) else {
            panic!("not a transaction");
        };
        assert_eq!(op.keys(signers[0]), vec!["admin"]);
    }

    #[test]
    fn methods_and_shared_test_files_resolve_but_program_code_does_not() {
        let p = project(&[
            (
                "programs/vault/src/lib.rs",
                "pub fn deposit() { msg!(\"x\"); }",
            ),
            (
                "tests/common/mod.rs",
                "pub struct Env; impl Env { pub async fn new() -> Env { Env } pub fn drain(&mut self) { self.svm.send_transaction(tx); } }",
            ),
            (
                "tests/exploit.rs",
                "#[tokio::test] async fn test_x() { let mut env = Env::new().await; env.drain(); deposit(); }",
            ),
        ]);
        assert_eq!(
            trace_names(&p, 2),
            vec![
                "Internal:new",
                "Internal:drain",
                "Other:send_transaction",
                "Other:deposit",
            ]
        );
    }

    #[test]
    fn keys_peel_references_clones_and_fields() {
        let p = project(&[(
            "t.rs",
            "fn f() { g(&ctx.payer.pubkey(), Some(&attacker.pubkey()), kp.insecure_clone(), spl_token::ID); }",
        )]);
        let file = p.units[0].solana.as_ref().unwrap();
        let StmtKind::Expr(e) = &file.functions[0].body[0].kind else {
            panic!()
        };
        let ExprKind::Call { args, .. } = &e.kind else {
            panic!()
        };
        let keys: Vec<String> = args.iter().map(key).collect();
        assert_eq!(keys, vec!["payer", "attacker", "kp", "spl_token::ID"]);
    }

    #[test]
    fn setup_names() {
        for name in [
            "setup",
            "set_up",
            "init_pool",
            "create_market",
            "new",
            "setup_env",
        ] {
            assert!(is_setup_name(name), "{name}");
        }
        for name in ["attack", "drain_vault", "send_tx", "upset"] {
            assert!(!is_setup_name(name), "{name}");
        }
    }

    #[test]
    fn recursion_does_not_loop() {
        let p = project(&[("t.rs", "fn f() { f(); } #[test] fn test_a() { f(); }")]);
        assert_eq!(trace_names(&p, 0), vec!["Internal:f", "Internal:f"]);
    }
}
