//! The execution trace of one test: what runs, in what order, and in which
//! phase.
//!
//! A Foundry test runs state-variable initializers and constructors, then
//! `setUp()`, then the test function. Helper functions the test calls are
//! inlined at the call site, so a prank inside `_attack()` is seen exactly
//! where it happens. Externally callable functions that are neither tests nor
//! `setUp` (flash-loan callbacks, `receive`, `fallback`) run when a protocol
//! calls back into the test contract; they form a third phase.

use crate::ir::{Expr, ExprKind, FnKind, Function, Pos, Stmt, StmtKind};
use crate::project::{Project, Suite, is_test_function};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Phase {
    /// Initializers, constructors and `setUp()`: the world the PoC builds.
    Setup,
    /// The test function and the helpers it calls: the exploit itself.
    Test,
    /// Callbacks into the test contract made during the exploit.
    Callback,
}

impl Phase {
    /// Whether this phase is part of the exploit rather than its setup.
    pub fn is_exploit(self) -> bool {
        matches!(self, Phase::Test | Phase::Callback)
    }
}

/// How a call is interpreted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallClass {
    /// A `vm.*` cheatcode.
    Cheat,
    /// A forge-std helper called without a receiver: `deal`, `hoax`, ...
    StdCheat,
    /// Any check that fails the test: `assert*`, `require`, `vm.expect*`.
    Assert,
    /// `console.log` and friends.
    Log,
    /// A language builtin or a library call: `keccak256`, `abi.encode`, ...
    Builtin,
    /// A type conversion: `address(x)`, `IERC20(x)`.
    Cast,
    /// A call to a function of the test contract. Resolved ones are inlined.
    Internal,
    /// A call to another contract.
    External,
    /// `new T(...)`.
    Deploy(String),
}

#[derive(Debug, Clone)]
pub struct Call<'p> {
    pub expr: &'p Expr,
    pub class: CallClass,
    pub receiver: Option<&'p Expr>,
    pub method: String,
    pub args: &'p [Expr],
}

#[derive(Debug, Clone)]
pub enum OpKind<'p> {
    Call(Call<'p>),
    /// `target = value`, from an assignment or a declaration.
    Assign {
        target: String,
        ty: Option<String>,
        value: Option<&'p Expr>,
    },
    /// `emit name(args)`.
    Emit {
        event: String,
        args: &'p [Expr],
    },
}

#[derive(Debug, Clone)]
pub struct Op<'p> {
    pub kind: OpKind<'p>,
    pub phase: Phase,
    /// Index of the file the op is written in.
    pub unit: usize,
    pub pos: Pos,
    /// The function the op is written in.
    pub function: String,
}

impl<'p> Op<'p> {
    pub fn call(&self) -> Option<&Call<'p>> {
        match &self.kind {
            OpKind::Call(c) => Some(c),
            _ => None,
        }
    }
}

/// Receivers whose member calls are cheatcodes. `vm` is the forge-std name;
/// the others appear in older harnesses that declare the interface themselves.
pub const CHEAT_RECEIVERS: &[&str] = &["vm", "cheats", "hevm", "cheat", "cheatcodes", "VM"];

/// forge-std helpers inherited from `Test` and called without a receiver.
pub const STD_CHEATS: &[&str] = &[
    "deal",
    "hoax",
    "startHoax",
    "changePrank",
    "makeAddr",
    "makeAddrAndKey",
    "makeAccount",
    "skip",
    "rewind",
    "bound",
    "deployCode",
    "deployCodeTo",
    "label",
];

const BUILTINS: &[&str] = &[
    "keccak256",
    "sha256",
    "ripemd160",
    "ecrecover",
    "addmod",
    "mulmod",
    "blockhash",
    "blobhash",
    "gasleft",
    "selfdestruct",
    "revert",
];

const BUILTIN_RECEIVERS: &[&str] = &["abi", "string", "bytes", "block", "msg", "tx"];

const LOG_RECEIVERS: &[&str] = &["console", "console2", "safeconsole"];

/// Names that fail the test when their condition does not hold.
pub fn is_assert_name(name: &str) -> bool {
    name.starts_with("assert")
        || name == "require"
        || name == "fail"
        || name.starts_with("expectRevert")
        || name.starts_with("expectPartialRevert")
        || name == "expectEmit"
        || name == "expectEmitAnonymous"
        || name == "expectCall"
}

/// The full ordered trace of one test.
#[derive(Debug)]
pub struct Trace<'p> {
    pub ops: Vec<Op<'p>>,
    pub test: &'p Function,
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

struct Builder<'p, 's> {
    project: &'p Project,
    suite: &'s Suite<'p>,
    ops: Vec<Op<'p>>,
    /// (linearization index, function name) of each function being walked.
    stack: Vec<(usize, String)>,
}

pub fn build<'p>(project: &'p Project, suite: &Suite<'p>, test: &'p Function) -> Trace<'p> {
    let mut b = Builder {
        project,
        suite,
        ops: Vec::new(),
        stack: Vec::new(),
    };

    // Most base first: initializers and constructors run base to derived.
    for (pos, located) in suite.linear.iter().enumerate().rev() {
        for var in &located.contract.vars {
            if let Some(init) = &var.init {
                b.visit(init, Phase::Setup, located.unit, "<state>");
            }
            b.ops.push(Op {
                kind: OpKind::Assign {
                    target: var.name.clone(),
                    ty: Some(var.ty.clone()),
                    value: var.init.as_ref(),
                },
                phase: Phase::Setup,
                unit: located.unit,
                pos: var.pos,
                function: "<state>".to_string(),
            });
        }
        if let Some(ctor) = located
            .contract
            .functions
            .iter()
            .find(|f| f.kind == FnKind::Constructor)
        {
            b.walk_function(pos, ctor, Phase::Setup);
        }
    }
    if let Some((pos, set_up)) = suite.function("setUp") {
        b.walk_function(pos, set_up, Phase::Setup);
    }
    if let Some((pos, _)) = suite.function(&test.name) {
        b.walk_function(pos, test, Phase::Test);
    }
    let mut seen: Vec<&str> = Vec::new();
    for (pos, located) in suite.linear.iter().enumerate() {
        for f in &located.contract.functions {
            let callback = matches!(f.kind, FnKind::Fallback | FnKind::Receive)
                || (f.kind == FnKind::Function
                    && f.is_externally_callable()
                    && !is_test_function(f)
                    && f.name != "setUp"
                    && !f.name.starts_with("invariant")
                    && !f.name.starts_with("test"));
            if callback && f.body.is_some() && !seen.contains(&f.name.as_str()) {
                seen.push(&f.name);
                b.walk_function(pos, f, Phase::Callback);
            }
        }
    }
    Trace { ops: b.ops, test }
}

impl<'p> Builder<'p, '_> {
    fn walk_function(&mut self, pos: usize, f: &'p Function, phase: Phase) {
        if self.stack.len() >= MAX_INLINE_DEPTH
            || self.stack.iter().any(|(p, n)| *p == pos && *n == f.name)
        {
            return;
        }
        self.walk_modified(pos, f, 0, phase);
    }

    /// Runs `f` wrapped in its modifiers from index `from` on. Each modifier's
    /// body runs in place, and its `_;` runs the rest of the chain, so a
    /// `balanceLog` modifier's before/after reads land around the test body.
    fn walk_modified(&mut self, pos: usize, f: &'p Function, from: usize, phase: Phase) {
        for (i, name) in f.modifiers.iter().enumerate().skip(from) {
            let Some((mpos, m)) = self.suite.function(name) else {
                continue;
            };
            if m.kind != FnKind::Modifier || self.stack.iter().any(|(p, n)| *p == mpos && n == name)
            {
                continue;
            }
            let Some(body) = &m.body else { continue };
            let unit = self.suite.linear[mpos].unit;
            self.stack.push((mpos, name.clone()));
            for stmt in body {
                if matches!(stmt.kind, StmtKind::Placeholder) {
                    self.walk_modified(pos, f, i + 1, phase);
                } else {
                    self.walk_stmt(stmt, phase, unit, name);
                }
            }
            self.stack.pop();
            return;
        }
        let Some(body) = &f.body else { return };
        let unit = self.suite.linear[pos].unit;
        self.stack.push((pos, f.name.clone()));
        for stmt in body {
            self.walk_stmt(stmt, phase, unit, &f.name);
        }
        self.stack.pop();
    }

    fn walk_stmt(&mut self, stmt: &'p Stmt, phase: Phase, unit: usize, function: &str) {
        match &stmt.kind {
            StmtKind::Expr(e) => self.visit(e, phase, unit, function),
            StmtKind::Decl { vars, init } => {
                if let Some(init) = init {
                    self.visit(init, phase, unit, function);
                }
                for (name, ty) in vars {
                    if name.is_empty() {
                        continue;
                    }
                    self.ops.push(Op {
                        kind: OpKind::Assign {
                            target: name.clone(),
                            ty: Some(ty.clone()),
                            value: init.as_ref(),
                        },
                        phase,
                        unit,
                        pos: stmt.pos,
                        function: function.to_string(),
                    });
                }
            }
            StmtKind::Emit { event, args } => {
                for a in args {
                    self.visit(a, phase, unit, function);
                }
                self.ops.push(Op {
                    kind: OpKind::Emit {
                        event: event.clone(),
                        args,
                    },
                    phase,
                    unit,
                    pos: stmt.pos,
                    function: function.to_string(),
                });
            }
            StmtKind::Return(Some(e)) => self.visit(e, phase, unit, function),
            StmtKind::Return(None)
            | StmtKind::Revert
            | StmtKind::Assembly
            | StmtKind::Placeholder => {}
        }
    }

    /// Post-order: an expression's sub-expressions run before it does.
    fn visit(&mut self, e: &'p Expr, phase: Phase, unit: usize, function: &str) {
        match &e.kind {
            ExprKind::Call { callee, args, .. } => {
                for child in callee.children() {
                    self.visit(child, phase, unit, function);
                }
                for a in args {
                    self.visit(a, phase, unit, function);
                }
                let (class, receiver, method, inline) = self.classify(callee);
                self.ops.push(Op {
                    kind: OpKind::Call(Call {
                        expr: e,
                        class,
                        receiver,
                        method,
                        args,
                    }),
                    phase,
                    unit,
                    pos: e.pos,
                    function: function.to_string(),
                });
                if let Some((pos, f)) = inline {
                    self.walk_function(pos, f, phase);
                }
            }
            ExprKind::Assign(lhs, rhs) => {
                self.visit(rhs, phase, unit, function);
                for child in lhs.children() {
                    self.visit(child, phase, unit, function);
                }
                self.ops.push(Op {
                    kind: OpKind::Assign {
                        target: crate::identity::key(lhs),
                        ty: None,
                        value: Some(rhs),
                    },
                    phase,
                    unit,
                    pos: e.pos,
                    function: function.to_string(),
                });
            }
            _ => {
                for child in e.children() {
                    self.visit(child, phase, unit, function);
                }
            }
        }
    }

    #[allow(clippy::type_complexity)]
    fn classify(
        &self,
        callee: &'p Expr,
    ) -> (
        CallClass,
        Option<&'p Expr>,
        String,
        Option<(usize, &'p Function)>,
    ) {
        let current = self.stack.last().map(|(p, _)| *p).unwrap_or(0);
        match &callee.kind {
            ExprKind::New(ty) => (CallClass::Deploy(ty.clone()), None, "new".to_string(), None),
            ExprKind::Type(ty) => (CallClass::Cast, None, ty.clone(), None),
            ExprKind::Ident(name) => {
                if let Some(found) = self.suite.function(name) {
                    return (CallClass::Internal, None, name.clone(), Some(found));
                }
                let class = if is_assert_name(name) || name == "assert" {
                    CallClass::Assert
                } else if STD_CHEATS.contains(&name.as_str()) {
                    CallClass::StdCheat
                } else if BUILTINS.contains(&name.as_str()) {
                    CallClass::Builtin
                } else if name.starts_with(|c: char| c.is_ascii_uppercase()) {
                    CallClass::Cast
                } else {
                    CallClass::Internal
                };
                (class, None, name.clone(), None)
            }
            ExprKind::Member(base, method) => {
                if let Some(r) = base.as_ident() {
                    if CHEAT_RECEIVERS.contains(&r) {
                        let class = if is_assert_name(method) {
                            CallClass::Assert
                        } else {
                            CallClass::Cheat
                        };
                        return (class, Some(base), method.clone(), None);
                    }
                    if LOG_RECEIVERS.contains(&r) {
                        return (CallClass::Log, Some(base), method.clone(), None);
                    }
                    if BUILTIN_RECEIVERS.contains(&r) || self.project.is_library(r) {
                        return (CallClass::Builtin, Some(base), method.clone(), None);
                    }
                    if r == "super" {
                        let found = self.suite.function_after(current, method);
                        return (CallClass::Internal, None, method.clone(), found);
                    }
                    if let Some(pos) = self.suite.position_of(r) {
                        let found = self.suite.linear[pos]
                            .contract
                            .function(method)
                            .map(|f| (pos, f));
                        return (CallClass::Internal, None, method.clone(), found);
                    }
                }
                if matches!(base.kind, ExprKind::Type(_)) {
                    return (CallClass::Builtin, Some(base), method.clone(), None);
                }
                (CallClass::External, Some(base), method.clone(), None)
            }
            _ => (CallClass::External, None, String::new(), None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trace_of(src: &str) -> Vec<String> {
        let p = Project::from_sources(&[("t.sol", src)]);
        let suites = p.suites();
        let s = &suites[0];
        let t = build(&p, s, s.tests[0]);
        t.ops
            .iter()
            .filter_map(|op| match &op.kind {
                OpKind::Call(c) => Some(format!("{:?}:{:?}:{}", op.phase, c.class, c.method)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn helpers_are_inlined_and_phases_assigned() {
        let ops = trace_of(
            r#"
            contract T {
                function setUp() public { vm.createSelectFork("x", 1); }
                function test_a() public { _go(); assertEq(1, 1); }
                function _go() internal { vm.prank(bob); pool.swap(1); }
                function onFlashLoan() external { deal(address(t), address(this), 1); }
            }
        "#,
        );
        assert_eq!(
            ops,
            vec![
                "Setup:Cheat:createSelectFork",
                "Test:Internal:_go",
                "Test:Cheat:prank",
                "Test:External:swap",
                "Test:Assert:assertEq",
                "Callback:Cast:address",
                "Callback:Cast:address",
                "Callback:StdCheat:deal",
            ]
        );
    }

    #[test]
    fn nested_calls_run_before_the_outer_call() {
        let ops = trace_of(
            "contract T { function test_a() public { vault.withdraw(token.balanceOf(me)); } }",
        );
        assert_eq!(
            ops,
            vec!["Test:External:balanceOf", "Test:External:withdraw"]
        );
    }

    #[test]
    fn super_calls_resolve_to_the_base_definition() {
        let p = Project::from_sources(&[(
            "t.sol",
            r#"
            abstract contract B { function setUp() public virtual { vm.store(a, b, c); } }
            contract T is B {
                function setUp() public override { super.setUp(); }
                function test_a() public {}
            }
        "#,
        )]);
        let suites = p.suites();
        let t = build(&p, &suites[0], suites[0].tests[0]);
        assert!(
            t.ops
                .iter()
                .any(|op| matches!(&op.kind, OpKind::Call(c) if c.method == "store"))
        );
    }

    #[test]
    fn modifiers_wrap_the_test_body() {
        let ops = trace_of(
            r#"
            contract T {
                modifier balanceLog() { a.balanceOf(me); _; a.balanceOf(me); }
                modifier other() { emitIt(); _; }
                function test_a() public balanceLog other { pool.swap(1); }
            }
        "#,
        );
        assert_eq!(
            ops,
            vec![
                "Test:External:balanceOf",
                "Test:Internal:emitIt",
                "Test:External:swap",
                "Test:External:balanceOf",
            ]
        );
    }

    #[test]
    fn recursion_does_not_loop() {
        let ops = trace_of(
            "contract T { function test_a() public { f(); } function f() internal { f(); } }",
        );
        assert_eq!(ops, vec!["Test:Internal:f", "Test:Internal:f"]);
    }
}
