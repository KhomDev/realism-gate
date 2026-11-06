//! Lowers a Rust source file, parsed with `syn`, into the model in
//! [`super::ir`].
//!
//! `syn` is the parser the Rust ecosystem's procedural macros are built on.
//! It reads a whole file into a full syntax tree, and with `proc-macro2`'s
//! span locations every node carries its line, column and byte range, so a
//! finding points at the exact call and quotes it as written.

use super::ir::{File, Function};
use crate::ir::{Expr, ExprKind, Lit, Pos, Stmt, StmtKind, Unit};
use proc_macro2::Span;
use syn::parse::ParseStream;
use syn::spanned::Spanned;

/// Macros that make a test fail outright. Inside `if cond { ... }` they turn
/// the `if` into an assertion that `cond` does not hold.
const FAILING_MACROS: &[&str] = &["panic", "unreachable", "todo", "unimplemented"];

/// Parses one Rust source. Never panics on malformed input: a file that does
/// not parse yields no functions and one entry in [`Unit::errors`].
pub fn parse_source(path: &str, source: &str) -> Unit {
    // `syn` drops a byte-order mark before parsing; slice the same text so
    // byte ranges line up.
    let text = source.strip_prefix('\u{feff}').unwrap_or(source);
    let (file, errors) = match syn::parse_file(text) {
        Ok(ast) => (Lower { src: text }.file(&ast), Vec::new()),
        Err(e) => {
            let at = e.span().start();
            let message = format!("error: {e} (line {}, column {})", at.line, at.column + 1);
            (File::default(), vec![message])
        }
    };
    // Spans live in a thread-local source map that grows with every parse.
    // Nothing holds a span past this point.
    proc_macro2::extra::invalidate_current_thread_spans();
    Unit {
        path: path.to_string(),
        source: source.to_string(),
        contracts: Vec::new(),
        rust: Some(file),
        errors,
    }
}

struct Lower<'s> {
    src: &'s str,
}

/// An expression that stands for nothing: the value of a loop, a `return`, or
/// a block without a tail.
fn nothing(pos: Pos) -> Expr {
    Expr {
        kind: ExprKind::Other(Vec::new()),
        text: String::new(),
        pos,
    }
}

fn is_nothing(e: &Expr) -> bool {
    matches!(&e.kind, ExprKind::Other(items) if items.is_empty())
}

fn path_string(path: &syn::Path) -> String {
    path.segments
        .iter()
        .map(|s| s.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}

fn last_segment(path: &syn::Path) -> String {
    path.segments
        .last()
        .map(|s| s.ident.to_string())
        .unwrap_or_default()
}

fn has_attr(attrs: &[syn::Attribute], pred: impl Fn(&str) -> bool) -> bool {
    attrs.iter().any(|a| pred(&last_segment(a.path())))
}

/// Comma- or semicolon-separated expressions, as far as they parse. The rest
/// of a macro body that is not an expression list (`matches!` patterns,
/// custom syntax) is skipped rather than failing the file.
fn macro_args(input: ParseStream<'_>) -> syn::Result<Vec<syn::Expr>> {
    let mut out = Vec::new();
    while !input.is_empty() {
        match input.parse::<syn::Expr>() {
            Ok(e) => out.push(e),
            Err(_) => break,
        }
        if input.parse::<syn::Token![,]>().is_err() && input.parse::<syn::Token![;]>().is_err() {
            break;
        }
    }
    input.step(|cursor| {
        let mut rest = *cursor;
        while let Some((_, next)) = rest.token_tree() {
            rest = next;
        }
        Ok(((), rest))
    })?;
    Ok(out)
}

/// Whether running this block always fails the test.
fn block_fails(block: &syn::Block) -> bool {
    block.stmts.iter().any(|s| {
        let mac = match s {
            syn::Stmt::Macro(m) => Some(&m.mac),
            syn::Stmt::Expr(syn::Expr::Macro(m), _) => Some(&m.mac),
            _ => None,
        };
        mac.is_some_and(|m| FAILING_MACROS.contains(&last_segment(&m.path).as_str()))
    })
}

impl Lower<'_> {
    fn pos(&self, span: Span) -> Pos {
        let at = span.start();
        Pos {
            line: at.line as u32,
            col: at.column as u32 + 1,
        }
    }

    fn text(&self, span: Span) -> String {
        self.src
            .get(span.byte_range())
            .unwrap_or_default()
            .to_string()
    }

    /// The source from the start of `first` to the end of `last`.
    fn text_between(&self, first: Span, last: Span) -> String {
        let (a, b) = (first.byte_range(), last.byte_range());
        self.src.get(a.start..b.end).unwrap_or_default().to_string()
    }

    fn file(&self, f: &syn::File) -> File {
        let mut out = File::default();
        self.items(&f.items, "", &mut out);
        out
    }

    fn items(&self, items: &[syn::Item], module: &str, out: &mut File) {
        for item in items {
            match item {
                syn::Item::Fn(f) => {
                    let start = f.attrs.first().map_or(f.sig.fn_token.span, |a| a.span());
                    out.functions
                        .push(self.function(&f.attrs, &f.sig, &f.block, None, module, start));
                }
                syn::Item::Impl(imp) => {
                    let owner = match &*imp.self_ty {
                        syn::Type::Path(p) => last_segment(&p.path),
                        other => self.text(other.span()),
                    };
                    for ii in &imp.items {
                        if let syn::ImplItem::Fn(m) = ii {
                            let start = m.attrs.first().map_or(m.sig.fn_token.span, |a| a.span());
                            out.functions.push(self.function(
                                &m.attrs,
                                &m.sig,
                                &m.block,
                                Some(owner.clone()),
                                module,
                                start,
                            ));
                        }
                    }
                }
                syn::Item::Mod(m) => {
                    if let Some((_, inner)) = &m.content {
                        let path = if module.is_empty() {
                            m.ident.to_string()
                        } else {
                            format!("{module}::{}", m.ident)
                        };
                        self.items(inner, &path, out);
                    }
                }
                syn::Item::Const(c) => {
                    let value = self.expr(&c.expr, &mut Vec::new());
                    out.consts.push((c.ident.to_string(), value));
                }
                syn::Item::Static(s) => {
                    let value = self.expr(&s.expr, &mut Vec::new());
                    out.consts.push((s.ident.to_string(), value));
                }
                _ => {}
            }
        }
    }

    fn function(
        &self,
        attrs: &[syn::Attribute],
        sig: &syn::Signature,
        block: &syn::Block,
        owner: Option<String>,
        module: &str,
        start: Span,
    ) -> Function {
        let mut method = false;
        let mut params = Vec::new();
        for input in &sig.inputs {
            match input {
                syn::FnArg::Receiver(_) => method = true,
                syn::FnArg::Typed(pt) => params.push(match &*pt.pat {
                    syn::Pat::Ident(id) => id.ident.to_string(),
                    _ => String::new(),
                }),
            }
        }
        let mut body = Vec::new();
        for s in &block.stmts {
            self.stmt(s, &mut body);
        }
        Function {
            name: sig.ident.to_string(),
            owner,
            method,
            module: module.to_string(),
            test: has_attr(attrs, |a| matches!(a, "test" | "rstest" | "test_case")),
            should_panic: has_attr(attrs, |a| a == "should_panic"),
            params,
            body,
            pos: self.pos(start),
        }
    }

    fn stmt(&self, s: &syn::Stmt, out: &mut Vec<Stmt>) {
        match s {
            syn::Stmt::Local(local) => {
                let mut vars = Vec::new();
                self.pattern(&local.pat, "", &mut vars);
                let pos = self.pos(local.span());
                let init = local.init.as_ref().map(|i| self.expr(&i.expr, out));
                out.push(Stmt {
                    kind: StmtKind::Decl { vars, init },
                    pos,
                });
                if let Some((_, diverge)) = local.init.as_ref().and_then(|i| i.diverge.as_ref()) {
                    self.expr_stmt(diverge, out);
                }
            }
            syn::Stmt::Expr(e, _) => self.expr_stmt(e, out),
            syn::Stmt::Macro(m) => {
                let e = self.mac(&m.mac, m.span());
                out.push(Stmt {
                    pos: e.pos,
                    kind: StmtKind::Expr(e),
                });
            }
            syn::Stmt::Item(_) => {}
        }
    }

    fn expr_stmt(&self, e: &syn::Expr, out: &mut Vec<Stmt>) {
        let lowered = self.expr(e, out);
        if !is_nothing(&lowered) {
            out.push(Stmt {
                pos: lowered.pos,
                kind: StmtKind::Expr(lowered),
            });
        }
    }

    /// The names a pattern binds, each with the type written next to it.
    fn pattern(&self, pat: &syn::Pat, ty: &str, out: &mut Vec<(String, String)>) {
        match pat {
            syn::Pat::Ident(id) => {
                out.push((id.ident.to_string(), ty.to_string()));
                if let Some((_, sub)) = &id.subpat {
                    self.pattern(sub, ty, out);
                }
            }
            syn::Pat::Type(pt) => self.pattern(&pt.pat, &self.text(pt.ty.span()), out),
            syn::Pat::Reference(r) => self.pattern(&r.pat, ty, out),
            syn::Pat::Tuple(t) => t.elems.iter().for_each(|p| self.pattern(p, "", out)),
            syn::Pat::TupleStruct(t) => t.elems.iter().for_each(|p| self.pattern(p, "", out)),
            syn::Pat::Slice(s) => s.elems.iter().for_each(|p| self.pattern(p, "", out)),
            syn::Pat::Struct(s) => s.fields.iter().for_each(|f| self.pattern(&f.pat, "", out)),
            syn::Pat::Paren(p) => self.pattern(&p.pat, ty, out),
            _ => {}
        }
    }

    /// Lowers an expression. Statements nested inside it (the bodies of
    /// blocks, branches, loops and closures) are appended to `pre`, so they
    /// run before the statement that holds the expression.
    fn expr(&self, e: &syn::Expr, pre: &mut Vec<Stmt>) -> Expr {
        let span = e.span();
        let pos = self.pos(span);
        let boxed = |x: Expr| Box::new(x);
        let kind = match e {
            syn::Expr::Path(p) => ExprKind::Ident(path_string(&p.path)),
            syn::Expr::Lit(l) => ExprKind::Lit(match &l.lit {
                syn::Lit::Int(_) | syn::Lit::Float(_) => Lit::Number,
                syn::Lit::Bool(b) => Lit::Bool(b.value),
                syn::Lit::Str(s) => Lit::Str(s.value()),
                _ => Lit::Other,
            }),
            syn::Expr::MethodCall(m) => {
                let receiver = self.expr(&m.receiver, pre);
                let callee = Expr {
                    text: self.text_between(m.receiver.span(), m.method.span()),
                    pos: receiver.pos,
                    kind: ExprKind::Member(boxed(receiver), m.method.to_string()),
                };
                let args = m.args.iter().map(|a| self.expr(a, pre)).collect();
                ExprKind::Call {
                    callee: boxed(callee),
                    args,
                    options: Vec::new(),
                }
            }
            syn::Expr::Call(c) => ExprKind::Call {
                callee: boxed(self.expr(&c.func, pre)),
                args: c.args.iter().map(|a| self.expr(a, pre)).collect(),
                options: Vec::new(),
            },
            syn::Expr::Field(f) => {
                let member = match &f.member {
                    syn::Member::Named(id) => id.to_string(),
                    syn::Member::Unnamed(i) => i.index.to_string(),
                };
                ExprKind::Member(boxed(self.expr(&f.base, pre)), member)
            }
            syn::Expr::Macro(m) => return self.mac(&m.mac, span),
            syn::Expr::Struct(s) => {
                let callee = Expr {
                    kind: ExprKind::Type(path_string(&s.path)),
                    text: self.text(s.path.span()),
                    pos: self.pos(s.path.span()),
                };
                let mut options: Vec<(String, Expr)> = s
                    .fields
                    .iter()
                    .map(|f| {
                        let name = match &f.member {
                            syn::Member::Named(id) => id.to_string(),
                            syn::Member::Unnamed(i) => i.index.to_string(),
                        };
                        (name, self.expr(&f.expr, pre))
                    })
                    .collect();
                if let Some(rest) = &s.rest {
                    options.push(("..".to_string(), self.expr(rest, pre)));
                }
                ExprKind::Call {
                    callee: boxed(callee),
                    args: Vec::new(),
                    options,
                }
            }
            syn::Expr::Reference(r) => {
                ExprKind::Unary("&".to_string(), boxed(self.expr(&r.expr, pre)))
            }
            syn::Expr::Unary(u) => {
                let op = match u.op {
                    syn::UnOp::Deref(_) => "*",
                    syn::UnOp::Not(_) => "!",
                    syn::UnOp::Neg(_) => "-",
                    _ => "?",
                };
                ExprKind::Unary(op.to_string(), boxed(self.expr(&u.expr, pre)))
            }
            syn::Expr::Paren(p) => return self.expr(&p.expr, pre),
            syn::Expr::Group(g) => return self.expr(&g.expr, pre),
            syn::Expr::Try(t) => return self.expr(&t.expr, pre),
            syn::Expr::Await(a) => return self.expr(&a.base, pre),
            syn::Expr::Cast(c) => return self.expr(&c.expr, pre),
            syn::Expr::Binary(b) => {
                let (l, r) = (self.expr(&b.left, pre), self.expr(&b.right, pre));
                let op = self.text(b.op.span());
                if op.len() >= 2
                    && op.ends_with('=')
                    && !matches!(op.as_str(), "==" | "!=" | "<=" | ">=")
                {
                    // `a += b`: `a` now depends on `b`.
                    ExprKind::Assign(boxed(l), boxed(r))
                } else {
                    ExprKind::Binary(boxed(l), op, boxed(r))
                }
            }
            syn::Expr::Assign(a) => ExprKind::Assign(
                boxed(self.expr(&a.left, pre)),
                boxed(self.expr(&a.right, pre)),
            ),
            syn::Expr::Index(i) => ExprKind::Index(
                boxed(self.expr(&i.expr, pre)),
                vec![self.expr(&i.index, pre)],
            ),
            syn::Expr::Tuple(t) => {
                ExprKind::Tuple(t.elems.iter().map(|x| self.expr(x, pre)).collect())
            }
            syn::Expr::Array(a) => {
                ExprKind::Array(a.elems.iter().map(|x| self.expr(x, pre)).collect())
            }
            syn::Expr::Repeat(r) => ExprKind::Array(vec![self.expr(&r.expr, pre)]),
            syn::Expr::Block(b) => return self.inline_block(&b.block, pre, pos),
            syn::Expr::Unsafe(b) => return self.inline_block(&b.block, pre, pos),
            syn::Expr::Async(b) => return self.inline_block(&b.block, pre, pos),
            syn::Expr::Const(b) => return self.inline_block(&b.block, pre, pos),
            syn::Expr::TryBlock(b) => return self.inline_block(&b.block, pre, pos),
            syn::Expr::If(i) => return self.if_expr(i, pre, span),
            syn::Expr::Match(m) => {
                self.expr_stmt(&m.expr, pre);
                let mut tails = Vec::new();
                for arm in &m.arms {
                    if let Some((_, guard)) = &arm.guard {
                        self.expr_stmt(guard, pre);
                    }
                    let tail = self.expr(&arm.body, pre);
                    if !is_nothing(&tail) {
                        tails.push(tail);
                    }
                }
                ExprKind::Other(tails)
            }
            syn::Expr::While(w) => {
                self.condition(&w.cond, pre);
                self.flatten(&w.body, pre);
                ExprKind::Other(Vec::new())
            }
            syn::Expr::ForLoop(f) => {
                let iterable = self.expr(&f.expr, pre);
                let mut vars = Vec::new();
                self.pattern(&f.pat, "", &mut vars);
                pre.push(Stmt {
                    pos: iterable.pos,
                    kind: StmtKind::Decl {
                        vars,
                        init: Some(iterable),
                    },
                });
                self.flatten(&f.body, pre);
                ExprKind::Other(Vec::new())
            }
            syn::Expr::Loop(l) => {
                self.flatten(&l.body, pre);
                ExprKind::Other(Vec::new())
            }
            syn::Expr::Closure(c) => {
                let body = self.expr(&c.body, pre);
                ExprKind::Other(if is_nothing(&body) {
                    Vec::new()
                } else {
                    vec![body]
                })
            }
            syn::Expr::Return(r) => {
                let value = r.expr.as_ref().map(|x| self.expr(x, pre));
                pre.push(Stmt {
                    kind: StmtKind::Return(value),
                    pos,
                });
                return nothing(pos);
            }
            syn::Expr::Let(l) => {
                self.let_condition(l, pre);
                return nothing(pos);
            }
            syn::Expr::Range(r) => ExprKind::Other(
                r.start
                    .iter()
                    .chain(r.end.iter())
                    .map(|x| self.expr(x, pre))
                    .collect(),
            ),
            _ => ExprKind::Other(Vec::new()),
        };
        Expr {
            kind,
            text: self.text(span),
            pos,
        }
    }

    /// A macro call: the callee is `name!`, the arguments are its
    /// comma-separated expressions.
    fn mac(&self, m: &syn::Macro, span: Span) -> Expr {
        let name = format!("{}!", last_segment(&m.path));
        let parsed = m.parse_body_with(macro_args).unwrap_or_default();
        let mut pre = Vec::new();
        let args: Vec<Expr> = parsed.iter().map(|a| self.expr(a, &mut pre)).collect();
        let callee = Expr {
            kind: ExprKind::Ident(name),
            text: self.text(m.path.span()),
            pos: self.pos(m.path.span()),
        };
        let call = Expr {
            kind: ExprKind::Call {
                callee: Box::new(callee),
                args,
                options: Vec::new(),
            },
            text: self.text(span),
            pos: self.pos(span),
        };
        if pre.is_empty() {
            return call;
        }
        // Blocks inside a macro's arguments are rare; keep their calls.
        let mut items: Vec<Expr> = pre
            .into_iter()
            .filter_map(|s| match s.kind {
                StmtKind::Expr(e) => Some(e),
                StmtKind::Decl { init, .. } => init,
                _ => None,
            })
            .collect();
        items.push(call);
        Expr {
            kind: ExprKind::Other(items),
            text: self.text(span),
            pos: self.pos(span),
        }
    }

    /// The statements of a block, inline; the value is its tail expression.
    fn inline_block(&self, block: &syn::Block, pre: &mut Vec<Stmt>, pos: Pos) -> Expr {
        let Some((last, init)) = block.stmts.split_last() else {
            return nothing(pos);
        };
        for s in init {
            self.stmt(s, pre);
        }
        match last {
            syn::Stmt::Expr(e, None) => self.expr(e, pre),
            other => {
                self.stmt(other, pre);
                nothing(pos)
            }
        }
    }

    fn flatten(&self, block: &syn::Block, pre: &mut Vec<Stmt>) {
        for s in &block.stmts {
            self.stmt(s, pre);
        }
    }

    /// The condition of an `if` or `while`: an expression, or `let PAT = x`.
    fn condition(&self, cond: &syn::Expr, pre: &mut Vec<Stmt>) {
        match cond {
            syn::Expr::Let(l) => self.let_condition(l, pre),
            other => self.expr_stmt(other, pre),
        }
    }

    fn let_condition(&self, l: &syn::ExprLet, pre: &mut Vec<Stmt>) {
        let init = self.expr(&l.expr, pre);
        let mut vars = Vec::new();
        self.pattern(&l.pat, "", &mut vars);
        pre.push(Stmt {
            pos: init.pos,
            kind: StmtKind::Decl {
                vars,
                init: Some(init),
            },
        });
    }

    fn if_expr(&self, i: &syn::ExprIf, pre: &mut Vec<Stmt>, span: Span) -> Expr {
        let pos = self.pos(span);
        // `if cond { panic!(..) }` fails the test exactly when `cond` holds:
        // it is `assert!(!cond)`, written longhand.
        if i.else_branch.is_none()
            && !matches!(&*i.cond, syn::Expr::Let(_))
            && block_fails(&i.then_branch)
        {
            let cond = self.expr(&i.cond, pre);
            let negated = Expr {
                text: format!("!({})", cond.text),
                pos: cond.pos,
                kind: ExprKind::Unary("!".to_string(), Box::new(cond)),
            };
            return Expr {
                kind: ExprKind::Call {
                    callee: Box::new(Expr {
                        kind: ExprKind::Ident("assert!".to_string()),
                        text: "if".to_string(),
                        pos,
                    }),
                    args: vec![negated],
                    options: Vec::new(),
                },
                text: self.text(span),
                pos,
            };
        }
        self.condition(&i.cond, pre);
        let mut tails = Vec::new();
        let then = self.inline_block(&i.then_branch, pre, pos);
        if !is_nothing(&then) {
            tails.push(then);
        }
        if let Some((_, other)) = &i.else_branch {
            let tail = self.expr(other, pre);
            if !is_nothing(&tail) {
                tails.push(tail);
            }
        }
        Expr {
            kind: ExprKind::Other(tails),
            text: self.text(span),
            pos,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(src: &str) -> File {
        let unit = parse_source("t.rs", src);
        assert!(unit.errors.is_empty(), "{:?}", unit.errors);
        unit.rust.unwrap()
    }

    fn calls(e: &Expr, out: &mut Vec<String>) {
        for c in e.children() {
            calls(c, out);
        }
        if let ExprKind::Call { callee, .. } = &e.kind {
            out.push(match &callee.kind {
                ExprKind::Member(_, m) => format!(".{m}"),
                ExprKind::Ident(n) | ExprKind::Type(n) => n.clone(),
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

    #[test]
    fn finds_tests_helpers_methods_and_modules() {
        let f = parse(
            r#"
            const POOL: Pubkey = pubkey!("11111111111111111111111111111111");
            fn setup() -> LiteSVM { LiteSVM::new() }
            struct Env;
            impl Env { fn attack(&mut self, amount: u64) {} }
            #[cfg(test)]
            mod tests {
                #[tokio::test]
                #[should_panic]
                async fn test_drain() {}
            }
        "#,
        );
        let names: Vec<(&str, &str, bool)> = f
            .functions
            .iter()
            .map(|x| (x.name.as_str(), x.module.as_str(), x.test))
            .collect();
        assert_eq!(
            names,
            vec![
                ("setup", "", false),
                ("attack", "", false),
                ("test_drain", "tests", true)
            ]
        );
        assert_eq!(f.functions[1].owner.as_deref(), Some("Env"));
        assert!(f.functions[1].method);
        assert_eq!(f.functions[1].params, vec!["amount"]);
        assert!(f.functions[2].should_panic);
        assert_eq!(f.consts[0].0, "POOL");
    }

    #[test]
    fn positions_and_text_are_exact() {
        let src = "fn t() {\n    let x = svm.get_balance(&attacker.pubkey()).unwrap();\n}\n";
        let f = parse(src);
        let StmtKind::Decl {
            vars,
            init: Some(init),
        } = &f.functions[0].body[0].kind
        else {
            panic!("not a declaration");
        };
        assert_eq!(vars, &vec![("x".to_string(), String::new())]);
        assert_eq!(init.text, "svm.get_balance(&attacker.pubkey()).unwrap()");
        assert_eq!(init.pos, Pos { line: 2, col: 13 });
        let ExprKind::Call { callee, .. } = &init.kind else {
            panic!("not a call");
        };
        assert_eq!(callee.text, "svm.get_balance(&attacker.pubkey()).unwrap");
    }

    #[test]
    fn blocks_are_flattened_in_source_order() {
        let f = parse(
            r#"
            fn t() {
                let v = { a(); b() };
                if cond() { c(); } else { d(); }
                match e() { Ok(_) => f(), Err(_) => g() }
                for x in h() { i(x); }
                let k = j().await?;
            }
        "#,
        );
        assert_eq!(
            body_calls(&f.functions[0]),
            vec!["a", "b", "cond", "c", "d", "e", "f", "g", "h", "i", "j"]
        );
    }

    #[test]
    fn macros_become_calls_with_their_arguments() {
        let f = parse(
            "fn t() { assert_eq!(a.balance(), 5, \"msg {}\", x); matches!(r, Ok(_)); vec![0u8; 32]; }",
        );
        let StmtKind::Expr(e) = &f.functions[0].body[0].kind else {
            panic!()
        };
        let ExprKind::Call { callee, args, .. } = &e.kind else {
            panic!()
        };
        assert_eq!(callee.as_ident(), Some("assert_eq!"));
        assert_eq!(args.len(), 4);
        assert_eq!(args[0].text, "a.balance()");
        assert_eq!(
            body_calls(&f.functions[0]),
            vec![".balance", "assert_eq!", "Ok", "matches!", "vec!"]
        );
    }

    #[test]
    fn struct_literals_keep_their_fields() {
        let f = parse(
            "fn t() { svm.set_account(vault, Account { lamports: 5, data, ..Account::default() }); }",
        );
        let StmtKind::Expr(e) = &f.functions[0].body[0].kind else {
            panic!()
        };
        let ExprKind::Call { args, .. } = &e.kind else {
            panic!()
        };
        let ExprKind::Call {
            callee, options, ..
        } = &args[1].kind
        else {
            panic!()
        };
        assert_eq!(callee.kind, ExprKind::Type("Account".to_string()));
        let fields: Vec<&str> = options.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(fields, vec!["lamports", "data", ".."]);
    }

    #[test]
    fn if_panic_is_an_assertion() {
        let f = parse("fn t() { if after <= before { panic!(\"no profit\"); } }");
        assert_eq!(body_calls(&f.functions[0]), vec!["assert!"]);
        let StmtKind::Expr(e) = &f.functions[0].body[0].kind else {
            panic!()
        };
        let ExprKind::Call { args, .. } = &e.kind else {
            panic!()
        };
        assert!(matches!(&args[0].kind, ExprKind::Unary(op, _) if op == "!"));
    }

    #[test]
    fn a_file_that_does_not_parse_is_reported_not_dropped() {
        let unit = parse_source("bad.rs", "fn t( {");
        assert_eq!(unit.errors.len(), 1);
        assert!(unit.errors[0].contains("(line 1"), "{:?}", unit.errors);
        assert!(unit.rust.unwrap().functions.is_empty());
    }

    #[test]
    fn spans_do_not_leak_between_files() {
        // A second parse on the same thread starts its positions at line 1.
        parse("fn a() {}\n\n\n");
        let f = parse("fn b() { x(); }");
        assert_eq!(f.functions[0].pos, Pos { line: 1, col: 1 });
    }
}
