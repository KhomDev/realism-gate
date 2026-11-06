//! The rules, as they apply to Solana program tests.
//!
//! The rule ids are the Solidity ones: each rule is a way a passing PoC
//! proves less than it claims, and that does not depend on the language. What
//! changes is the evidence. `vm.store` becomes `set_account` on a program's
//! account, `vm.prank(admin)` becomes a transaction `admin` signs, an
//! unpinned fork becomes a test that reads accounts from a live RPC.
//!
//! RG07 (mocked calls) has no Solana counterpart and is not checked here.

use super::env::{Class, Env};
use super::harness::Event;
use super::ir::Dialect;
use super::trace::{Call, CallClass, Op, OpKind, Trace, key};
use crate::finding::Severity;
use crate::identity::{compact, is_mock_name, words};
use crate::ir::{Expr, ExprKind, Lit};
use crate::rules::{Raw, snippet};
use std::collections::{BTreeMap, BTreeSet};

/// Words in an instruction that say it builds the world.
const SETUP_TX_WORDS: &[&str] = &[
    "init",
    "initialize",
    "initialise",
    "create",
    "setup",
    "configure",
    "deploy",
    "bootstrap",
    "prepare",
    "fund",
    "mint",
];

/// One transaction in the trace.
struct Tx<'p> {
    op: usize,
    signers: Vec<String>,
    instructions: Vec<&'p Expr>,
}

/// Everything a rule can look at for one test.
pub struct Ctx<'p, 'a> {
    pub trace: &'a Trace<'p>,
    pub env: &'a Env<'p>,
    /// The harness event of each op, where it has one.
    events: Vec<Option<Event<'p>>>,
    txs: Vec<Tx<'p>>,
    /// The first op of the exploit. Everything before it builds the world.
    exploit_start: Option<usize>,
}

/// Words of every function, method and type an instruction expression
/// names. A local standing for the whole instruction is followed to its
/// value (`ix` assigned from `init_pool_ix(..)`), and so are the instructions
/// a transaction is built from (`new Transaction().add(ix)`); names inside
/// the instruction's arguments are not, so `&admin` does not bring in the
/// `setup()` that returned `admin`.
fn instruction_words(env: &Env<'_>, e: &Expr, out: &mut BTreeSet<String>, follow: bool) {
    match &e.kind {
        ExprKind::Call { callee, args, .. } => {
            let name = match &callee.kind {
                ExprKind::Ident(p) | ExprKind::Type(p) => p.rsplit("::").next().unwrap_or(p),
                ExprKind::Member(_, m) => m.as_str(),
                _ => "",
            };
            out.extend(words(name));
            if name == "add" {
                for c in callee.children() {
                    instruction_words(env, c, out, false);
                }
                for a in args {
                    instruction_words(env, a, out, true);
                }
                return;
            }
        }
        ExprKind::Ident(name) if follow => {
            for v in env.assigns.get(name).into_iter().flatten() {
                instruction_words(env, v, out, false);
            }
        }
        ExprKind::Unary(_, inner) if follow => return instruction_words(env, inner, out, true),
        _ => {}
    }
    for c in e.children() {
        instruction_words(env, c, out, false);
    }
}

impl<'p, 'a> Ctx<'p, 'a> {
    pub fn new(trace: &'a Trace<'p>, env: &'a Env<'p>) -> Ctx<'p, 'a> {
        let events: Vec<Option<Event<'p>>> = trace
            .ops
            .iter()
            .map(|op| op.call().and_then(super::harness::event))
            .collect();
        // Each built transaction, and each send whose transaction was not
        // built where the trace can see it.
        let mut txs: Vec<Tx<'p>> = Vec::new();
        let mut built_since_send = false;
        for (i, ev) in events.iter().enumerate() {
            match ev {
                Some(Event::Transaction {
                    signers,
                    implicit,
                    instructions,
                    accounts,
                }) => {
                    let op = &trace.ops[i];
                    let mut keys: Vec<String> = Vec::new();
                    for s in signers {
                        for k in op.keys(s) {
                            if !keys.contains(&k) {
                                keys.push(k);
                            }
                        }
                    }
                    // The provider wallet or client payer signs every request
                    // as the fee payer. It authorizes the instruction only
                    // when it is the only signer or the instruction names it.
                    if let Some(fee_payer) = implicit {
                        let named =
                            accounts.is_some_and(|a| account_keys(op, a).contains(*fee_payer));
                        if (keys.is_empty() || named) && !keys.iter().any(|k| k == fee_payer) {
                            keys.insert(0, fee_payer.to_string());
                        }
                    }
                    txs.push(Tx {
                        op: i,
                        signers: keys,
                        instructions: instructions.iter().flat_map(|e| op.exprs(e)).collect(),
                    });
                    built_since_send = true;
                }
                Some(Event::Send) => {
                    if !built_since_send {
                        txs.push(Tx {
                            op: i,
                            signers: Vec::new(),
                            instructions: Vec::new(),
                        });
                    }
                    built_since_send = false;
                }
                _ => {}
            }
        }
        let mut ctx = Ctx {
            trace,
            env,
            events,
            txs,
            exploit_start: None,
        };
        ctx.exploit_start = ctx.find_exploit_start();
        ctx
    }

    fn op(&self, i: usize) -> &Op<'p> {
        &self.trace.ops[i]
    }

    /// Whether op `i` is part of the exploit rather than its setup. Without
    /// any transaction to anchor it, everything outside a setup helper is.
    fn is_exploit(&self, i: usize) -> bool {
        match self.exploit_start {
            Some(start) => i >= start,
            None => !self.trace.ops[i].in_setup_helper,
        }
    }

    fn phase_text(&self, i: usize) -> &'static str {
        if self.is_exploit(i) {
            "in the exploit"
        } else {
            "while setting up"
        }
    }

    fn by_phase(&self, i: usize, exploit: Severity, setup: Severity) -> Severity {
        if self.is_exploit(i) { exploit } else { setup }
    }

    /// Where the exploit begins: the first transaction that does not build
    /// the world. A transaction builds the world when it is sent from a setup
    /// helper, or when nobody who reads as the attacker signs it and its
    /// instructions say `init`, `create`, `setup` or the like. When every
    /// transaction builds the world, the exploit begins at the attacker's
    /// first one.
    fn find_exploit_start(&self) -> Option<usize> {
        let attacker_signs = |tx: &Tx<'_>| tx.signers.iter().any(|s| self.env.is_attacker(s));
        let builds_world = |tx: &Tx<'_>| {
            self.trace.ops[tx.op].in_setup_helper || (!attacker_signs(tx) && self.says_setup(tx))
        };
        self.txs
            .iter()
            .find(|t| !builds_world(t))
            .or_else(|| self.txs.iter().find(|t| attacker_signs(t)))
            .map(|t| t.op)
    }

    /// Whether a transaction is sent from a setup helper or its instructions
    /// say `init`, `create`, `setup` or the like, whoever signs it.
    fn says_setup(&self, tx: &Tx<'_>) -> bool {
        if self.trace.ops[tx.op].in_setup_helper {
            return true;
        }
        let mut ws = BTreeSet::new();
        for ix in &tx.instructions {
            instruction_words(self.env, ix, &mut ws, true);
        }
        ws.iter().any(|w| SETUP_TX_WORDS.contains(&w.as_str()))
    }
}

/// Runs every Rust rule on one test.
pub fn check_test(ctx: &Ctx<'_, '_>) -> Vec<Raw> {
    let mut out = Vec::new();
    attacker_holds_privilege(ctx, &mut out);
    privileged_signer(ctx, &mut out);
    deployer_is_attacker(ctx, &mut out);
    account_writes(ctx, &mut out);
    code_replacement(ctx, &mut out);
    no_assertion(ctx, &mut out);
    tautological_assertion(ctx, &mut out);
    no_baseline(ctx, &mut out);
    live_rpc(ctx, &mut out);
    out
}

/// A finding about a transaction, placed at the call that sends it: the line
/// in the test, or in the helper the test calls, that names the instruction
/// and the signers, rather than the inside of a generic `send` wrapper.
fn tx_raw(rule: &'static str, severity: Severity, op: &Op<'_>, message: String) -> Raw {
    let (unit, expr) = match (op.site, op.call()) {
        (Some(site), _) => (site.unit, site.expr),
        (None, Some(c)) => (op.unit, c.expr),
        (None, None) => return raw(rule, severity, op, message, ""),
    };
    Raw {
        rule,
        severity,
        unit,
        pos: expr.pos,
        message,
        evidence: snippet(&expr.text, 120),
    }
}

fn raw(
    rule: &'static str,
    severity: Severity,
    op: &Op<'_>,
    message: String,
    evidence: &str,
) -> Raw {
    Raw {
        rule,
        severity,
        unit: op.unit,
        pos: op.pos,
        message,
        evidence: snippet(evidence, 120),
    }
}

/// The identities an accounts struct or object names:
/// `.accounts({ authority: provider.wallet.publicKey })` names `wallet`.
fn account_keys(op: &Op<'_>, accounts: &Expr) -> BTreeSet<String> {
    match &accounts.kind {
        ExprKind::Call { options, .. } => options.iter().map(|(_, v)| op.key(v)).collect(),
        _ => BTreeSet::new(),
    }
}

/// The method as the test spells it: `setAccount` for a TypeScript
/// `svm.setAccount(..)`, which the IR names `set_account`.
fn written_name<'c>(c: &'c Call<'_>) -> &'c str {
    let ExprKind::Call { callee, .. } = &c.expr.kind else {
        return &c.name;
    };
    callee
        .text
        .rsplit(['.', ':'])
        .next()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .unwrap_or(&c.name)
}

/// RG01: the harness gives the attacker a role, or the attacker is a role
/// holder's keypair.
fn attacker_holds_privilege(ctx: &Ctx<'_, '_>, out: &mut Vec<Raw>) {
    let env = ctx.env;
    for g in &env.grants {
        if !env.is_attacker(&g.grantee) {
            continue;
        }
        // The attacker granting itself a role in an ordinary transaction is
        // the attack, an access-control bug, not something the harness did.
        // The grant counts when someone else signs it, or when its
        // transaction sets the program up: an attacker who initializes the
        // program as its own admin is its admin.
        // With no transaction in sight, who grants it is unknown.
        let Some(tx) = ctx.txs.iter().find(|t| t.op >= g.op) else {
            continue;
        };
        if tx.signers.iter().any(|s| env.is_attacker(s)) && !ctx.says_setup(tx) {
            continue;
        }
        let op = ctx.op(g.op);
        let text = op.call().map_or("", |c| c.expr.text.as_str());
        out.push(raw(
            "RG01",
            Severity::Error,
            op,
            format!(
                "The harness makes the attacker `{}` a role holder (`{}`). The PoC then proves what that role can do, not what an outsider can.",
                g.grantee,
                snippet(&g.text, 80)
            ),
            text,
        ));
    }
    for op in &ctx.trace.ops {
        let OpKind::Assign {
            target,
            value: Some(value),
            ..
        } = &op.kind
        else {
            continue;
        };
        if !env.is_attacker(target) || !super::trace::is_identity_shaped(value) {
            continue;
        }
        let source = op.key(value);
        if source == *target || env.is_attacker(&source) {
            continue;
        }
        if let Some(reason) = env.privileged(&source) {
            out.push(raw(
                "RG01",
                Severity::Error,
                op,
                format!(
                    "The attacker `{target}` is the keypair of `{source}` ({reason}). Signing as it proves what that role can do."
                ),
                &value.text,
            ));
        }
    }
}

/// RG02: a transaction in the exploit is signed by a role holder.
fn privileged_signer(ctx: &Ctx<'_, '_>, out: &mut Vec<Raw>) {
    let env = ctx.env;
    for tx in &ctx.txs {
        if !ctx.is_exploit(tx.op) {
            continue;
        }
        let hit = tx.signers.iter().find_map(|s| {
            if env.declared_privileged.contains(s) {
                return None;
            }
            env.privileged(s).map(|why| (s, why))
        });
        let Some((signer, why)) = hit else { continue };
        out.push(tx_raw(
            "RG02",
            Severity::Warn,
            ctx.op(tx.op),
            format!(
                "The exploit sends a transaction signed by `{signer}`: {why}. An attacker cannot produce that signature. Move it before the exploit if it is configuration, or declare it under `privileged:` in realism.yaml if the claim relies on that role acting."
            ),
        ));
    }
}

/// RG03: the exploit is signed only by the keys that built the world.
fn deployer_is_attacker(ctx: &Ctx<'_, '_>, out: &mut Vec<Raw>) {
    let env = ctx.env;
    let builders: BTreeSet<&String> = ctx
        .txs
        .iter()
        .filter(|t| !ctx.is_exploit(t.op))
        .flat_map(|t| &t.signers)
        .collect();
    if builders.is_empty() {
        return;
    }
    for tx in ctx.txs.iter().filter(|t| ctx.is_exploit(t.op)) {
        let only_builders = !tx.signers.is_empty()
            && tx.signers.iter().all(|s| {
                builders.contains(s)
                    && !matches!(env.class_of(s), Class::Attacker | Class::User)
                    && env.privileged(s).is_none()
            });
        if !only_builders {
            continue;
        }
        let op = ctx.op(tx.op);
        let names: Vec<String> = tx.signers.iter().map(|s| format!("`{s}`")).collect();
        out.push(tx_raw(
            "RG03",
            Severity::Warn,
            op,
            format!(
                "This exploit transaction is signed only by {}, which also signed the transactions that set the protocol up. If setting up gave that key any authority, the PoC proves what the deployer can do. Sign the exploit with a separate attacker keypair.",
                names.join(" and ")
            ),
        ));
        return;
    }
}

/// What an account value written with `set_account` is made of.
#[derive(Debug, PartialEq, Eq)]
enum Shape {
    /// Copied from a recorded dump or a live cluster: real state.
    Copied,
    /// Executable: a program.
    Program,
    /// A token account with a balance.
    TokenBalance,
    /// Lamports and no data.
    LamportsOnly,
    /// Anything else: state the test made up.
    State,
}

/// Text that says an account value was copied from a recorded dump, matched
/// without regard to case so that `loadAccount` and `load_account` agree.
const COPY_HINTS: &[&str] = &[
    "include_bytes!",
    "fs::read",
    "read_to_string",
    "readfile",
    "from_file",
    "fromfile",
    "load_account",
    "loadaccount",
    "read_account",
    "readaccount",
    "base64",
    "serde_json",
    "from_json",
    "json.parse",
    "snapshot",
    "dump",
];

/// Whether `e` builds a client for a cluster: `RpcClient::new(..)`,
/// web3.js's `new Connection(..)`.
fn is_cluster_client(e: &Expr) -> bool {
    let ExprKind::Call { callee, .. } = &e.kind else {
        return false;
    };
    callee.as_ident().is_some_and(|p| {
        let mut segs = p.rsplit("::");
        let (last, owner) = (segs.next().unwrap_or(""), segs.next().unwrap_or(""));
        last.starts_with("new") && (owner.ends_with("RpcClient") || owner == "Connection")
    })
}

/// The account value and what it was built from, one assignment deep.
fn account_texts<'p>(env: &Env<'p>, op: &Op<'p>, account: &'p Expr) -> Vec<&'p Expr> {
    let mut out = vec![account];
    let mut names = Vec::new();
    collect_idents(account, &mut names);
    for n in names {
        let resolved = op.name_key(&n);
        for name in [n, resolved] {
            out.extend(env.assigns.get(&name).into_iter().flatten().copied());
        }
    }
    out
}

fn collect_idents(e: &Expr, out: &mut Vec<String>) {
    if let ExprKind::Ident(n) = &e.kind {
        out.push(n.clone());
    }
    for c in e.children() {
        collect_idents(c, out);
    }
}

/// The struct literal or constructor call that builds an `Account`, if one
/// of `parts` is one: `Account { .. }`, `Account::new(l, n, owner)`,
/// `AccountSharedData::new(..)`, or a TypeScript object with account fields
/// (`{ lamports, data, owner, executable }`).
fn account_literal<'e>(parts: &[&'e Expr]) -> Option<&'e Expr> {
    fn find(e: &Expr) -> Option<&Expr> {
        if let ExprKind::Call {
            callee, options, ..
        } = &e.kind
        {
            let is_account = match &callee.kind {
                ExprKind::Type(p) if p.is_empty() => options
                    .iter()
                    .any(|(n, _)| matches!(n.as_str(), "lamports" | "data" | "executable")),
                ExprKind::Type(p) => p.ends_with("Account") || p.ends_with("AccountSharedData"),
                ExprKind::Ident(p) => {
                    p.ends_with("Account::new") || p.ends_with("AccountSharedData::new")
                }
                _ => false,
            };
            if is_account {
                return Some(e);
            }
        }
        e.children().into_iter().find_map(find)
    }
    parts.iter().find_map(|p| find(p))
}

fn field<'e>(lit: &'e Expr, name: &str) -> Option<&'e Expr> {
    match &lit.kind {
        ExprKind::Call { options, .. } => options.iter().find(|(n, _)| n == name).map(|(_, v)| v),
        _ => None,
    }
}

fn is_empty_data(e: &Expr) -> bool {
    let t = compact(&e.text);
    t == "vec![]" || t == "Vec::new()" || t == "vec![0;0]" || t == "Default::default()"
}

fn shape(env: &Env<'_>, parts: &[&Expr]) -> Shape {
    let texts: Vec<String> = parts.iter().map(|p| compact(&p.text)).collect();
    let any = |needle: &str| texts.iter().any(|t| t.contains(needle));
    let rpc_fetch = parts.iter().any(|p| {
        let mut calls = Vec::new();
        receivers_of(p, "get_account", &mut calls);
        receivers_of(p, "get_account_info", &mut calls);
        calls.iter().any(|r| {
            let k = key(r);
            words(&k).iter().any(|w| w == "rpc")
                || env
                    .assigns
                    .get(&k)
                    .into_iter()
                    .flatten()
                    .any(|v| is_cluster_client(v))
        })
    });
    let copied = texts.iter().any(|t| {
        let t = t.to_ascii_lowercase();
        COPY_HINTS.iter().any(|h| t.contains(h))
    });
    if rpc_fetch || copied {
        return Shape::Copied;
    }
    let lit = account_literal(parts);
    if let Some(lit) = lit
        && field(lit, "executable")
            .is_some_and(|v| matches!(v.kind, ExprKind::Lit(Lit::Bool(true))))
    {
        return Shape::Program;
    }
    // What the value is built with: `token_account(..)`, `Mint { .. }`,
    // `spl_token::state::Account { .. }`.
    let mut builders = BTreeSet::new();
    for p in parts {
        callee_words(p, &mut builders);
    }
    let is_mint = builders.contains("mint") && !builders.contains("account")
        || ["MintLayout", "MINT_SIZE", "token::state::Mint"]
            .iter()
            .any(|h| any(h));
    // An account the token program owns is a token account or a mint.
    let token_owned = lit.and_then(|lit| field(lit, "owner")).is_some_and(|o| {
        let t = compact(&o.text);
        t.contains("TOKEN_PROGRAM_ID")
            || t.contains("TOKEN_2022_PROGRAM_ID")
            || t.starts_with("spl_token::")
            || t.starts_with("spl_token_2022::")
    });
    let token = builders.contains("token")
        || token_owned
        || [
            "TokenAccount",
            "token::state::Account",
            "token_2022::state::Account",
            "AccountLayout",
        ]
        .iter()
        .any(|h| any(h));
    if token && !is_mint {
        return Shape::TokenBalance;
    }
    let lamports_only = lit.is_some_and(|lit| match &lit.kind {
        // Account::new(lamports, space, owner)
        ExprKind::Call { args, options, .. } if options.is_empty() => {
            args.get(1).is_some_and(|s| compact(&s.text) == "0")
        }
        // Account { lamports, data: vec![], .. }
        ExprKind::Call { .. } => {
            field(lit, "lamports").is_some() && field(lit, "data").is_none_or(is_empty_data)
        }
        _ => false,
    });
    if lamports_only {
        Shape::LamportsOnly
    } else {
        Shape::State
    }
}

/// Words of every function and type an expression calls or builds.
fn callee_words(e: &Expr, out: &mut BTreeSet<String>) {
    if let ExprKind::Call { callee, .. } = &e.kind {
        let name = match &callee.kind {
            ExprKind::Ident(p) | ExprKind::Type(p) => p.as_str(),
            ExprKind::Member(_, m) => m.as_str(),
            _ => "",
        };
        out.extend(name.split("::").flat_map(words));
    }
    for c in e.children() {
        callee_words(c, out);
    }
}

fn receivers_of<'e>(e: &'e Expr, method: &str, out: &mut Vec<&'e Expr>) {
    if let ExprKind::Call { callee, .. } = &e.kind
        && let ExprKind::Member(receiver, m) = &callee.kind
        && m == method
    {
        out.push(receiver);
    }
    for c in e.children() {
        receivers_of(c, method, out);
    }
}

/// RG04 and RG06: `set_account` or `add_account` writes an account of the
/// protocol. Writing its balance is RG06, writing anything else is RG04.
fn account_writes(ctx: &Ctx<'_, '_>, out: &mut Vec<Raw>) {
    let env = ctx.env;
    for (i, ev) in ctx.events.iter().enumerate() {
        let Some(Event::WriteAccount {
            target,
            account: Some(account),
        }) = ev
        else {
            continue;
        };
        let op = ctx.op(i);
        let k = op.key(target);
        let class = env
            .class_of_expr(op.resolved(target))
            .unwrap_or_else(|| env.class_of(&k));
        if matches!(
            class,
            Class::Attacker | Class::Payer | Class::User | Class::Mock | Class::Program
        ) {
            continue;
        }
        let parts = account_texts(env, op, account);
        let owner = account_literal(&parts).and_then(|lit| {
            field(lit, "owner").or_else(|| match &lit.kind {
                ExprKind::Call { args, options, .. } if options.is_empty() => args.get(2),
                _ => None,
            })
        });
        let why = match (&class, owner) {
            (Class::Protocol(why), _) => why.clone(),
            (_, Some(owner)) if env.is_program_under_test(owner) => {
                format!("owned by the program `{}`", key(owner))
            }
            _ => continue,
        };
        let evidence = op.call().map_or(String::new(), |c| c.expr.text.clone());
        let method = op.call().map_or("set_account", written_name);
        match shape(env, &parts) {
            Shape::Copied | Shape::Program => {}
            s @ (Shape::TokenBalance | Shape::LamportsOnly) => {
                let what = if s == Shape::TokenBalance {
                    "the token balance"
                } else {
                    "the lamports"
                };
                out.push(raw(
                    "RG06",
                    ctx.by_phase(i, Severity::Error, Severity::Warn),
                    op,
                    format!(
                        "{method} sets {what} of `{k}` ({why}) {}. It transfers nothing and overwrites what the account held, so the program's own accounting never sees it. Fund the attacker and transfer instead.",
                        ctx.phase_text(i)
                    ),
                    &evidence,
                ));
            }
            Shape::State => out.push(raw(
                "RG04",
                ctx.by_phase(i, Severity::Error, Severity::Warn),
                op,
                format!(
                    "{method} writes the account `{k}` ({why}) {}, bypassing every check the program would make. The PoC has written the state it claims to reach.",
                    ctx.phase_text(i)
                ),
                &evidence,
            )),
        }
    }
}

/// RG05: a test double's code loaded at a real program's address.
fn code_replacement(ctx: &Ctx<'_, '_>, out: &mut Vec<Raw>) {
    let env = ctx.env;
    for (i, ev) in ctx.events.iter().enumerate() {
        let op = ctx.op(i);
        let (id, code): (&Expr, Vec<&Expr>) = match ev {
            Some(Event::LoadProgram { id, code }) => (id, code.clone()),
            Some(Event::WriteAccount {
                target,
                account: Some(account),
            }) => {
                let parts = account_texts(env, op, account);
                if shape(env, &parts) != Shape::Program {
                    continue;
                }
                (target, parts)
            }
            _ => continue,
        };
        // What the code is called: the names and paths it is loaded from.
        let mut names = Vec::new();
        for c in &code {
            collect_idents(c, &mut names);
            string_literals(c, &mut names);
        }
        let Some(double) = names.iter().find(|n| {
            let stem = n.rsplit(['/', '\\']).next().unwrap_or(n);
            is_mock_name(stem.split('.').next().unwrap_or(stem))
        }) else {
            continue;
        };
        let k = op.key(id);
        let real_address = !env.is_fresh(&k)
            && (matches!(env.class_of(&k), Class::Protocol(_))
                || env.is_fixed(&k)
                || k.ends_with("::ID")
                || k.ends_with("::id")
                || compact(&id.text).ends_with("::id()"));
        if !real_address {
            continue;
        }
        out.push(raw(
            "RG05",
            Severity::Warn,
            op,
            format!(
                "`{k}` gets the code of `{double}` {}. From here on the PoC runs against a program the cluster does not have at that address.",
                ctx.phase_text(i)
            ),
            &op.call().map_or(String::new(), |c| c.expr.text.clone()),
        ));
    }
}

fn string_literals(e: &Expr, out: &mut Vec<String>) {
    if let ExprKind::Lit(Lit::Str(s)) = &e.kind {
        out.push(s.clone());
    }
    for c in e.children() {
        string_literals(c, out);
    }
}

/// RG08: no assertion in the exploit.
fn no_assertion(ctx: &Ctx<'_, '_>, out: &mut Vec<Raw>) {
    let test = ctx.trace.test;
    if test.should_panic {
        return;
    }
    let mut asserts = 0;
    let mut logs = 0;
    for (i, _, c) in ctx.trace.calls() {
        if !ctx.is_exploit(i) {
            continue;
        }
        match c.class {
            CallClass::Assert => asserts += 1,
            CallClass::Log => logs += 1,
            _ => {}
        }
    }
    if asserts > 0 {
        return;
    }
    let succeeds = match ctx.trace.dialect {
        Dialect::Rust => "`.unwrap()` on a transaction",
        Dialect::TypeScript => "awaiting a transaction",
    };
    let (severity, message) = if logs > 0 {
        (
            Severity::Warn,
            format!(
                "`{}` prints {logs} value(s) but asserts none. The test passes whatever those values are; a reader has to trust the numbers; {succeeds} only checks that it did not fail.",
                test.name
            ),
        )
    } else {
        (
            Severity::Error,
            format!(
                "`{}` contains no assertion. It passes whatever the program does; {succeeds} only checks that it did not fail.",
                test.name
            ),
        )
    };
    out.push(Raw {
        rule: "RG08",
        severity,
        unit: ctx.trace.unit,
        pos: test.pos,
        message,
        evidence: test.header.clone(),
    });
}

const COMPARE_OPS: &[&str] = &["==", "!=", "<", "<=", ">", ">="];

fn tautology_of_pair(env: &Env<'_>, a: &Expr, b: &Expr, same_fails: bool) -> Option<String> {
    if !same_fails && compact(&a.text) == compact(&b.text) {
        return Some(format!("both sides are `{}`", compact(&a.text)));
    }
    if env.is_constant(a) && env.is_constant(b) {
        return Some(format!(
            "`{}` and `{}` are both values the test wrote itself",
            compact(&a.text),
            compact(&b.text)
        ));
    }
    None
}

fn tautology_of_condition(env: &Env<'_>, cond: &Expr) -> Option<String> {
    match &cond.kind {
        ExprKind::Lit(Lit::Bool(true)) => Some("the condition is the literal `true`".to_string()),
        // `assert.ok(false)` after a call that should have thrown always
        // fails: it is a deliberate failure, not a tautology.
        ExprKind::Lit(Lit::Bool(false)) => None,
        ExprKind::Binary(l, op, r) if COMPARE_OPS.contains(&op.as_str()) => {
            tautology_of_pair(env, l, r, op == "!=" || op == "<" || op == ">")
        }
        _ if env.is_constant(cond) => Some(format!("`{}` is a constant", compact(&cond.text))),
        _ => None,
    }
}

/// RG09: an assertion that cannot fail.
fn tautological_assertion(ctx: &Ctx<'_, '_>, out: &mut Vec<Raw>) {
    for (i, op, c) in ctx.trace.calls() {
        if !ctx.is_exploit(i) || c.class != CallClass::Assert {
            continue;
        }
        let base = c.name.strip_prefix("debug_").unwrap_or(&c.name);
        let reason = match base {
            "assert" => c
                .args
                .first()
                .and_then(|cond| tautology_of_condition(ctx.env, cond)),
            "assert_eq" if c.args.len() >= 2 => {
                tautology_of_pair(ctx.env, &c.args[0], &c.args[1], false)
            }
            "assert_ne" if c.args.len() >= 2 => {
                tautology_of_pair(ctx.env, &c.args[0], &c.args[1], true)
            }
            _ => None,
        };
        let Some(reason) = reason else { continue };
        out.push(raw(
            "RG09",
            Severity::Error,
            op,
            format!("This assertion cannot fail: {reason}."),
            &c.expr.text,
        ));
    }
}

/// Words in a function or field name that make its value a measurement.
const MEASURE_WORDS: &[&str] = &[
    "balance",
    "balances",
    "lamports",
    "amount",
    "supply",
    "reserve",
    "reserves",
    "shares",
    "debt",
    "collateral",
    "liquidity",
    "tvl",
];

fn is_measure_name(name: &str) -> bool {
    words(name)
        .iter()
        .any(|w| MEASURE_WORDS.contains(&w.as_str()))
}

/// Whose balance a measurement reads, and of what.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Subject {
    who: String,
    lamports: bool,
}

/// The account a measurement is about: the last identity-shaped argument of
/// the first call inside it that has one. The last, because a helper takes
/// the harness first: `token_balance(&svm, &attacker_tokens)`.
fn subject_of(op: &Op<'_>, env: &Env<'_>, e: &Expr, lamports: bool) -> Subject {
    fn first_account(op: &Op<'_>, env: &Env<'_>, e: &Expr, depth: usize) -> Option<String> {
        if let ExprKind::Call { args, .. } = &e.kind
            && let Some(a) = args
                .iter()
                .rev()
                .find(|a| super::trace::is_identity_shaped(a))
        {
            return Some(op.key(a));
        }
        if let ExprKind::Ident(n) = &e.kind
            && depth < 2
            && let Some(v) = env.assigns.get(n).and_then(|vs| vs.first())
        {
            return first_account(op, env, v, depth + 1);
        }
        e.children()
            .into_iter()
            .find_map(|c| first_account(op, env, c, depth))
    }
    let who = first_account(op, env, e, 0).unwrap_or_else(|| compact(&e.text));
    Subject { who, lamports }
}

/// How a measurement is told apart from others: its text, with string
/// arguments left out, so `balances(payer, "before")` and
/// `balances(payer, "after")` are the same read taken twice.
fn read_key(e: &Expr) -> String {
    let ExprKind::Call { callee, args, .. } = &e.kind else {
        return compact(&e.text);
    };
    let is_str = |a: &Expr| matches!(a.kind, ExprKind::Lit(Lit::Str(_)));
    if !args.iter().any(is_str) {
        return compact(&e.text);
    }
    let kept: Vec<String> = args
        .iter()
        .filter(|a| !is_str(a))
        .map(|a| compact(&a.text))
        .collect();
    format!("{}({})", compact(&callee.text), kept.join(","))
}

/// The read a call makes, when its name says it measures something:
/// `svm.get_balance(&pk)`, `token_balance(&svm, &ata)`.
fn measure_call(op: &Op<'_>, env: &Env<'_>, e: &Expr) -> Option<(String, Subject)> {
    let ExprKind::Call { callee, .. } = &e.kind else {
        return None;
    };
    let name = match &callee.kind {
        ExprKind::Member(_, m) => m.as_str(),
        ExprKind::Ident(p) => p.rsplit("::").next().unwrap_or(p),
        _ => return None,
    };
    if !is_measure_name(name) {
        return None;
    }
    let lamports = name == "get_balance" || words(name).iter().any(|w| w == "lamports");
    Some((read_key(e), subject_of(op, env, e, lamports)))
}

/// Measurements in an expression: calls whose name reads as one
/// (`get_balance`, `token_balance`) and `.lamports` / `.amount` fields.
/// Maps each read's text to its subject. With `deep` false, nested calls are
/// not entered: each is an op of its own and records its reads there.
fn reads_in(op: &Op<'_>, env: &Env<'_>, e: &Expr, deep: bool, out: &mut BTreeMap<String, Subject>) {
    match &e.kind {
        ExprKind::Call { .. } => {
            if !deep {
                return;
            }
            out.extend(measure_call(op, env, e));
        }
        ExprKind::Member(base, m) if m == "lamports" || m == "amount" => {
            out.insert(compact(&e.text), subject_of(op, env, base, m == "lamports"));
            if !deep {
                return;
            }
        }
        _ => {}
    }
    for c in e.children() {
        reads_in(op, env, c, deep, out);
    }
}

fn idents_in(e: &Expr, out: &mut BTreeSet<String>) {
    if let ExprKind::Ident(n) = &e.kind {
        out.insert(n.clone());
    }
    for c in e.children() {
        idents_in(c, out);
    }
}

fn is_zero(e: &Expr) -> bool {
    literal_value(e) == Some(0.0)
}

/// The value of a number literal such as `0`, `1_000_000`, `5u64`, `1e9` or
/// `10n`.
fn literal_value(e: &Expr) -> Option<f64> {
    if !matches!(e.kind, ExprKind::Lit(Lit::Number)) {
        return None;
    }
    let text: String = e.text.chars().filter(|c| *c != '_').collect();
    let mantissa_len = text
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(text.len());
    let (mantissa, rest) = text.split_at(mantissa_len);
    let mantissa: f64 = mantissa.parse().ok()?;
    let exponent = rest
        .strip_prefix(['e', 'E'])
        .map(|r| {
            let digits: String = r
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '-' || *c == '+')
                .collect();
            digits.parse::<i32>().unwrap_or(0)
        })
        .unwrap_or(0);
    Some(mantissa * 10f64.powi(exponent))
}

/// For a directional `assert!(a > b)`, the claimed lower bound when it is a
/// literal. `None` when the assertion is not directional.
fn directional(c: &Call<'_>) -> Option<Option<f64>> {
    let base = c.name.strip_prefix("debug_").unwrap_or(&c.name);
    if base != "assert" {
        return None;
    }
    let ExprKind::Binary(l, op, r) = &c.args.first()?.kind else {
        return None;
    };
    match op.as_str() {
        ">" | ">=" => Some(literal_value(r)),
        "<" | "<=" if !(is_zero(l) || is_zero(r)) => Some(literal_value(l)),
        _ => None,
    }
}

/// A balance the harness sets before the attack: an airdrop, or an account
/// written with lamports or a token balance.
struct Funding {
    op: usize,
    who: String,
    lamports: bool,
    /// The amount, when it is a literal.
    amount: Option<f64>,
}

impl Funding {
    /// Whether a claim with this lower bound measures a gain over the funded
    /// amount, as in Solidity: funding to zero always does, "more than 0"
    /// over a funded balance never does, and otherwise the bound has to reach
    /// the funded amount when both are literals.
    fn is_baseline_for(&self, bound: Option<f64>) -> bool {
        match (self.amount, bound) {
            (Some(0.0), _) => true,
            (_, Some(0.0)) => false,
            (Some(a), Some(b)) => b >= a,
            _ => true,
        }
    }
}

/// RG10: a directional assertion whose every measurement is read once, after
/// the attack.
fn no_baseline(ctx: &Ctx<'_, '_>, out: &mut Vec<Raw>) {
    let env = ctx.env;
    let ops = &ctx.trace.ops;
    // Every measurement read, in order: (op index, canonical text).
    let mut reads: Vec<(usize, String)> = Vec::new();
    // Which measurements each local was computed from.
    let mut provenance: BTreeMap<String, BTreeMap<String, Subject>> = BTreeMap::new();
    let mut funding: Vec<Funding> = Vec::new();
    struct Claim {
        op: usize,
        reads: BTreeMap<String, Subject>,
        bound: Option<f64>,
    }
    let mut claims: Vec<Claim> = Vec::new();

    for (i, op) in ops.iter().enumerate() {
        match &op.kind {
            OpKind::Call(c) => {
                let mut here: BTreeMap<String, Subject> =
                    measure_call(op, env, c.expr).into_iter().collect();
                for a in c.args {
                    reads_in(op, env, a, false, &mut here);
                }
                for r in here.into_keys() {
                    reads.push((i, r));
                }
                match &ctx.events[i] {
                    Some(Event::Airdrop { target, amount }) => funding.push(Funding {
                        op: i,
                        who: op.key(target),
                        lamports: true,
                        amount: amount.and_then(literal_value),
                    }),
                    Some(Event::WriteAccount {
                        target,
                        account: Some(account),
                    }) => {
                        let parts = account_texts(env, op, account);
                        let lamports = account_literal(&parts)
                            .and_then(|l| field(l, "lamports"))
                            .and_then(literal_value);
                        let token = shape(env, &parts) == Shape::TokenBalance;
                        funding.push(Funding {
                            op: i,
                            who: op.key(target),
                            lamports: !token,
                            amount: if token { None } else { lamports },
                        });
                    }
                    _ => {}
                }
                if c.class == CallClass::Assert
                    && ctx.is_exploit(i)
                    && let Some(bound) = directional(c)
                {
                    let mut used = BTreeMap::new();
                    let mut names = BTreeSet::new();
                    for a in c.args {
                        reads_in(op, env, a, true, &mut used);
                        idents_in(a, &mut names);
                    }
                    for n in names {
                        if let Some(p) = provenance.get(&n) {
                            used.extend(p.clone());
                        }
                    }
                    if !used.is_empty() {
                        claims.push(Claim {
                            op: i,
                            reads: used,
                            bound,
                        });
                    }
                }
            }
            OpKind::Assign {
                target,
                value: Some(value),
                ..
            } => {
                let mut from = BTreeMap::new();
                reads_in(op, env, value, true, &mut from);
                let mut shallow = BTreeMap::new();
                reads_in(op, env, value, false, &mut shallow);
                for r in shallow.into_keys() {
                    reads.push((i, r));
                }
                let mut names = BTreeSet::new();
                idents_in(value, &mut names);
                for n in names {
                    if let Some(p) = provenance.get(&n) {
                        from.extend(p.clone());
                    }
                }
                provenance.insert(target.clone(), from);
            }
            OpKind::Assign { .. } => {}
        }
    }

    let attacks: Vec<usize> = ctx
        .txs
        .iter()
        .map(|t| t.op)
        .filter(|&i| ctx.is_exploit(i))
        .collect();
    let Some(&first_attack) = attacks.first() else {
        return;
    };
    let occurrences = |r: &str, until: usize| -> Vec<usize> {
        reads
            .iter()
            .filter(|(i, t)| *i <= until && t == r)
            .map(|(i, _)| *i)
            .collect()
    };
    let aliases = |who: &str| -> BTreeSet<String> {
        let mut set = BTreeSet::from([who.to_string()]);
        let mut todo = vec![who.to_string()];
        while let Some(k) = todo.pop() {
            for v in env.assigns.get(&k).into_iter().flatten() {
                if super::trace::is_identity_shaped(v) && set.insert(key(v)) {
                    todo.push(key(v));
                }
            }
            if set.len() > 16 {
                break;
            }
        }
        set
    };
    for claim in &claims {
        let after_attack = claim.reads.keys().any(|r| {
            let last = occurrences(r, claim.op).last().copied().unwrap_or(claim.op);
            last > first_attack
        });
        if !after_attack {
            continue;
        }
        let baselined = claim.reads.iter().any(|(r, subject)| {
            let at = occurrences(r, claim.op);
            if let (Some(&first), Some(&last)) = (at.first(), at.last())
                && first < last
                && attacks.iter().any(|&w| w > first && w < last)
            {
                return true;
            }
            let who = aliases(&subject.who);
            let funded: Vec<&Funding> = funding
                .iter()
                .filter(|f| {
                    f.op < claim.op && f.lamports == subject.lamports && who.contains(&f.who)
                })
                .collect();
            if funded.is_empty() {
                // A keypair the test just made holds nothing until funded.
                return subject.lamports && who.iter().any(|k| env.is_fresh(k));
            }
            // The test fixed this balance, then attacked before the claim:
            // the claim measures from a known value.
            funded.iter().any(|f| {
                f.is_baseline_for(claim.bound) && attacks.iter().any(|&w| w > f.op && w < claim.op)
            })
        });
        // As in Solidity: one claim with a baseline means the test measures
        // a gain, and the others are not reported.
        if baselined {
            return;
        }
        let op = &ops[claim.op];
        let shown: Vec<String> = claim
            .reads
            .keys()
            .take(2)
            .map(|r| format!("`{r}`"))
            .collect();
        out.push(raw(
            "RG10",
            Severity::Warn,
            op,
            format!(
                "The asserted value {} is read only after the attack. Without a before-value the test cannot tell a gain from a balance that was already there.",
                shown.join(" and ")
            ),
            &op.call().map_or(String::new(), |c| c.expr.text.clone()),
        ));
        return;
    }
}

/// Text in a cluster URL or name that says it is the local validator.
const LOCAL_CLUSTERS: &[&str] = &[
    "localhost",
    "127.0.0.1",
    "localnet",
    "rpcendpoint",
    "rpc_endpoint",
];

/// RG11: the test reads the chain as it is today.
fn live_rpc(ctx: &Ctx<'_, '_>, out: &mut Vec<Raw>) {
    for (i, ev) in ctx.events.iter().enumerate() {
        if !matches!(ev, Some(Event::LiveRpc)) {
            continue;
        }
        let op = ctx.op(i);
        // A local test validator is not a live cluster, and neither is the
        // endpoint the test's own provider already uses. A URL held in a
        // local or a constant is read from its value.
        let local = op.call().and_then(|c| c.args.first()).is_some_and(|url| {
            let url = op.resolved(url);
            let k = key(url);
            std::iter::once(url)
                .chain(ctx.env.assigns.get(&k).into_iter().flatten().copied())
                .chain(ctx.env.consts.get(&k).copied())
                .any(|e| {
                    let text = e.text.to_ascii_lowercase();
                    LOCAL_CLUSTERS.iter().any(|l| text.contains(l))
                })
        });
        if local {
            continue;
        }
        out.push(raw(
            "RG11",
            Severity::Warn,
            op,
            "The test creates a client for a live cluster, so every account it reads through it is the chain as it is today. Balances, prices and the bug itself can change before a triager runs it.".to_string(),
            &op.call().map_or(String::new(), |c| c.expr.text.clone()),
        ));
    }
}

/// Whether a test touches a Solana harness at all. A Rust test that never
/// builds a harness or a transaction is not a PoC, and is not checked.
pub fn uses_harness(ctx: &Ctx<'_, '_>) -> bool {
    ctx.events.iter().any(Option::is_some)
}
