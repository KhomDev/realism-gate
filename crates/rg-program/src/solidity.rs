//! Solidity: external and public functions, their modifiers and their
//! `msg.sender` guards.
//!
//! `solar`, the parser written for the Foundry toolchain, parses each file.
//! The parsed file is lowered into a small owned model immediately, so the
//! parser's arena can be dropped and contracts from different files can be
//! lined up for inheritance. The rules are documented on [`crate`].

use crate::synutil::short;
use crate::{
    AccountConstraint, Authority, EntryPoint, EntryPointKind, Language, Program, Provenance, Span,
};
use solar_parse::Parser;
use solar_parse::ast::{self, CallArgsKind};
use solar_parse::interface::source_map::{FileName, SourceMap};
use solar_parse::interface::{ColorChoice, Session, Span as SolarSpan};
use std::collections::{BTreeMap, BTreeSet};

/// How deep a modifier or an internal helper call is followed.
const MAX_FOLLOW_DEPTH: usize = 3;

pub(crate) fn collect(
    files: &[(String, String)],
    in_scope: &[String],
) -> (Vec<Program>, Vec<String>) {
    let scope: BTreeSet<&str> = in_scope.iter().map(String::as_str).collect();
    let mut notes = Vec::new();
    let mut contracts: Vec<SolContract> = Vec::new();
    for (file, source) in files {
        let (parsed, errors) = parse_file(file, source);
        for error in errors {
            notes.push(format!("{file}: {error}"));
        }
        contracts.extend(parsed);
    }
    // File order is already sorted by the walk; the sort is what keeps the
    // model stable if a caller passes the files in another order.
    contracts.sort_by(|a, b| {
        (&a.file, a.span.line, a.span.column, &a.name).cmp(&(
            &b.file,
            b.span.line,
            b.span.column,
            &b.name,
        ))
    });

    let mut programs = Vec::new();
    for contract in &contracts {
        if !scope.contains(contract.file.as_str())
            || !matches!(contract.kind, SolKind::Contract | SolKind::Abstract)
        {
            continue;
        }
        let mut entry_points = Vec::new();
        for function in &contract.functions {
            if is_entry_point(function) {
                entry_points.push(resolve(contract, function, &contracts));
            }
        }
        if !entry_points.is_empty() {
            programs.push(Program {
                name: contract.name.clone(),
                id: None,
                language: Language::Solidity,
                entry_points,
            });
        }
    }
    (programs, notes)
}

fn is_entry_point(function: &SolFn) -> bool {
    if function.body.is_none() {
        return false;
    }
    match function.kind {
        SolFnKind::Function => matches!(
            function.visibility.as_deref(),
            Some("public") | Some("external")
        ),
        SolFnKind::Fallback | SolFnKind::Receive => true,
        SolFnKind::Constructor | SolFnKind::Modifier => false,
    }
}

fn resolve(contract: &SolContract, function: &SolFn, all: &[SolContract]) -> EntryPoint {
    let linear = linearize(contract, all);
    let mut analysis = Analysis::default();
    let substitution = Substitution::default();
    let mut visited = BTreeSet::new();
    visited.insert(function_key(function));

    for invocation in &function.modifiers {
        check_modifier(
            invocation,
            &linear,
            &substitution,
            0,
            &mut visited,
            &mut analysis,
        );
    }
    if let Some(body) = &function.body {
        check_block(
            body,
            Provenance::SolidityBody,
            &linear,
            &substitution,
            0,
            &mut visited,
            &mut analysis,
        );
    }

    let mut entry = EntryPoint::new(
        &contract.name,
        &function.name,
        EntryPointKind::Function,
        function.span.clone(),
    );
    entry.notes.append(&mut analysis.notes);
    entry.constraints = analysis.constraints;
    if let Some((_, span, provenance)) = analysis.unknowns.first() {
        let reason = analysis
            .unknowns
            .iter()
            .map(|(reason, _, _)| reason.clone())
            .collect::<Vec<_>>()
            .join("; ");
        return entry.unknown(reason, span.clone(), *provenance);
    }
    let mut authorities: Vec<Authority> = Vec::new();
    for (span, provenance) in analysis.owners {
        authorities.push(Authority::Owner {
            spans: vec![span],
            provenance,
        });
    }
    for (name, span, provenance) in analysis.roles {
        authorities.push(Authority::Role {
            name,
            spans: vec![span],
            provenance,
        });
    }
    entry.with_authorities(
        authorities,
        Authority::None {
            spans: vec![function.span.clone()],
            provenance: Provenance::SolidityBody,
        },
    )
}

#[derive(Default)]
struct Analysis {
    owners: Vec<(Span, Provenance)>,
    roles: Vec<(String, Span, Provenance)>,
    constraints: Vec<AccountConstraint>,
    notes: Vec<String>,
    unknowns: Vec<(String, Span, Provenance)>,
}

impl Analysis {
    fn owner(&mut self, span: Span, provenance: Provenance) {
        if !self.owners.iter().any(|(seen, _)| *seen == span) {
            self.owners.push((span, provenance));
        }
    }

    fn role(&mut self, name: String, span: Span, provenance: Provenance) {
        if !self.roles.iter().any(|(seen, _, _)| *seen == name) {
            self.roles.push((name, span, provenance));
        }
    }

    fn unknown(&mut self, reason: String, span: Span, provenance: Provenance) {
        if !self.unknowns.iter().any(|(seen, _, _)| *seen == reason) {
            self.unknowns.push((reason, span, provenance));
        }
    }
}

fn check_modifier(
    invocation: &SolModifier,
    linear: &[&SolContract],
    substitution: &Substitution,
    depth: usize,
    visited: &mut BTreeSet<String>,
    analysis: &mut Analysis,
) {
    let found = linear.iter().find_map(|contract| {
        contract
            .functions
            .iter()
            .find(|function| {
                function.kind == SolFnKind::Modifier && function.name == invocation.name
            })
            .map(|function| (contract, function))
    });
    match found {
        Some((_, function)) => {
            let key = function_key(function);
            if depth > MAX_FOLLOW_DEPTH || !visited.insert(key.clone()) {
                return;
            }
            let inner = substitution.for_call(&function.params, &invocation.args);
            if let Some(body) = &function.body {
                check_block(
                    body,
                    Provenance::SolidityModifier,
                    linear,
                    &inner,
                    depth + 1,
                    visited,
                    analysis,
                );
            }
            visited.remove(&key);
        }
        None => match invocation.name.as_str() {
            // The canonical modifiers are the two the brief names; without a
            // definition in the tree their meaning is still known, and the
            // note says the body was not found.
            "onlyOwner" => {
                analysis.owner(invocation.span.clone(), Provenance::SolidityModifier);
                analysis.notes.push(
                    "modifier `onlyOwner` is not defined in the tree; its canonical owner check \
                     is used"
                        .to_string(),
                );
            }
            "onlyRole" => match invocation.args.first() {
                Some(role) => {
                    analysis.role(
                        short(role),
                        invocation.span.clone(),
                        Provenance::SolidityModifier,
                    );
                    analysis.notes.push(
                        "modifier `onlyRole` is not defined in the tree; its canonical role \
                         check is used"
                            .to_string(),
                    );
                }
                None => analysis.unknown(
                    "`onlyRole` has no role argument".to_string(),
                    invocation.span.clone(),
                    Provenance::SolidityModifier,
                ),
            },
            _ => analysis.unknown(
                format!(
                    "modifier `{}` is not defined in the contract or its bases",
                    invocation.name
                ),
                invocation.span.clone(),
                Provenance::SolidityModifier,
            ),
        },
    }
}

fn check_block(
    statements: &[SolStmt],
    provenance: Provenance,
    linear: &[&SolContract],
    substitution: &Substitution,
    depth: usize,
    visited: &mut BTreeSet<String>,
    analysis: &mut Analysis,
) {
    for statement in statements {
        check_statement(
            statement,
            true,
            provenance,
            linear,
            substitution,
            depth,
            visited,
            analysis,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn check_statement(
    statement: &SolStmt,
    demanded: bool,
    provenance: Provenance,
    linear: &[&SolContract],
    substitution: &Substitution,
    depth: usize,
    visited: &mut BTreeSet<String>,
    analysis: &mut Analysis,
) {
    match statement {
        SolStmt::Require(condition) => {
            if demanded {
                check_condition(condition, true, provenance, substitution, analysis);
            }
        }
        SolStmt::Expr(expression) => {
            if demanded {
                check_expression_statement(
                    expression,
                    provenance,
                    linear,
                    substitution,
                    depth,
                    visited,
                    analysis,
                );
            }
        }
        SolStmt::If {
            condition,
            then_branch,
            else_branch,
            then_rejects,
            else_rejects,
        } => {
            if demanded {
                if *then_rejects && !*else_rejects {
                    check_condition(condition, false, provenance, substitution, analysis);
                } else if *else_rejects && !*then_rejects {
                    check_condition(condition, true, provenance, substitution, analysis);
                }
            }
            // Only a branch that is on the fall-through path is demanded.
            if demanded {
                if *then_rejects {
                    check_block(
                        else_branch,
                        provenance,
                        linear,
                        substitution,
                        depth,
                        visited,
                        analysis,
                    );
                } else if *else_rejects {
                    check_block(
                        then_branch,
                        provenance,
                        linear,
                        substitution,
                        depth,
                        visited,
                        analysis,
                    );
                }
            }
        }
        SolStmt::Block(statements) => {
            if demanded {
                check_block(
                    statements,
                    provenance,
                    linear,
                    substitution,
                    depth,
                    visited,
                    analysis,
                );
            }
        }
        SolStmt::Revert | SolStmt::Return => {}
    }
}

fn check_expression_statement(
    expression: &SolExpr,
    provenance: Provenance,
    linear: &[&SolContract],
    substitution: &Substitution,
    depth: usize,
    visited: &mut BTreeSet<String>,
    analysis: &mut Analysis,
) {
    let SolExprKind::Call { callee, args } = &expression.kind else {
        return;
    };
    let Some(name) = callee_name(callee) else {
        return;
    };
    if name == "_checkOwner" {
        analysis.owner(expression.span.clone(), provenance);
        return;
    }
    if name == "_checkRole" {
        let role = match (args.first(), args.get(1)) {
            (Some(role), None) => Some(role),
            (Some(role), Some(account)) if is_msg_sender(account) => Some(role),
            _ => None,
        };
        if let Some(role) = role {
            let name = substitution.apply(&role.text);
            analysis.role(short(&name), expression.span.clone(), provenance);
        }
        return;
    }
    if depth >= MAX_FOLLOW_DEPTH {
        return;
    }
    let Some(function) = find_function(linear, name) else {
        return;
    };
    let key = function_key(function);
    if !visited.insert(key.clone()) {
        return;
    }
    let inner = substitution.for_call(
        &function.params,
        &args.iter().map(|a| a.text.clone()).collect::<Vec<_>>(),
    );
    if let Some(body) = &function.body {
        check_block(
            body,
            provenance,
            linear,
            &inner,
            depth + 1,
            visited,
            analysis,
        );
    }
    visited.remove(&key);
}

fn check_condition(
    expression: &SolExpr,
    must_hold: bool,
    provenance: Provenance,
    substitution: &Substitution,
    analysis: &mut Analysis,
) {
    match &expression.kind {
        SolExprKind::Unary { op, inner } if op == "!" => {
            check_condition(inner, !must_hold, provenance, substitution, analysis);
        }
        SolExprKind::Binary { left, op, right } => match op.as_str() {
            "&&" => {
                if must_hold {
                    check_condition(left, true, provenance, substitution, analysis);
                    check_condition(right, true, provenance, substitution, analysis);
                } else if mentions_authority(left) && mentions_authority(right) {
                    analysis.unknown(
                        format!("disjunctive authority check `{}`", short(&expression.text)),
                        expression.span.clone(),
                        provenance,
                    );
                }
            }
            "||" => {
                if must_hold {
                    if mentions_authority(left) || mentions_authority(right) {
                        analysis.unknown(
                            format!("disjunctive authority check `{}`", short(&expression.text)),
                            expression.span.clone(),
                            provenance,
                        );
                    }
                } else {
                    check_condition(left, false, provenance, substitution, analysis);
                    check_condition(right, false, provenance, substitution, analysis);
                }
            }
            "==" | "!=" => {
                let equality = op == "==";
                let demanded = (equality && must_hold) || (!equality && !must_hold);
                if demanded {
                    comparison(left, right, expression, provenance, substitution, analysis);
                }
            }
            _ => {
                if must_hold && mentions_msg_sender(expression) {
                    analysis.unknown(
                        format!(
                            "unrecognised msg.sender check `{}`",
                            short(&expression.text)
                        ),
                        expression.span.clone(),
                        provenance,
                    );
                }
            }
        },
        SolExprKind::Call { callee, args } => {
            if let Some(name) = callee_name(callee) {
                if matches!(name, "hasRole" | "has_role") {
                    let role = match (args.first(), args.get(1)) {
                        (Some(role), None) => Some(role),
                        (Some(role), Some(account)) if is_msg_sender(account) => Some(role),
                        _ => None,
                    };
                    if must_hold && let Some(role) = role {
                        let name = substitution.apply(&role.text);
                        analysis.role(short(&name), expression.span.clone(), provenance);
                    }
                    return;
                }
                if name == "_checkRole" {
                    let role = match (args.first(), args.get(1)) {
                        (Some(role), None) => Some(role),
                        (Some(role), Some(account)) if is_msg_sender(account) => Some(role),
                        _ => None,
                    };
                    if let Some(role) = role {
                        let name = substitution.apply(&role.text);
                        analysis.role(short(&name), expression.span.clone(), provenance);
                    }
                    return;
                }
                if matches!(name, "require" | "assert") {
                    if let Some(condition) = args.first() {
                        check_condition(condition, must_hold, provenance, substitution, analysis);
                    }
                    return;
                }
            }
            if must_hold && mentions_msg_sender(expression) {
                analysis.unknown(
                    format!(
                        "unrecognised msg.sender check `{}`",
                        short(&expression.text)
                    ),
                    expression.span.clone(),
                    provenance,
                );
            }
        }
        _ => {
            if must_hold && mentions_authority(expression) {
                analysis.unknown(
                    format!("unrecognised authority check `{}`", short(&expression.text)),
                    expression.span.clone(),
                    provenance,
                );
            }
        }
    }
}

fn comparison(
    left: &SolExpr,
    right: &SolExpr,
    expression: &SolExpr,
    provenance: Provenance,
    substitution: &Substitution,
    analysis: &mut Analysis,
) {
    if is_msg_sender(left) {
        check_against(right, expression, provenance, substitution, analysis);
    } else if is_msg_sender(right) {
        check_against(left, expression, provenance, substitution, analysis);
    } else if mentions_authority(left) || mentions_authority(right) {
        // A comparison of two non-caller accounts (`owner() == admin`) is not a
        // caller authority.
    }
}

fn check_against(
    other: &SolExpr,
    expression: &SolExpr,
    provenance: Provenance,
    substitution: &Substitution,
    analysis: &mut Analysis,
) {
    if is_owner(other) {
        analysis.owner(expression.span.clone(), provenance);
        return;
    }
    if is_msg_sender(other) {
        return;
    }
    let text = short(&other.text);
    if text == "address(this)" || is_zero_address(&text) {
        // A self-call gate or a zero-address sanity check is not authority.
        return;
    }
    if let SolExprKind::Call { callee, args } = &other.kind
        && callee_name(callee).is_some_and(|name| matches!(name, "hasRole" | "has_role"))
    {
        if let Some(role) = args.first() {
            let name = substitution.apply(&role.text);
            analysis.role(short(&name), expression.span.clone(), provenance);
        }
        return;
    }
    if let SolExprKind::Ident(name) = &other.kind
        && (name == "owner" || name == "_owner")
    {
        analysis.owner(expression.span.clone(), provenance);
        return;
    }
    let name = substitution.apply(&text);
    analysis.role(short(&name), expression.span.clone(), provenance);
}

fn is_msg_sender(expression: &SolExpr) -> bool {
    match &expression.kind {
        SolExprKind::Member { base, name } => {
            name == "sender" && matches!(&base.kind, SolExprKind::Ident(ident) if ident == "msg")
        }
        SolExprKind::Call { callee, args } => {
            args.is_empty()
                && callee_name(callee)
                    .is_some_and(|name| name == "_msgSender" || name == "msgSender")
        }
        _ => false,
    }
}

fn is_owner(expression: &SolExpr) -> bool {
    match &expression.kind {
        SolExprKind::Call { callee, args } => {
            args.is_empty()
                && callee_name(callee).is_some_and(|name| name == "owner" || name == "_owner")
        }
        SolExprKind::Ident(name) => name == "owner" || name == "_owner",
        _ => false,
    }
}

fn is_zero_address(text: &str) -> bool {
    let Some(rest) = text.strip_prefix("address(0") else {
        return false;
    };
    let Some(inner) = rest.strip_suffix(')') else {
        return false;
    };
    inner
        .chars()
        .all(|character| matches!(character, '0' | 'x' | 'X'))
}

fn mentions_msg_sender(expression: &SolExpr) -> bool {
    expression.text.contains("msg.sender") || expression.text.contains("_msgSender")
}

fn mentions_authority(expression: &SolExpr) -> bool {
    mentions_msg_sender(expression)
        || expression.text.contains("owner()")
        || expression.text.contains("_owner()")
        || expression.text.contains("hasRole")
        || expression.text.contains("_checkRole")
}

fn callee_name(callee: &SolExpr) -> Option<&str> {
    match &callee.kind {
        SolExprKind::Ident(name) => Some(name),
        SolExprKind::Member { name, .. } => Some(name),
        _ => None,
    }
}

fn function_key(function: &SolFn) -> String {
    format!(
        "{}:{}:{}:{}",
        function.span.file, function.span.line, function.span.column, function.name
    )
}

fn find_function<'a>(linear: &[&'a SolContract], name: &str) -> Option<&'a SolFn> {
    linear.iter().find_map(|contract| {
        contract.functions.iter().find(|function| {
            function.kind == SolFnKind::Function && function.name == name && function.body.is_some()
        })
    })
}

/// The inheritance order `project.rs` uses: the contract first, then bases
/// depth-first, most derived first, without repeats.
fn linearize<'a>(contract: &'a SolContract, all: &'a [SolContract]) -> Vec<&'a SolContract> {
    let mut out: Vec<&'a SolContract> = Vec::new();
    let mut stack = vec![contract];
    while let Some(next) = stack.pop() {
        if out
            .iter()
            .any(|seen| seen.file == next.file && seen.name == next.name)
        {
            continue;
        }
        for base in &next.bases {
            if let Some(found) = resolve_contract(all, &next.file, base) {
                stack.push(found);
            }
        }
        out.push(next);
    }
    out
}

fn resolve_contract<'a>(
    all: &'a [SolContract],
    from_file: &str,
    name: &str,
) -> Option<&'a SolContract> {
    all.iter()
        .find(|candidate| candidate.file == from_file && candidate.name == name)
        .or_else(|| all.iter().find(|candidate| candidate.name == name))
}

#[derive(Default)]
struct Substitution {
    map: BTreeMap<String, String>,
}

impl Substitution {
    fn apply(&self, text: &str) -> String {
        let trimmed = text.trim();
        self.map
            .get(trimmed)
            .cloned()
            .unwrap_or_else(|| text.to_string())
    }

    fn for_call(&self, parameters: &[String], arguments: &[String]) -> Substitution {
        let map = parameters
            .iter()
            .zip(arguments)
            .map(|(parameter, argument)| (parameter.clone(), self.apply(argument)))
            .collect();
        Substitution { map }
    }
}

// --- The owned model of one parsed file -----------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SolKind {
    Contract,
    Abstract,
    Interface,
    Library,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SolFnKind {
    Function,
    Modifier,
    Constructor,
    Fallback,
    Receive,
}

struct SolContract {
    name: String,
    kind: SolKind,
    bases: Vec<String>,
    functions: Vec<SolFn>,
    file: String,
    span: Span,
}

struct SolFn {
    name: String,
    kind: SolFnKind,
    visibility: Option<String>,
    params: Vec<String>,
    modifiers: Vec<SolModifier>,
    body: Option<Vec<SolStmt>>,
    span: Span,
}

struct SolModifier {
    name: String,
    args: Vec<String>,
    span: Span,
}

enum SolStmt {
    Expr(SolExpr),
    Require(SolExpr),
    If {
        condition: SolExpr,
        then_branch: Vec<SolStmt>,
        else_branch: Vec<SolStmt>,
        then_rejects: bool,
        else_rejects: bool,
    },
    Block(Vec<SolStmt>),
    Revert,
    Return,
}

struct SolExpr {
    kind: SolExprKind,
    text: String,
    span: Span,
}

enum SolExprKind {
    Ident(String),
    Member {
        base: Box<SolExpr>,
        name: String,
    },
    Call {
        callee: Box<SolExpr>,
        args: Vec<SolExpr>,
    },
    Binary {
        left: Box<SolExpr>,
        op: String,
        right: Box<SolExpr>,
    },
    Unary {
        op: String,
        inner: Box<SolExpr>,
    },
    Bool,
    Other,
}

// --- Parsing solar's AST into the owned model -----------------------------

fn parse_file(file: &str, source: &str) -> (Vec<SolContract>, Vec<String>) {
    let sess = Session::builder()
        .with_buffer_emitter(ColorChoice::Never)
        .build();
    let mut contracts = Vec::new();
    sess.enter_sequential(|| {
        let arena = ast::Arena::new();
        let mut parser = match Parser::from_source_code(
            &sess,
            &arena,
            FileName::Custom(file.to_string()),
            source.to_string(),
        ) {
            Ok(parser) => parser,
            Err(_) => return,
        };
        match parser.parse_file() {
            Ok(unit) => {
                let lower = Lower {
                    sm: sess.source_map(),
                    file,
                };
                contracts = lower.unit(&unit);
            }
            Err(error) => {
                error.emit();
            }
        }
    });
    let errors = match sess.emitted_errors() {
        Some(Err(diagnostics)) => vec![summarize(&diagnostics.to_string())],
        _ => Vec::new(),
    };
    (contracts, errors)
}

fn summarize(rendered: &str) -> String {
    for line in rendered.lines() {
        if line.starts_with("error") {
            return format!("did not parse cleanly: {}", line.trim());
        }
    }
    "did not parse cleanly".to_string()
}

struct Lower<'a> {
    sm: &'a SourceMap,
    file: &'a str,
}

impl Lower<'_> {
    fn span(&self, span: SolarSpan) -> Span {
        let location = self.sm.lookup_char_pos(span.lo());
        Span {
            file: self.file.to_string(),
            line: u32::try_from(location.data.line).unwrap_or(u32::MAX),
            column: u32::try_from(location.data.col.0 + 1).unwrap_or(u32::MAX),
            text: short(&self.sm.span_to_snippet(span).unwrap_or_default()),
        }
    }

    fn text(&self, span: SolarSpan) -> String {
        short(&self.sm.span_to_snippet(span).unwrap_or_default())
    }

    fn unit(&self, unit: &ast::SourceUnit<'_>) -> Vec<SolContract> {
        unit.items
            .iter()
            .filter_map(|item| match &item.kind {
                ast::ItemKind::Contract(contract) => Some(self.contract(contract, item.span)),
                _ => None,
            })
            .collect()
    }

    fn contract(&self, contract: &ast::ItemContract<'_>, span: SolarSpan) -> SolContract {
        let kind = match contract.kind {
            ast::ContractKind::Contract => SolKind::Contract,
            ast::ContractKind::AbstractContract => SolKind::Abstract,
            ast::ContractKind::Interface => SolKind::Interface,
            ast::ContractKind::Library => SolKind::Library,
        };
        let functions = contract
            .body
            .iter()
            .filter_map(|item| match &item.kind {
                ast::ItemKind::Function(function) => Some(self.function(function, item.span)),
                _ => None,
            })
            .collect();
        SolContract {
            name: contract.name.as_str().to_string(),
            kind,
            bases: contract
                .bases
                .iter()
                .map(|base| base.name.last().as_str().to_string())
                .collect(),
            functions,
            file: self.file.to_string(),
            span: self.span(span),
        }
    }

    fn function(&self, function: &ast::ItemFunction<'_>, span: SolarSpan) -> SolFn {
        let kind = match function.kind {
            ast::FunctionKind::Function => SolFnKind::Function,
            ast::FunctionKind::Modifier => SolFnKind::Modifier,
            ast::FunctionKind::Constructor => SolFnKind::Constructor,
            ast::FunctionKind::Fallback => SolFnKind::Fallback,
            ast::FunctionKind::Receive => SolFnKind::Receive,
        };
        let name = match (&function.header.name, kind) {
            (Some(name), _) => name.as_str().to_string(),
            (None, SolFnKind::Constructor) => "constructor".to_string(),
            (None, SolFnKind::Fallback) => "fallback".to_string(),
            (None, SolFnKind::Receive) => "receive".to_string(),
            (None, _) => String::new(),
        };
        SolFn {
            name,
            kind,
            visibility: function
                .header
                .visibility
                .map(|visibility| visibility.to_str().to_string()),
            params: function
                .header
                .parameters
                .vars
                .iter()
                .map(|variable| {
                    variable
                        .name
                        .map(|name| name.as_str().to_string())
                        .unwrap_or_default()
                })
                .collect(),
            modifiers: function
                .header
                .modifiers
                .iter()
                .map(|modifier| SolModifier {
                    name: modifier.name.last().as_str().to_string(),
                    args: match &modifier.arguments.kind {
                        CallArgsKind::Unnamed(arguments) => arguments
                            .iter()
                            .map(|argument| self.text(argument.span))
                            .collect(),
                        CallArgsKind::Named(arguments) => arguments
                            .iter()
                            .map(|argument| {
                                format!("{}: {}", argument.name, self.text(argument.value.span))
                            })
                            .collect(),
                    },
                    span: self.span(modifier.span()),
                })
                .collect(),
            body: function.body.as_ref().map(|body| self.block(body)),
            span: self.span(span),
        }
    }

    fn block(&self, block: &ast::Block<'_>) -> Vec<SolStmt> {
        block
            .stmts
            .iter()
            .flat_map(|statement| self.statement(statement))
            .collect()
    }

    fn statement(&self, statement: &ast::Stmt<'_>) -> Vec<SolStmt> {
        match &statement.kind {
            ast::StmtKind::Expr(expression) => vec![self.expression_statement(expression)],
            ast::StmtKind::DeclSingle(variable) => match &variable.initializer {
                Some(initializer) => vec![SolStmt::Expr(self.expr(initializer))],
                None => Vec::new(),
            },
            ast::StmtKind::DeclMulti(_, initializer) => {
                vec![SolStmt::Expr(self.expr(initializer))]
            }
            ast::StmtKind::Block(block) | ast::StmtKind::UncheckedBlock(block) => {
                vec![SolStmt::Block(self.block(block))]
            }
            ast::StmtKind::If(condition, then_branch, else_branch) => {
                let then_lowered = self.statement(then_branch);
                let else_lowered = else_branch
                    .as_ref()
                    .map(|branch| self.statement(branch))
                    .unwrap_or_default();
                vec![SolStmt::If {
                    condition: self.expr(condition),
                    then_rejects: rejects(&then_lowered),
                    else_rejects: rejects(&else_lowered),
                    then_branch: then_lowered,
                    else_branch: else_lowered,
                }]
            }
            ast::StmtKind::For { body, .. }
            | ast::StmtKind::While(_, body)
            | ast::StmtKind::DoWhile(body, _) => vec![SolStmt::Block(self.statement(body))],
            ast::StmtKind::Try(try_statement) => {
                let mut statements = vec![SolStmt::Expr(self.expr(&*try_statement.expr))];
                for clause in try_statement.clauses.iter() {
                    statements.push(SolStmt::Block(self.block(&clause.block)));
                }
                statements
            }
            ast::StmtKind::Revert(_, _) => vec![SolStmt::Revert],
            ast::StmtKind::Return(_) => vec![SolStmt::Return],
            _ => Vec::new(),
        }
    }

    fn expression_statement(&self, expression: &ast::Expr<'_>) -> SolStmt {
        if let ast::ExprKind::Call(callee, arguments) = &expression.kind
            && let ast::ExprKind::Ident(name) = &callee.kind
        {
            if matches!(name.as_str(), "require" | "assert")
                && let Some(condition) = first_argument(arguments)
            {
                return SolStmt::Require(self.expr(condition));
            }
            if name.as_str() == "revert" {
                // `revert();` is an expression call; `revert Custom();` is a
                // statement. Both reject the path.
                return SolStmt::Revert;
            }
        }
        SolStmt::Expr(self.expr(expression))
    }

    fn expr(&self, expression: &ast::Expr<'_>) -> SolExpr {
        let kind = match &expression.kind {
            ast::ExprKind::Ident(name) => SolExprKind::Ident(name.as_str().to_string()),
            ast::ExprKind::Member(base, member) => SolExprKind::Member {
                base: Box::new(self.expr(base)),
                name: member.as_str().to_string(),
            },
            ast::ExprKind::Call(callee, arguments) => {
                let args = match &arguments.kind {
                    CallArgsKind::Unnamed(arguments) => arguments
                        .iter()
                        .map(|argument| self.expr(argument))
                        .collect(),
                    CallArgsKind::Named(arguments) => arguments
                        .iter()
                        .map(|argument| self.expr(&*argument.value))
                        .collect(),
                };
                SolExprKind::Call {
                    callee: Box::new(self.expr(callee)),
                    args,
                }
            }
            ast::ExprKind::Binary(left, operator, right) => SolExprKind::Binary {
                left: Box::new(self.expr(left)),
                op: operator.kind.to_str().to_string(),
                right: Box::new(self.expr(right)),
            },
            ast::ExprKind::Unary(operator, inner) => SolExprKind::Unary {
                op: operator.kind.to_str().to_string(),
                inner: Box::new(self.expr(inner)),
            },
            ast::ExprKind::Lit(literal, _) => match &literal.kind {
                ast::LitKind::Bool(_) => SolExprKind::Bool,
                _ => SolExprKind::Other,
            },
            _ => SolExprKind::Other,
        };
        SolExpr {
            kind,
            text: self.text(expression.span),
            span: self.span(expression.span),
        }
    }
}

fn first_argument<'a, 'ast>(arguments: &'a ast::CallArgs<'ast>) -> Option<&'a ast::Expr<'ast>> {
    match &arguments.kind {
        CallArgsKind::Unnamed(arguments) => arguments.first().map(|argument| &**argument),
        CallArgsKind::Named(arguments) => arguments.first().map(|argument| &*argument.value),
    }
}

fn rejects(statements: &[SolStmt]) -> bool {
    for statement in statements.iter().rev() {
        match statement {
            SolStmt::Revert | SolStmt::Return => return true,
            SolStmt::Block(inner) => {
                if !inner.is_empty() {
                    return rejects(inner);
                }
            }
            _ => return false,
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(source: &str) -> Vec<SolContract> {
        let (contracts, errors) = parse_file("src/A.sol", source);
        assert!(errors.is_empty(), "{errors:?}");
        contracts
    }

    fn entry(contracts: &[SolContract], contract: &str, function: &str) -> EntryPoint {
        let found = contracts
            .iter()
            .find(|candidate| candidate.name == contract)
            .unwrap();
        let function = found
            .functions
            .iter()
            .find(|candidate| candidate.name == function)
            .unwrap();
        resolve(found, function, contracts)
    }

    #[test]
    fn only_owner_resolves_through_the_canonical_name() {
        let contracts = parse("contract Vault { function withdraw() external onlyOwner {} }");
        let entry = entry(&contracts, "Vault", "withdraw");
        assert_eq!(entry.authorities.len(), 1);
        assert_eq!(entry.authorities[0].kind(), "owner");
    }

    #[test]
    fn only_role_carries_its_argument() {
        let contracts =
            parse("contract Vault { function mint() external onlyRole(MINTER_ROLE) {} }");
        let entry = entry(&contracts, "Vault", "mint");
        assert_eq!(entry.authorities.len(), 1);
        assert_eq!(entry.authorities[0].kind(), "role");
        assert_eq!(entry.authorities[0].name(), Some("MINTER_ROLE"));
    }

    #[test]
    fn a_body_require_on_owner_is_an_owner() {
        let contracts = parse(
            "contract Vault { function close() external { require(msg.sender == owner(), \"no\"); } }",
        );
        let entry = entry(&contracts, "Vault", "close");
        assert_eq!(entry.authorities[0].kind(), "owner");
    }

    #[test]
    fn an_if_revert_on_the_owner_is_an_owner() {
        let contracts = parse(
            "contract Vault { function close() external { if (msg.sender != owner()) { revert(); } } }",
        );
        let entry = entry(&contracts, "Vault", "close");
        assert_eq!(entry.authorities[0].kind(), "owner");
    }

    #[test]
    fn an_unrecognised_sender_check_is_unknown() {
        let contracts = parse(
            "contract Vault { function close() external { require(isAllowed(msg.sender), \"no\"); } }",
        );
        let entry = entry(&contracts, "Vault", "close");
        assert_eq!(entry.authorities[0].kind(), "unknown");
    }

    #[test]
    fn a_disjunction_is_unknown() {
        let contracts = parse(
            "contract Vault { function close() external { require(msg.sender == owner() || msg.sender == admin, \"no\"); } }",
        );
        let entry = entry(&contracts, "Vault", "close");
        assert_eq!(entry.authorities[0].kind(), "unknown");
    }

    #[test]
    fn inheritance_puts_the_most_derived_contract_first() {
        let contracts = parse(
            "contract Base { modifier onlyAdmin() { require(msg.sender == admin, \"no\"); _; } } \
             contract Child is Base { function f() external onlyAdmin {} }",
        );
        let child = contracts
            .iter()
            .find(|contract| contract.name == "Child")
            .unwrap();
        let linear = linearize(child, &contracts);
        assert_eq!(linear[0].name, "Child");
        assert_eq!(linear[1].name, "Base");
        let entry = entry(&contracts, "Child", "f");
        assert_eq!(entry.authorities[0].kind(), "role");
        assert_eq!(entry.authorities[0].name(), Some("admin"));
    }

    #[test]
    fn a_modifier_argument_reaches_the_inherited_role_check() {
        let contracts = parse(
            "contract Access { modifier onlyRole(bytes32 role) { require(hasRole(role, msg.sender), \"no\"); _; } } \
             contract Vault is Access { function mint() external onlyRole(MINTER_ROLE) {} }",
        );
        let entry = entry(&contracts, "Vault", "mint");
        assert_eq!(entry.authorities[0].kind(), "role");
        assert_eq!(entry.authorities[0].name(), Some("MINTER_ROLE"));
    }

    #[test]
    fn a_canonical_modifier_defined_without_a_check_contributes_nothing() {
        let contracts = parse(
            "contract Vault { modifier onlyOwner() { _; } function open() external onlyOwner {} }",
        );
        let entry = entry(&contracts, "Vault", "open");
        assert_eq!(entry.authorities[0].kind(), "none");
    }

    #[test]
    fn a_zero_address_sanity_check_is_not_authority() {
        let contracts = parse(
            "contract Vault { function open() external { require(msg.sender != address(0), \"no\"); } }",
        );
        let entry = entry(&contracts, "Vault", "open");
        assert_eq!(entry.authorities[0].kind(), "none");
    }

    #[test]
    fn a_missing_custom_modifier_is_unknown() {
        let contracts = parse("contract Vault { function open() external onlyAdmin {} }");
        let entry = entry(&contracts, "Vault", "open");
        assert_eq!(entry.authorities[0].kind(), "unknown");
    }
}
