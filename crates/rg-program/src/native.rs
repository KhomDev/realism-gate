//! Native Solana: instruction handlers and their `AccountInfo` checks.
//!
//! A handler is a function that takes `AccountInfo`s; each handler is an entry
//! point in its own right, so a dispatcher and the per-instruction functions
//! it calls are reported separately rather than merged into one answer. A
//! function that takes accounts, is called by another handler and does not
//! return a result is an accessor (`get_account(accounts, 0)`), not an
//! instruction, and is left out. The checks are read from the handler's own
//! body: `is_signer` demands, owner comparisons and key comparisons. The rules
//! are documented on [`crate`].

use crate::synutil::{SourceIndex, short};
use crate::{
    AccountConstraint, Authority, ConstraintKind, EntryPoint, EntryPointKind, Language, Program,
    Provenance, Span,
};
use std::collections::BTreeMap;
use syn::parse::Parser as _;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{BinOp, Expr, FnArg, ImplItem, Item, Stmt, Token, UnOp};

pub(crate) fn collect(files: &[(String, String)]) -> (Vec<Program>, Vec<String>) {
    let mut notes = Vec::new();
    let mut handlers: Vec<Handler> = Vec::new();
    for (file, source) in files {
        if crate::anchor::is_anchor_source(source) {
            continue;
        }
        match syn::parse_file(source) {
            Ok(ast) => collect_handlers(&ast.items, file, source, &mut handlers),
            Err(error) => notes.push(format!(
                "{file}: did not parse: line {}, column {}: {error}",
                error.span().start().line,
                error.span().start().column + 1
            )),
        }
    }

    let mut by_program: BTreeMap<String, Vec<EntryPoint>> = BTreeMap::new();
    let called: Vec<std::collections::BTreeSet<String>> = handlers
        .iter()
        .map(|handler| called_names(&handler.body))
        .collect();
    for (index, handler) in handlers.iter().enumerate() {
        if !handler.returns_result
            && handlers.iter().enumerate().any(|(other, caller)| {
                other != index
                    && caller.program == handler.program
                    && called[other].contains(&handler.name)
            })
        {
            // `get_account(accounts, 0)` takes accounts and is called by a
            // handler, but it is an accessor, not an instruction.
            continue;
        }
        by_program
            .entry(handler.program.clone())
            .or_default()
            .push(analyse(handler));
    }
    let programs = by_program
        .into_iter()
        .map(|(name, entry_points)| Program {
            name,
            id: None,
            language: Language::NativeSolana,
            entry_points,
        })
        .collect();
    (programs, notes)
}

struct Handler {
    name: String,
    program: String,
    span: Span,
    body: Vec<OwnedStmt>,
    /// Whether the function returns `ProgramResult`/`Result`. An accessor that
    /// returns an account is not an instruction handler.
    returns_result: bool,
}

/// A statement, lowered to what the check analysis reads. `syn`'s AST is
/// owned, but lowering keeps the analysis independent of parser types and
/// makes the source text of every expression available where it was written.
enum OwnedStmt {
    Expr(OwnedExpr),
    /// `require!(cond, ...)` / `assert!(cond, ...)`, either as a macro or as a
    /// function call.
    Require(OwnedExpr),
    If {
        condition: OwnedExpr,
        then_branch: Vec<OwnedStmt>,
        else_branch: Vec<OwnedStmt>,
        then_rejects: bool,
        else_rejects: bool,
    },
    Block(Vec<OwnedStmt>),
    Match(Vec<Vec<OwnedStmt>>),
}

struct OwnedExpr {
    kind: ExprKind,
    text: String,
    span: Span,
}

enum ExprKind {
    /// A path; the last segment's name.
    Path(String),
    /// `base.name`
    Field {
        base: Box<OwnedExpr>,
        name: String,
    },
    /// `receiver.method(args)`
    Method {
        receiver: Box<OwnedExpr>,
        name: String,
        args: Vec<OwnedExpr>,
    },
    /// `callee(args)`
    Call {
        callee: Box<OwnedExpr>,
        args: Vec<OwnedExpr>,
    },
    Binary {
        left: Box<OwnedExpr>,
        op: BinOpKind,
        right: Box<OwnedExpr>,
    },
    Not(Box<OwnedExpr>),
    Bool(bool),
    /// `base[index]`, named by its source text; the element is kept for the
    /// lowering, not read by the analysis.
    Index,
    /// Anything else, with its source text.
    Other,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BinOpKind {
    And,
    Or,
    Eq,
    Ne,
    Other,
}

fn collect_handlers(items: &[Item], file: &str, source: &str, out: &mut Vec<Handler>) {
    let index = SourceIndex::new(source);
    for item in items {
        match item {
            Item::Fn(function) if takes_account_info(&function.sig, &index) => {
                out.push(lower_handler(
                    &function.sig,
                    &function.block,
                    file,
                    source,
                    &index,
                ));
            }
            Item::Impl(implementation) => {
                for impl_item in &implementation.items {
                    if let ImplItem::Fn(function) = impl_item
                        && takes_account_info(&function.sig, &index)
                    {
                        out.push(lower_handler(
                            &function.sig,
                            &function.block,
                            file,
                            source,
                            &index,
                        ));
                    }
                }
            }
            _ => {}
        }
    }
}

fn lower_handler(
    signature: &syn::Signature,
    block: &syn::Block,
    file: &str,
    source: &str,
    index: &SourceIndex,
) -> Handler {
    let returns_result = match &signature.output {
        syn::ReturnType::Default => false,
        syn::ReturnType::Type(_, ty) => index.text(ty.span()).contains("Result"),
    };
    Handler {
        name: signature.ident.to_string(),
        program: program_name(file),
        span: index.spanned(file, signature),
        body: lower_block(block, file, source, index),
        returns_result,
    }
}

/// Direct calls in a body, by callee name. Used only to tell an accessor from
/// an instruction.
fn called_names(body: &[OwnedStmt]) -> std::collections::BTreeSet<String> {
    let mut out = std::collections::BTreeSet::new();
    for statement in body {
        statement_calls(statement, &mut out);
    }
    out
}

fn statement_calls(statement: &OwnedStmt, out: &mut std::collections::BTreeSet<String>) {
    match statement {
        OwnedStmt::Expr(expression) | OwnedStmt::Require(expression) => {
            expression_calls(expression, out);
        }
        OwnedStmt::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            expression_calls(condition, out);
            for nested in then_branch.iter().chain(else_branch) {
                statement_calls(nested, out);
            }
        }
        OwnedStmt::Block(statements) => {
            for nested in statements {
                statement_calls(nested, out);
            }
        }
        OwnedStmt::Match(arms) => {
            for arm in arms {
                for nested in arm {
                    statement_calls(nested, out);
                }
            }
        }
    }
}

fn expression_calls(expression: &OwnedExpr, out: &mut std::collections::BTreeSet<String>) {
    match &expression.kind {
        ExprKind::Path(_) => {}
        ExprKind::Field { base, .. } => expression_calls(base, out),
        ExprKind::Method {
            receiver,
            name,
            args,
        } => {
            out.insert(name.clone());
            expression_calls(receiver, out);
            for argument in args {
                expression_calls(argument, out);
            }
        }
        ExprKind::Call { callee, args } => {
            if let ExprKind::Path(name) = &callee.kind {
                out.insert(name.clone());
            }
            for argument in args {
                expression_calls(argument, out);
            }
        }
        ExprKind::Binary { left, right, .. } => {
            expression_calls(left, out);
            expression_calls(right, out);
        }
        ExprKind::Not(inner) => expression_calls(inner, out),
        ExprKind::Bool(_) | ExprKind::Index | ExprKind::Other => {}
    }
}

fn lower_block(
    block: &syn::Block,
    file: &str,
    source: &str,
    index: &SourceIndex,
) -> Vec<OwnedStmt> {
    block
        .stmts
        .iter()
        .map(|statement| lower_statement(statement, file, source, index))
        .collect()
}

/// The source and index are threaded so that lowering stays a pure function
/// of the parsed file; nested blocks are lowered with the same source.
fn lower_statement(statement: &Stmt, file: &str, source: &str, index: &SourceIndex) -> OwnedStmt {
    match statement {
        Stmt::Expr(expression, _) => match expression {
            Expr::If(if_expression) => {
                let then_branch = lower_block(&if_expression.then_branch, file, source, index);
                let (else_branch, else_rejects) = match &if_expression.else_branch {
                    Some((_, otherwise)) => (
                        lower_expression_as_statements(otherwise, file, source, index),
                        expression_rejects(otherwise),
                    ),
                    None => (Vec::new(), false),
                };
                OwnedStmt::If {
                    condition: lower_expression(&if_expression.cond, file, index),
                    then_rejects: block_rejects(&then_branch),
                    else_rejects,
                    then_branch,
                    else_branch,
                }
            }
            Expr::Block(block) => OwnedStmt::Block(lower_block(&block.block, file, source, index)),
            Expr::Match(match_expression) => OwnedStmt::Match(
                match_expression
                    .arms
                    .iter()
                    .map(|arm| lower_expression_as_statements(&arm.body, file, source, index))
                    .collect(),
            ),
            Expr::Macro(mac) if is_require_macro(&mac.mac) => {
                let condition = first_macro_argument(&mac.mac)
                    .map(|argument| lower_expression(&argument, file, index));
                match condition {
                    Some(condition) => OwnedStmt::Require(condition),
                    None => OwnedStmt::Require(OwnedExpr {
                        kind: ExprKind::Other,
                        text: short(&index.text(mac.span())),
                        span: index.spanned(file, mac),
                    }),
                }
            }
            _ => OwnedStmt::Expr(lower_expression(expression, file, index)),
        },
        Stmt::Macro(mac) if is_require_macro(&mac.mac) => {
            let condition = first_macro_argument(&mac.mac)
                .map(|argument| lower_expression(&argument, file, index));
            match condition {
                Some(condition) => OwnedStmt::Require(condition),
                None => OwnedStmt::Require(OwnedExpr {
                    kind: ExprKind::Other,
                    text: short(&index.text(mac.span())),
                    span: index.spanned(file, mac),
                }),
            }
        }
        Stmt::Local(local) => match &local.init {
            Some(init) => OwnedStmt::Expr(lower_expression(&init.expr, file, index)),
            None => OwnedStmt::Block(Vec::new()),
        },
        _ => OwnedStmt::Block(Vec::new()),
    }
}

fn lower_expression_as_statements(
    expression: &Expr,
    file: &str,
    source: &str,
    index: &SourceIndex,
) -> Vec<OwnedStmt> {
    match expression {
        Expr::Block(block) => lower_block(&block.block, file, source, index),
        other => vec![OwnedStmt::Expr(lower_expression(other, file, index))],
    }
}

fn lower_expression(expression: &Expr, file: &str, index: &SourceIndex) -> OwnedExpr {
    let text = short(&index.text(expression.span()));
    let span = index.spanned(file, expression);
    match expression {
        Expr::Path(path) => OwnedExpr {
            kind: ExprKind::Path(
                path.path
                    .segments
                    .last()
                    .map(|segment| segment.ident.to_string())
                    .unwrap_or_default(),
            ),
            text,
            span,
        },
        Expr::Field(field) => OwnedExpr {
            kind: ExprKind::Field {
                base: Box::new(lower_expression(&field.base, file, index)),
                name: match &field.member {
                    syn::Member::Named(name) => name.to_string(),
                    syn::Member::Unnamed(index) => index.index.to_string(),
                },
            },
            text,
            span,
        },
        Expr::MethodCall(call) => OwnedExpr {
            kind: ExprKind::Method {
                receiver: Box::new(lower_expression(&call.receiver, file, index)),
                name: call.method.to_string(),
                args: call
                    .args
                    .iter()
                    .map(|argument| lower_expression(argument, file, index))
                    .collect(),
            },
            text,
            span,
        },
        Expr::Call(call) => OwnedExpr {
            kind: ExprKind::Call {
                callee: Box::new(lower_expression(&call.func, file, index)),
                args: call
                    .args
                    .iter()
                    .map(|argument| lower_expression(argument, file, index))
                    .collect(),
            },
            text,
            span,
        },
        Expr::Binary(binary) => OwnedExpr {
            kind: ExprKind::Binary {
                left: Box::new(lower_expression(&binary.left, file, index)),
                op: match binary.op {
                    BinOp::And(_) => BinOpKind::And,
                    BinOp::Or(_) => BinOpKind::Or,
                    BinOp::Eq(_) => BinOpKind::Eq,
                    BinOp::Ne(_) => BinOpKind::Ne,
                    _ => BinOpKind::Other,
                },
                right: Box::new(lower_expression(&binary.right, file, index)),
            },
            text,
            span,
        },
        Expr::Unary(unary) if matches!(unary.op, UnOp::Not(_)) => OwnedExpr {
            kind: ExprKind::Not(Box::new(lower_expression(&unary.expr, file, index))),
            text,
            span,
        },
        Expr::Lit(literal) => match &literal.lit {
            syn::Lit::Bool(value) => OwnedExpr {
                kind: ExprKind::Bool(value.value),
                text,
                span,
            },
            _ => OwnedExpr {
                kind: ExprKind::Other,
                text,
                span,
            },
        },
        Expr::Index(index_expression) => {
            let _ = index_expression;
            OwnedExpr {
                kind: ExprKind::Index,
                text,
                span,
            }
        }
        Expr::Reference(reference) => lower_expression(&reference.expr, file, index),
        Expr::Paren(paren) => lower_expression(&paren.expr, file, index),
        Expr::Group(group) => lower_expression(&group.expr, file, index),
        _ => OwnedExpr {
            kind: ExprKind::Other,
            text,
            span,
        },
    }
}

fn is_require_macro(mac: &syn::Macro) -> bool {
    mac.path
        .segments
        .last()
        .is_some_and(|segment| matches!(segment.ident.to_string().as_str(), "require" | "assert"))
}

fn first_macro_argument(mac: &syn::Macro) -> Option<Expr> {
    let parser = Punctuated::<Expr, Token![,]>::parse_terminated;
    parser.parse2(mac.tokens.clone()).ok()?.into_iter().next()
}

fn block_rejects(statements: &[OwnedStmt]) -> bool {
    statements.iter().any(|statement| match statement {
        // A lowered `return` becomes a `Call` to `return`; the lowering keeps
        // it as `Other` text starting with `return`. The analysis treats any
        // statement whose text starts with `return` as rejecting the path.
        OwnedStmt::Expr(expression) => expression.text.starts_with("return"),
        _ => false,
    })
}

fn expression_rejects(expression: &Expr) -> bool {
    match expression {
        Expr::Return(_) => true,
        Expr::Block(block) => block.block.stmts.iter().any(statement_rejects),
        Expr::Macro(mac) => mac.mac.path.segments.last().is_some_and(|segment| {
            matches!(
                segment.ident.to_string().as_str(),
                "panic" | "unreachable" | "bail"
            )
        }),
        Expr::If(if_expression) => {
            if_expression
                .then_branch
                .stmts
                .iter()
                .any(statement_rejects)
                && if_expression
                    .else_branch
                    .as_ref()
                    .is_some_and(|(_, otherwise)| expression_rejects(otherwise))
        }
        _ => false,
    }
}

fn statement_rejects(statement: &Stmt) -> bool {
    match statement {
        Stmt::Expr(Expr::Return(_), _) => true,
        Stmt::Expr(Expr::Macro(mac), _) => rejects_macro(&mac.mac),
        Stmt::Macro(mac) => rejects_macro(&mac.mac),
        _ => false,
    }
}

fn rejects_macro(mac: &syn::Macro) -> bool {
    mac.path.segments.last().is_some_and(|segment| {
        matches!(
            segment.ident.to_string().as_str(),
            "panic" | "unreachable" | "bail"
        )
    })
}

/// Reduces the parsed signature to whether any parameter is an `AccountInfo`
/// container. `&[AccountInfo]`, `&[&AccountInfo]` and `Vec<AccountInfo>` all
/// count; a lone `&AccountInfo` does not, because it cannot carry an
/// instruction's account list.
fn takes_account_info(signature: &syn::Signature, index: &SourceIndex) -> bool {
    signature.inputs.iter().any(|argument| match argument {
        FnArg::Typed(argument) => {
            let text = index.text(argument.ty.span());
            text.contains("AccountInfo") && (text.contains('[') || text.contains("Vec"))
        }
        FnArg::Receiver(_) => false,
    })
}

/// The program a file belongs to: `programs/<name>/...`, `<name>/src/...`, or
/// the file stem.
fn program_name(path: &str) -> String {
    let parts: Vec<&str> = path.split('/').collect();
    if let Some(position) = parts.iter().position(|part| *part == "programs")
        && let Some(name) = parts.get(position + 1)
    {
        return normalise(name);
    }
    if let Some(position) = parts.iter().position(|part| *part == "src")
        && position > 0
    {
        return normalise(parts[position - 1]);
    }
    std::path::Path::new(path)
        .file_stem()
        .map(|stem| normalise(&stem.to_string_lossy()))
        .unwrap_or_else(|| "program".to_string())
}

fn normalise(name: &str) -> String {
    name.chars()
        .map(|character| if character == '-' { '_' } else { character })
        .collect()
}

#[derive(Default)]
struct State {
    signers: Vec<(String, Span)>,
    roles: Vec<(String, Span)>,
    constraints: Vec<AccountConstraint>,
    deferred_keys: Vec<(String, String, Span)>,
    notes: Vec<String>,
    ambiguous: Vec<String>,
}

impl State {
    fn signer(&mut self, account: String, span: Span) {
        if !self.signers.iter().any(|(seen, _)| *seen == account) {
            self.signers.push((account, span));
        }
    }

    fn role(&mut self, name: String, span: Span) {
        if !self.roles.iter().any(|(seen, _)| *seen == name) {
            self.roles.push((name, span));
        }
    }

    fn ambiguous(&mut self, reason: String) {
        if !self.ambiguous.contains(&reason) {
            self.ambiguous.push(reason);
        }
    }
}

fn analyse(handler: &Handler) -> EntryPoint {
    let mut state = State::default();
    for statement in &handler.body {
        check_statement(statement, &mut state);
    }
    // A key comparison binds a signer to a configured key. Resolved after the
    // whole body so the order of the checks does not change the answer.
    let deferred = std::mem::take(&mut state.deferred_keys);
    for (left, right, span) in &deferred {
        let left_signer = state.signers.iter().any(|(name, _)| name == left);
        let right_signer = state.signers.iter().any(|(name, _)| name == right);
        match (left_signer, right_signer) {
            (true, false) => state.role(right.clone(), span.clone()),
            (false, true) => state.role(left.clone(), span.clone()),
            _ => {}
        }
    }

    let mut entry_point = EntryPoint::new(
        &handler.program,
        &handler.name,
        EntryPointKind::Instruction,
        handler.span.clone(),
    );
    entry_point.notes.append(&mut state.notes);
    entry_point.constraints = state.constraints;
    if !state.ambiguous.is_empty() {
        let reason = state.ambiguous.join("; ");
        return entry_point.unknown(reason, handler.span.clone(), Provenance::NativeCheck);
    }
    let mut authorities: Vec<Authority> = Vec::new();
    for (name, span) in state.signers {
        authorities.push(Authority::Signer {
            account: name,
            spans: vec![span],
            provenance: Provenance::NativeCheck,
        });
    }
    for (name, span) in state.roles {
        authorities.push(Authority::Role {
            name,
            spans: vec![span],
            provenance: Provenance::NativeCheck,
        });
    }
    entry_point.with_authorities(
        authorities,
        Authority::None {
            spans: vec![handler.span.clone()],
            provenance: Provenance::NativeCheck,
        },
    )
}

fn check_statement(statement: &OwnedStmt, state: &mut State) {
    match statement {
        OwnedStmt::Expr(expression) => check_expression(expression, false, state),
        OwnedStmt::Require(condition) => check_expression(condition, true, state),
        OwnedStmt::If {
            condition,
            then_branch,
            else_branch,
            then_rejects,
            else_rejects,
        } => {
            if *then_rejects && !*else_rejects {
                check_expression(condition, false, state);
            } else if *else_rejects && !*then_rejects {
                check_expression(condition, true, state);
            }
            for nested in then_branch {
                check_statement(nested, state);
            }
            for nested in else_branch {
                check_statement(nested, state);
            }
        }
        OwnedStmt::Block(statements) => {
            for nested in statements {
                check_statement(nested, state);
            }
        }
        OwnedStmt::Match(arms) => {
            for arm in arms {
                for nested in arm {
                    check_statement(nested, state);
                }
            }
        }
    }
}

/// `must_hold`: the expression is a guard that must be true for execution to
/// continue.
fn check_expression(expression: &OwnedExpr, must_hold: bool, state: &mut State) {
    match &expression.kind {
        ExprKind::Not(inner) => check_expression(inner, !must_hold, state),
        ExprKind::Binary { left, op, right } => match op {
            BinOpKind::And => {
                if must_hold {
                    check_expression(left, true, state);
                    check_expression(right, true, state);
                } else if mentions_signer(left) && mentions_signer(right) {
                    state.ambiguous(format!(
                        "disjunctive signer check `{}`",
                        short(&expression.text)
                    ));
                }
            }
            BinOpKind::Or => {
                if must_hold {
                    if mentions_signer(left) || mentions_signer(right) {
                        state.ambiguous(format!(
                            "disjunctive signer check `{}`",
                            short(&expression.text)
                        ));
                    }
                } else {
                    check_expression(left, false, state);
                    check_expression(right, false, state);
                }
            }
            BinOpKind::Eq | BinOpKind::Ne => {
                let equality = matches!(op, BinOpKind::Eq);
                let demanded = (equality && must_hold) || (!equality && !must_hold);
                if demanded {
                    comparison(left, right, state);
                }
            }
            BinOpKind::Other => {}
        },
        ExprKind::Method { receiver, name, .. } if name == "is_signer" && must_hold => {
            match account_name(receiver) {
                Ok(account) => state.signer(account, expression.span.clone()),
                Err(reason) => state.ambiguous(reason),
            }
        }
        ExprKind::Field { base, name } if name == "is_signer" && must_hold => {
            match account_name(base) {
                Ok(account) => state.signer(account, expression.span.clone()),
                Err(reason) => state.ambiguous(reason),
            }
        }
        ExprKind::Call { callee, args } => {
            if let ExprKind::Path(name) = &callee.kind {
                if matches!(name.as_str(), "require" | "assert") {
                    if let Some(condition) = args.first() {
                        check_expression(condition, true, state);
                    }
                    return;
                }
                if matches!(name.as_str(), "has_role" | "hasRole")
                    && must_hold
                    && let Some(role) = args.first()
                {
                    state.role(short(&role.text), expression.span.clone());
                    return;
                }
            }
            if must_hold && mentions_signer(expression) {
                state.ambiguous(format!(
                    "unrecognised signer check `{}`",
                    short(&expression.text)
                ));
            }
        }
        _ => {}
    }
}

fn comparison(left: &OwnedExpr, right: &OwnedExpr, state: &mut State) {
    if let Some(account) = owner_field(left) {
        state.constraints.push(AccountConstraint {
            account,
            kind: ConstraintKind::OwnedBy {
                expression: short(&right.text),
            },
            span: left.span.clone(),
        });
        return;
    }
    if let Some(account) = owner_field(right) {
        state.constraints.push(AccountConstraint {
            account,
            kind: ConstraintKind::OwnedBy {
                expression: short(&left.text),
            },
            span: right.span.clone(),
        });
        return;
    }
    if let Some((left_text, right_text, key_side, span)) = key_pair(left, right) {
        state.constraints.push(AccountConstraint {
            account: key_side,
            kind: ConstraintKind::KeyEquals {
                left: left_text.clone(),
                right: right_text.clone(),
            },
            span: span.clone(),
        });
        state.deferred_keys.push((left_text, right_text, span));
        return;
    }
    // `x.is_signer == true`
    if let Some(receiver) = signer_receiver(left)
        && is_bool_literal(right, true)
    {
        match account_name(receiver) {
            Ok(account) => state.signer(account, left.span.clone()),
            Err(reason) => state.ambiguous(reason),
        }
    } else if let Some(receiver) = signer_receiver(right)
        && is_bool_literal(left, true)
    {
        match account_name(receiver) {
            Ok(account) => state.signer(account, right.span.clone()),
            Err(reason) => state.ambiguous(reason),
        }
    }
}

fn owner_field(expression: &OwnedExpr) -> Option<String> {
    match &expression.kind {
        ExprKind::Field { base, name } if name == "owner" => Some(short(&base.text)),
        _ => None,
    }
}

/// `(left, right, key_side, span)` when either side reads a `.key`/`.key()`.
fn key_pair(left: &OwnedExpr, right: &OwnedExpr) -> Option<(String, String, String, Span)> {
    let left_key = key_operand(left);
    let right_key = key_operand(right);
    match (left_key, right_key) {
        (Some(left_text), None) => Some((
            left_text.clone(),
            short(&right.text),
            left_text,
            left.span.clone(),
        )),
        (None, Some(right_text)) => Some((
            short(&left.text),
            right_text.clone(),
            right_text,
            right.span.clone(),
        )),
        (Some(left_text), Some(right_text)) => {
            Some((left_text.clone(), right_text, left_text, left.span.clone()))
        }
        (None, None) => None,
    }
}

fn key_operand(expression: &OwnedExpr) -> Option<String> {
    match &expression.kind {
        ExprKind::Method {
            receiver,
            name,
            args,
        } if name == "key" && args.is_empty() => Some(short(&receiver.text)),
        ExprKind::Field { base, name } if name == "key" => Some(short(&base.text)),
        _ => None,
    }
}

fn signer_receiver(expression: &OwnedExpr) -> Option<&OwnedExpr> {
    match &expression.kind {
        ExprKind::Method {
            receiver,
            name,
            args,
        } if name == "is_signer" && args.is_empty() => Some(receiver),
        ExprKind::Field { base, name } if name == "is_signer" => Some(base),
        _ => None,
    }
}

fn is_bool_literal(expression: &OwnedExpr, wanted: bool) -> bool {
    matches!(&expression.kind, ExprKind::Bool(value) if *value == wanted)
}

fn account_name(expression: &OwnedExpr) -> Result<String, String> {
    match &expression.kind {
        ExprKind::Path(name) => Ok(name.clone()),
        ExprKind::Field { .. } | ExprKind::Index => Ok(short(&expression.text)),
        ExprKind::Other | ExprKind::Call { .. } | ExprKind::Method { .. } => Err(format!(
            "`is_signer` check on an expression the model cannot name: `{}`",
            short(&expression.text)
        )),
        ExprKind::Binary { .. } | ExprKind::Not(_) | ExprKind::Bool(_) => Err(format!(
            "`is_signer` check on an expression the model cannot name: `{}`",
            short(&expression.text)
        )),
    }
}

fn mentions_signer(expression: &OwnedExpr) -> bool {
    expression.text.contains("is_signer")
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::ItemFn;

    fn index_of(source: &str) -> SourceIndex<'_> {
        SourceIndex::new(source)
    }

    #[test]
    fn program_names_come_from_the_workspace_layout() {
        assert_eq!(program_name("programs/solva/src/lib.rs"), "solva");
        assert_eq!(program_name("level0/src/processor.rs"), "level0");
        assert_eq!(program_name("src/processor.rs"), "processor");
        assert_eq!(program_name("one-off/src/lib.rs"), "one_off");
    }

    #[test]
    fn a_handler_takes_an_account_info_slice() {
        let source = "pub fn process_instruction(program_id: &Pubkey, accounts: &[AccountInfo], data: &[u8]) -> ProgramResult { Ok(()) }";
        let function: ItemFn = syn::parse_str(source).unwrap();
        let index = index_of(source);
        assert!(takes_account_info(&function.sig, &index));
        let source = "pub fn helper(x: u64) -> u64 { x }";
        let function: ItemFn = syn::parse_str(source).unwrap();
        let index = index_of(source);
        assert!(!takes_account_info(&function.sig, &index));
    }

    #[test]
    fn a_negated_signer_check_in_an_if_is_a_requirement() {
        let source = r#"
fn f(accounts: &[AccountInfo]) -> ProgramResult {
    let authority = &accounts[0];
    if !authority.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    Ok(())
}
"#;
        let function: ItemFn = syn::parse_str(source).unwrap();
        let index = index_of(source);
        let handler = lower_handler(&function.sig, &function.block, "src/lib.rs", source, &index);
        let entry = analyse(&handler);
        assert_eq!(entry.authorities.len(), 1);
        assert_eq!(entry.authorities[0].kind(), "signer");
        assert_eq!(entry.authorities[0].name(), Some("authority"));
    }

    #[test]
    fn a_positive_if_signer_check_is_not_a_requirement() {
        let source = r#"
fn f(accounts: &[AccountInfo]) -> ProgramResult {
    let authority = &accounts[0];
    if authority.is_signer {
        do_admin_thing()?;
    }
    Ok(())
}
"#;
        let function: ItemFn = syn::parse_str(source).unwrap();
        let index = index_of(source);
        let handler = lower_handler(&function.sig, &function.block, "src/lib.rs", source, &index);
        let entry = analyse(&handler);
        assert_eq!(entry.authorities[0].kind(), "none");
    }

    #[test]
    fn a_disjunctive_signer_check_is_unknown() {
        let source = r#"
fn f(accounts: &[AccountInfo]) -> ProgramResult {
    require!(accounts[0].is_signer || accounts[1].is_signer, E::Missing);
    Ok(())
}
"#;
        let function: ItemFn = syn::parse_str(source).unwrap();
        let index = index_of(source);
        let handler = lower_handler(&function.sig, &function.block, "src/lib.rs", source, &index);
        let entry = analyse(&handler);
        assert_eq!(entry.authorities.len(), 1);
        assert_eq!(entry.authorities[0].kind(), "unknown");
    }

    #[test]
    fn a_key_comparison_binds_a_signer() {
        let source = r#"
fn f(accounts: &[AccountInfo]) -> ProgramResult {
    let authority = &accounts[0];
    let config = &accounts[1];
    require!(authority.is_signer, E::Missing);
    if authority.key != config.authority {
        return Err(E::Mismatch.into());
    }
    Ok(())
}
"#;
        let function: ItemFn = syn::parse_str(source).unwrap();
        let index = index_of(source);
        let handler = lower_handler(&function.sig, &function.block, "src/lib.rs", source, &index);
        let entry = analyse(&handler);
        let kinds: Vec<&str> = entry.authorities.iter().map(Authority::kind).collect();
        assert!(kinds.contains(&"signer"), "{kinds:?}");
        assert!(
            kinds.contains(&"role"),
            "{kinds:?} constraints={:?}",
            entry.constraints
        );
    }
}
