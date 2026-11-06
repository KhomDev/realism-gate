//! Lowers the `solar` Solidity AST into the owned model in [`crate::ir`].
//!
//! `solar` is the Solidity parser written for the Foundry toolchain. Using a
//! real parser instead of regular expressions is what lets the rules tell a
//! cheatcode call from a comment that mentions one, follow an argument into a
//! nested call, and point at an exact line and column.

use crate::ir::{
    Contract, ContractKind, Expr, ExprKind, FnKind, Function, Lit, Pos, StateVar, Stmt, StmtKind,
    Unit,
};
use solar_parse::Parser;
use solar_parse::ast::{self, LitKind};
use solar_parse::interface::source_map::{FileName, SourceMap};
use solar_parse::interface::{ColorChoice, Session, Span};

/// Parses one Solidity source. Never panics on malformed input: parser errors
/// are returned in [`Unit::errors`] alongside whatever the parser recovered.
pub fn parse_source(path: &str, source: &str) -> Unit {
    let sess = Session::builder()
        .with_buffer_emitter(ColorChoice::Never)
        .build();
    let contracts = sess.enter_sequential(|| {
        let arena = ast::Arena::new();
        let mut parser = match Parser::from_source_code(
            &sess,
            &arena,
            FileName::Custom(path.to_string()),
            source.to_string(),
        ) {
            Ok(parser) => parser,
            Err(_) => return Vec::new(),
        };
        match parser.parse_file() {
            Ok(unit) => Lower {
                sm: sess.source_map(),
            }
            .unit(&unit),
            Err(err) => {
                err.emit();
                Vec::new()
            }
        }
    });
    let errors = match sess.emitted_errors() {
        Some(Err(diags)) => summarize_diagnostics(&diags.to_string()),
        _ => Vec::new(),
    };
    Unit {
        path: path.to_string(),
        source: source.to_string(),
        contracts,
        solana: None,
        errors,
    }
}

/// Reduces the parser's rendered diagnostics to `message (line N, column M)`.
///
/// The rendered form carries source excerpts and, in some builds, notes naming
/// the parser's own source files on the machine that compiled it. Neither
/// belongs in a report that may be shared, so only the headline and the
/// position survive.
fn summarize_diagnostics(rendered: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current: Option<String> = None;
    for line in rendered.lines() {
        let trimmed = line.trim_start();
        if line.starts_with("error") {
            if let Some(done) = current.take() {
                out.push(done);
            }
            current = Some(line.trim().to_string());
        } else if let Some(msg) = current.as_mut()
            && let Some(rest) = trimmed
                .strip_prefix("╭▸ ")
                .or_else(|| trimmed.strip_prefix("--> "))
        {
            // `<file>:line:col` -> `(line N, column M)`
            let mut parts = rest.rsplitn(3, ':');
            if let (Some(col), Some(line_no)) = (parts.next(), parts.next())
                && !msg.contains("(line ")
            {
                msg.push_str(&format!(
                    " (line {}, column {})",
                    line_no.trim(),
                    col.trim()
                ));
            }
        }
    }
    if let Some(done) = current {
        out.push(done);
    }
    out
}

struct Lower<'a> {
    sm: &'a SourceMap,
}

impl Lower<'_> {
    fn pos(&self, span: Span) -> Pos {
        let loc = self.sm.lookup_char_pos(span.lo());
        Pos {
            line: loc.data.line as u32,
            col: loc.data.col.0 as u32 + 1,
        }
    }

    fn text(&self, span: Span) -> String {
        self.sm.span_to_snippet(span).unwrap_or_default()
    }

    fn unit(&self, unit: &ast::SourceUnit<'_>) -> Vec<Contract> {
        unit.items
            .iter()
            .filter_map(|item| match &item.kind {
                ast::ItemKind::Contract(c) => Some(self.contract(c, item.span)),
                _ => None,
            })
            .collect()
    }

    fn contract(&self, c: &ast::ItemContract<'_>, span: Span) -> Contract {
        let kind = match c.kind {
            ast::ContractKind::Contract => ContractKind::Contract,
            ast::ContractKind::AbstractContract => ContractKind::Abstract,
            ast::ContractKind::Interface => ContractKind::Interface,
            ast::ContractKind::Library => ContractKind::Library,
        };
        let mut vars = Vec::new();
        let mut functions = Vec::new();
        for item in c.body.iter() {
            match &item.kind {
                ast::ItemKind::Function(f) => functions.push(self.function(f, item.span)),
                ast::ItemKind::Variable(v) => vars.push(StateVar {
                    name: v.name.map(|n| n.as_str().to_string()).unwrap_or_default(),
                    ty: self.text(v.ty.span),
                    constant: matches!(
                        v.mutability,
                        Some(ast::VarMut::Constant) | Some(ast::VarMut::Immutable)
                    ),
                    init: v.initializer.as_ref().map(|e| self.expr(e)),
                    pos: self.pos(item.span),
                }),
                _ => {}
            }
        }
        Contract {
            name: c.name.as_str().to_string(),
            kind,
            bases: c
                .bases
                .iter()
                .map(|m| m.name.last().as_str().to_string())
                .collect(),
            vars,
            functions,
            pos: self.pos(span),
        }
    }

    fn function(&self, f: &ast::ItemFunction<'_>, span: Span) -> Function {
        let kind = match f.kind {
            ast::FunctionKind::Constructor => FnKind::Constructor,
            ast::FunctionKind::Function => FnKind::Function,
            ast::FunctionKind::Modifier => FnKind::Modifier,
            ast::FunctionKind::Fallback => FnKind::Fallback,
            ast::FunctionKind::Receive => FnKind::Receive,
        };
        let name = match (&f.header.name, kind) {
            (Some(name), _) => name.as_str().to_string(),
            (None, FnKind::Constructor) => "constructor".to_string(),
            (None, FnKind::Fallback) => "fallback".to_string(),
            (None, FnKind::Receive) => "receive".to_string(),
            (None, _) => String::new(),
        };
        let params = f
            .header
            .parameters
            .vars
            .iter()
            .map(|v| {
                (
                    v.name.map(|n| n.as_str().to_string()).unwrap_or_default(),
                    self.text(v.ty.span),
                )
            })
            .collect();
        let body = f.body.as_ref().map(|b| {
            let mut out = Vec::new();
            self.block(b, &mut out);
            out
        });
        Function {
            name,
            kind,
            visibility: f.header.visibility.map(|v| v.to_str().to_string()),
            params,
            modifiers: f
                .header
                .modifiers
                .iter()
                .map(|m| m.name.last().as_str().to_string())
                .collect(),
            body,
            pos: self.pos(span),
        }
    }

    fn block(&self, block: &ast::Block<'_>, out: &mut Vec<Stmt>) {
        for stmt in block.stmts.iter() {
            self.stmt(stmt, out);
        }
    }

    fn push_expr(&self, e: &ast::Expr<'_>, out: &mut Vec<Stmt>) {
        let expr = self.expr(e);
        out.push(Stmt {
            pos: expr.pos,
            kind: StmtKind::Expr(expr),
        });
    }

    fn stmt(&self, s: &ast::Stmt<'_>, out: &mut Vec<Stmt>) {
        let pos = self.pos(s.span);
        match &s.kind {
            ast::StmtKind::Expr(e) => self.push_expr(e, out),
            ast::StmtKind::DeclSingle(v) => out.push(Stmt {
                pos,
                kind: StmtKind::Decl {
                    vars: vec![(
                        v.name.map(|n| n.as_str().to_string()).unwrap_or_default(),
                        self.text(v.ty.span),
                    )],
                    init: v.initializer.as_ref().map(|e| self.expr(e)),
                },
            }),
            ast::StmtKind::DeclMulti(vars, init) => out.push(Stmt {
                pos,
                kind: StmtKind::Decl {
                    vars: vars
                        .iter()
                        .map(|slot| match slot {
                            solar_parse::interface::SpannedOption::Some(v) => (
                                v.name.map(|n| n.as_str().to_string()).unwrap_or_default(),
                                self.text(v.ty.span),
                            ),
                            solar_parse::interface::SpannedOption::None(_) => {
                                (String::new(), String::new())
                            }
                        })
                        .collect(),
                    init: Some(self.expr(init)),
                },
            }),
            ast::StmtKind::Block(b) | ast::StmtKind::UncheckedBlock(b) => self.block(b, out),
            ast::StmtKind::If(cond, then, otherwise) => {
                self.push_expr(cond, out);
                self.stmt(then, out);
                if let Some(otherwise) = otherwise {
                    self.stmt(otherwise, out);
                }
            }
            ast::StmtKind::For {
                init,
                cond,
                next,
                body,
            } => {
                if let Some(init) = init {
                    self.stmt(init, out);
                }
                if let Some(cond) = cond {
                    self.push_expr(cond, out);
                }
                self.stmt(body, out);
                if let Some(next) = next {
                    self.push_expr(next, out);
                }
            }
            ast::StmtKind::While(cond, body) => {
                self.push_expr(cond, out);
                self.stmt(body, out);
            }
            ast::StmtKind::DoWhile(body, cond) => {
                self.stmt(body, out);
                self.push_expr(cond, out);
            }
            ast::StmtKind::Emit(path, args) => out.push(Stmt {
                pos,
                kind: StmtKind::Emit {
                    event: path.last().as_str().to_string(),
                    args: self.call_args(args).0,
                },
            }),
            ast::StmtKind::Return(e) => out.push(Stmt {
                pos,
                kind: StmtKind::Return(e.as_ref().map(|e| self.expr(e))),
            }),
            ast::StmtKind::Revert(..) => out.push(Stmt {
                pos,
                kind: StmtKind::Revert,
            }),
            ast::StmtKind::Try(t) => {
                self.push_expr(t.expr, out);
                for clause in t.clauses.iter() {
                    self.block(&clause.block, out);
                }
            }
            ast::StmtKind::Assembly(_) => out.push(Stmt {
                pos,
                kind: StmtKind::Assembly,
            }),
            ast::StmtKind::Placeholder => out.push(Stmt {
                pos,
                kind: StmtKind::Placeholder,
            }),
            ast::StmtKind::Break | ast::StmtKind::Continue => {}
        }
    }

    /// Returns `(positional or named argument values, call options)`. Named
    /// arguments are kept in written order; the rules only need their values.
    fn call_args(&self, args: &ast::CallArgs<'_>) -> (Vec<Expr>, Vec<(String, Expr)>) {
        let values = match &args.kind {
            ast::CallArgsKind::Unnamed(items) => items.iter().map(|e| self.expr(e)).collect(),
            ast::CallArgsKind::Named(items) => {
                items.iter().map(|arg| self.expr(arg.value)).collect()
            }
        };
        (values, Vec::new())
    }

    fn expr(&self, e: &ast::Expr<'_>) -> Expr {
        let kind = match &e.kind {
            ast::ExprKind::Ident(id) => ExprKind::Ident(id.as_str().to_string()),
            ast::ExprKind::Member(base, id) => {
                ExprKind::Member(Box::new(self.expr(base)), id.as_str().to_string())
            }
            ast::ExprKind::Call(callee, args) => {
                let (args, _) = self.call_args(args);
                match &callee.kind {
                    ast::ExprKind::CallOptions(inner, named) => ExprKind::Call {
                        callee: Box::new(self.expr(inner)),
                        args,
                        options: named
                            .iter()
                            .map(|arg| (arg.name.as_str().to_string(), self.expr(arg.value)))
                            .collect(),
                    },
                    _ => ExprKind::Call {
                        callee: Box::new(self.expr(callee)),
                        args,
                        options: Vec::new(),
                    },
                }
            }
            ast::ExprKind::CallOptions(inner, named) => {
                let mut items = vec![self.expr(inner)];
                items.extend(named.iter().map(|arg| self.expr(arg.value)));
                ExprKind::Other(items)
            }
            ast::ExprKind::New(ty) => ExprKind::New(self.text(ty.span)),
            ast::ExprKind::Lit(lit, _) => ExprKind::Lit(match &lit.kind {
                LitKind::Number(_) | LitKind::Rational(_) => Lit::Number,
                LitKind::Bool(b) => Lit::Bool(*b),
                LitKind::Str(..) => Lit::Str(lit.symbol.as_str().to_string()),
                LitKind::Address(_) => Lit::Address,
                LitKind::Err(_) => Lit::Other,
            }),
            ast::ExprKind::Index(base, idx) => {
                let idx = match idx {
                    ast::IndexKind::Index(Some(i)) => vec![self.expr(i)],
                    ast::IndexKind::Index(None) => vec![],
                    ast::IndexKind::Range(a, b) => {
                        a.iter().chain(b.iter()).map(|e| self.expr(e)).collect()
                    }
                };
                ExprKind::Index(Box::new(self.expr(base)), idx)
            }
            ast::ExprKind::Binary(l, op, r) => ExprKind::Binary(
                Box::new(self.expr(l)),
                op.kind.to_str().to_string(),
                Box::new(self.expr(r)),
            ),
            ast::ExprKind::Unary(op, inner) => {
                ExprKind::Unary(op.kind.to_str().to_string(), Box::new(self.expr(inner)))
            }
            ast::ExprKind::Delete(inner) => {
                ExprKind::Unary("delete".to_string(), Box::new(self.expr(inner)))
            }
            ast::ExprKind::Assign(l, _, r) => {
                ExprKind::Assign(Box::new(self.expr(l)), Box::new(self.expr(r)))
            }
            ast::ExprKind::Ternary(c, a, b) => ExprKind::Ternary(
                Box::new(self.expr(c)),
                Box::new(self.expr(a)),
                Box::new(self.expr(b)),
            ),
            ast::ExprKind::Tuple(items) => ExprKind::Tuple(
                items
                    .iter()
                    .filter_map(|slot| match slot {
                        solar_parse::interface::SpannedOption::Some(e) => Some(self.expr(e)),
                        solar_parse::interface::SpannedOption::None(_) => None,
                    })
                    .collect(),
            ),
            ast::ExprKind::Array(items) => {
                ExprKind::Array(items.iter().map(|e| self.expr(e)).collect())
            }
            ast::ExprKind::TypeCall(ty) => ExprKind::Type(format!("type({})", self.text(ty.span))),
            ast::ExprKind::Type(ty) => ExprKind::Type(self.text(ty.span)),
            ast::ExprKind::Payable(args) => ExprKind::Call {
                callee: Box::new(Expr {
                    kind: ExprKind::Type("payable".to_string()),
                    text: "payable".to_string(),
                    pos: self.pos(e.span),
                }),
                args: self.call_args(args).0,
                options: Vec::new(),
            },
            ast::ExprKind::Err(_) => ExprKind::Other(Vec::new()),
        };
        Expr {
            kind,
            text: self.text(e.span),
            pos: self.pos(e.span),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lowers_contract_functions_and_calls() {
        let src = r#"
            pragma solidity ^0.8.0;
            contract T is Base, Test {
                Vault vault;
                function setUp() public { vault = new Vault(); }
                function test_x() external {
                    vm.prank(alice);
                    vault.deposit{value: 1 ether}(1);
                    assertEq(vault.balanceOf(alice), 1);
                }
            }
        "#;
        let unit = parse_source("t.sol", src);
        assert!(unit.errors.is_empty(), "{:?}", unit.errors);
        let c = &unit.contracts[0];
        assert_eq!(c.name, "T");
        assert_eq!(c.bases, vec!["Base", "Test"]);
        assert_eq!(c.vars[0].name, "vault");
        assert_eq!(c.vars[0].ty, "Vault");
        let test = c.function("test_x").unwrap();
        assert_eq!(test.visibility.as_deref(), Some("external"));
        let body = test.body.as_ref().unwrap();
        assert_eq!(body.len(), 3);
        let StmtKind::Expr(call) = &body[1].kind else {
            panic!("expected an expression statement")
        };
        let ExprKind::Call { options, args, .. } = &call.kind else {
            panic!("expected a call")
        };
        assert_eq!(options[0].0, "value");
        assert_eq!(args.len(), 1);
        assert_eq!(body[1].pos.line, 8);
    }

    #[test]
    fn malformed_source_reports_errors_without_panicking() {
        let unit = parse_source("bad.sol", "contract { function ( }");
        assert!(!unit.errors.is_empty());
        for e in &unit.errors {
            assert!(e.starts_with("error"), "{e}");
            assert!(e.contains("(line 1, column"), "{e}");
            assert!(!e.contains('\n'), "{e}");
        }
    }

    #[test]
    fn diagnostics_keep_only_headline_and_position() {
        let rendered = "error: unexpected token\n    ╭▸ <a.sol>:111:51\n    │\n111 │ x\n    ╰ note: created at /somewhere/on/a/build/host/errors.rs:92:24\nerror: second\n  --> <a.sol>:3:4\n";
        assert_eq!(
            summarize_diagnostics(rendered),
            vec![
                "error: unexpected token (line 111, column 51)",
                "error: second (line 3, column 4)"
            ]
        );
    }

    #[test]
    fn modifiers_and_placeholders_are_kept() {
        let src =
            "contract T { modifier m() { a(); _; b(); } function test_x() public m { c(); } }";
        let unit = parse_source("t.sol", src);
        let c = &unit.contracts[0];
        assert_eq!(c.function("test_x").unwrap().modifiers, vec!["m"]);
        let m = c.function("m").unwrap().body.clone().unwrap();
        assert!(matches!(m[1].kind, StmtKind::Placeholder));
    }

    #[test]
    fn control_flow_is_flattened_in_source_order() {
        let src = r#"
            contract T {
                function f() public {
                    if (a()) { b(); } else { c(); }
                    for (uint i; i < 2; i++) { d(); }
                    try x.y() { e(); } catch { g(); }
                }
            }
        "#;
        let unit = parse_source("t.sol", src);
        let body = unit.contracts[0]
            .function("f")
            .unwrap()
            .body
            .clone()
            .unwrap();
        let texts: Vec<String> = body
            .iter()
            .filter_map(|s| match &s.kind {
                StmtKind::Expr(e) => Some(e.text.clone()),
                StmtKind::Decl { .. } => Some("decl".into()),
                _ => None,
            })
            .collect();
        assert_eq!(
            texts,
            vec![
                "a()", "b()", "c()", "decl", "i < 2", "d()", "i++", "x.y()", "e()", "g()"
            ]
        );
    }
}
