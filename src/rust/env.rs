//! Who is who in a Solana test: the attacker's keypair, the harness's payer,
//! a role holder, an ordinary user, and the program's own accounts.
//!
//! As in Solidity, this is read from names and data flow, never from the
//! program: an account is the protocol's when the test derives it as a PDA,
//! addresses it by a fixed public key, or names it for protocol state
//! (`vault`, `pool`, `config`). A `realism.yaml` declaration overrides the
//! attacker heuristic.

use super::harness::{self, Event};
use super::trace::{OpKind, Trace, is_identity_shaped, key};
use crate::identity::{
    compact, is_attacker_name, is_grant_method, is_mock_name, is_user_name, role_word, words,
};
use crate::ir::{Expr, ExprKind};
use crate::project::Project;
use std::collections::{BTreeMap, BTreeSet};

/// What an account or keypair is presumed to be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Class {
    /// The harness's fee payer: `payer` from `ProgramTest::start()`,
    /// `context.payer`.
    Payer,
    Attacker,
    /// An ordinary user: a victim, a depositor, `alice`.
    User,
    /// A test double.
    Mock,
    /// An account of the protocol under test. The string says why.
    Protocol(String),
    /// The address of a program the test loads.
    Program,
    Unknown,
}

/// Words that name protocol state rather than a person.
const PROTOCOL_WORDS: &[&str] = &[
    "vault",
    "pool",
    "reserve",
    "reserves",
    "market",
    "state",
    "config",
    "global",
    "treasury",
    "obligation",
    "escrow",
    "oracle",
    "amm",
    "bank",
    "lending",
    "protocol",
    "custody",
    "pda",
];

/// Calls that derive a program-derived address.
const PDA_CALLS: &[&str] = &[
    "find_program_address",
    "create_program_address",
    "try_find_program_address",
    "get_program_address",
];

/// Calls that build a fixed public key from a literal.
const FIXED_KEY_CALLS: &[&str] = &[
    "pubkey!",
    "from_str",
    "from_str_const",
    "new_from_array",
    "new_const",
];

/// Calls that make a fresh, empty keypair or address.
const FRESH_CALLS: &[&str] = &["new_unique", "new", "generate", "default"];

/// Crates whose `ID` is a well-known program, not the program under test.
const LIBRARY_CRATES: &[&str] = &[
    "system_program",
    "spl_token",
    "spl_token_2022",
    "spl_associated_token_account",
    "token_program",
    "anchor_spl",
    "sysvar",
    "bpf_loader",
    "bpf_loader_upgradeable",
    "native_loader",
    "compute_budget",
    "stake",
    "vote",
    "address_lookup_table",
    "memo",
];

/// A role assignment the harness makes.
#[derive(Debug, Clone)]
pub struct Grant {
    pub grantee: String,
    pub text: String,
    pub op: usize,
}

/// The name of the called function, for `f(..)`, `a::f(..)` or `x.f(..)`.
/// A macro keeps its `!`: `pubkey!(..)` is not `kp.pubkey()`.
fn callee_name(e: &Expr) -> Option<&str> {
    let ExprKind::Call { callee, .. } = &e.kind else {
        return None;
    };
    match &callee.kind {
        ExprKind::Ident(p) | ExprKind::Type(p) => Some(p.rsplit("::").next().unwrap_or(p)),
        ExprKind::Member(_, m) => Some(m),
        _ => None,
    }
}

/// The call wrapped by `.unwrap()`, `.expect(..)` and `?`-free chains.
fn peel_unwrap(e: &Expr) -> &Expr {
    if let ExprKind::Call { callee, .. } = &e.kind
        && let ExprKind::Member(base, m) = &callee.kind
        && matches!(m.as_str(), "unwrap" | "expect" | "unwrap_or_default" | "0")
    {
        return peel_unwrap(base);
    }
    if let ExprKind::Member(base, m) = &e.kind
        && m == "0"
    {
        return peel_unwrap(base);
    }
    e
}

/// A word in `name` that marks a role, unless the name also says whose it
/// is: `user_authority` and `attacker_owner` are not roles.
pub fn role_of(name: &str) -> Option<String> {
    let base = name.rsplit("::").next().unwrap_or(name);
    let ws = words(base);
    if ws.iter().any(|w| w == "payer") || is_attacker_name(base) || is_user_name(base) {
        return None;
    }
    role_word(base)
}

/// Facts about the identities a test uses, gathered from its whole trace.
#[derive(Debug, Default)]
pub struct Env<'p> {
    pub assigns: BTreeMap<String, Vec<&'p Expr>>,
    /// `const` and `static` items of the files the test reads.
    pub consts: BTreeMap<String, &'p Expr>,
    /// Keys of the program ids the test loads.
    pub programs: BTreeSet<String>,
    pub grants: Vec<Grant>,
    pub declared_attackers: BTreeSet<String>,
    pub declared_privileged: BTreeSet<String>,
    pub declared_victims: BTreeSet<String>,
}

impl<'p> Env<'p> {
    pub fn new(project: &'p Project, trace: &Trace<'p>) -> Env<'p> {
        let mut env = Env::default();
        let mut units: BTreeSet<usize> = trace.ops.iter().map(|op| op.unit).collect();
        units.insert(trace.unit);
        for u in units {
            if let Some(file) = &project.units[u].rust {
                for (name, value) in &file.consts {
                    env.consts.entry(name.clone()).or_insert(value);
                }
            }
        }
        for op in &trace.ops {
            match &op.kind {
                OpKind::Assign { target, value } => {
                    if let Some(v) = value {
                        env.assigns.entry(target.clone()).or_default().push(v);
                    }
                }
                OpKind::Call(c) => {
                    if let Some(Event::LoadProgram { id, .. }) = harness::event(c) {
                        env.programs.insert(op.key(id));
                    }
                }
            }
        }
        // Grants need the classes above, so they come second.
        for (i, op, c) in trace.calls() {
            if c.fields.is_empty() && is_grant_method(&c.name) {
                // spl_token's set_authority(token_program, account,
                // new_authority, kind, owner, signers): only the new
                // authority is granted anything.
                let grantees: Vec<&Expr> = if c.name == "set_authority" && c.args.len() >= 5 {
                    vec![&c.args[2]]
                } else {
                    c.args.iter().collect()
                };
                for arg in grantees {
                    if !is_identity_shaped(arg) {
                        continue;
                    }
                    let k = op.key(arg);
                    if matches!(env.class_of(&k), Class::Protocol(_) | Class::Program) {
                        continue;
                    }
                    env.grants.push(Grant {
                        grantee: k,
                        text: c.expr.text.clone(),
                        op: i,
                    });
                }
            }
            // `Initialize { admin: attacker.pubkey(), .. }`: a role field set
            // in a struct that builds or reconfigures the protocol.
            if !c.fields.is_empty() && is_configuring(&c.name) {
                for (field, value) in c.fields {
                    // `owner` and `authority` also say whose account it is:
                    // `owner: alice.pubkey()` is alice's account, not a role.
                    let holder_field = matches!(field.as_str(), "owner" | "authority");
                    if role_of(field).is_some()
                        && is_identity_shaped(value)
                        && !(holder_field && env.class_of(&op.key(value)) == Class::User)
                    {
                        env.grants.push(Grant {
                            grantee: op.key(value),
                            text: format!("{field}: {}", compact(&value.text)),
                            op: i,
                        });
                    }
                }
            }
        }
        env
    }

    fn values(&self, k: &str) -> Vec<&'p Expr> {
        self.assigns.get(k).cloned().unwrap_or_default()
    }

    pub fn is_attacker(&self, k: &str) -> bool {
        self.class_of(k) == Class::Attacker
    }

    pub fn class_of(&self, k: &str) -> Class {
        self.class_depth(k, 0)
    }

    /// The class of an address written inline rather than through a local:
    /// `Pubkey::from_str(PRICE).unwrap()`, `Pubkey::find_program_address(..).0`.
    pub fn class_of_expr(&self, e: &Expr) -> Option<Class> {
        match callee_name(peel_unwrap(e)) {
            Some(n) if PDA_CALLS.contains(&n) => {
                Some(Class::Protocol("a program-derived address".to_string()))
            }
            Some(n) if FIXED_KEY_CALLS.contains(&n) => {
                Some(Class::Protocol("a fixed address".to_string()))
            }
            _ => None,
        }
    }

    fn class_depth(&self, k: &str, depth: usize) -> Class {
        if depth > 6 {
            return Class::Unknown;
        }
        if self.declared_attackers.contains(k) {
            return Class::Attacker;
        }
        let values = self.values(k);
        // Derivation comes before names: an account the program derives is
        // the program's state, whoever it describes (`attacker_position`).
        for v in &values {
            let v = peel_unwrap(v);
            match callee_name(v) {
                Some(n) if PDA_CALLS.contains(&n) => {
                    return Class::Protocol("a program-derived address".to_string());
                }
                Some(n) if n.starts_with("get_associated_token_address") => {
                    if let ExprKind::Call { args, .. } = &v.kind
                        && let Some(owner) = args.first()
                    {
                        let owner_class = self.class_depth(&key(owner), depth + 1);
                        return match owner_class {
                            Class::Unknown => Class::Unknown,
                            other => other,
                        };
                    }
                }
                _ => {}
            }
        }
        if self.declared_attackers.is_empty() && is_attacker_name(k) {
            return Class::Attacker;
        }
        let base = k.rsplit("::").next().unwrap_or(k);
        let ws = words(base);
        if ws.iter().any(|w| w == "payer") {
            return Class::Payer;
        }
        if is_mock_name(base) {
            return Class::Mock;
        }
        if is_user_name(base) || self.declared_victims.contains(k) {
            return Class::User;
        }
        if self.programs.contains(k) {
            return Class::Program;
        }
        let fixed = |e: &Expr| callee_name(e).is_some_and(|n| FIXED_KEY_CALLS.contains(&n));
        if values.iter().any(|v| fixed(v)) || self.consts.get(k).is_some_and(|v| fixed(v)) {
            return Class::Protocol("a fixed address".to_string());
        }
        if let Some(w) = ws.iter().find(|w| PROTOCOL_WORDS.contains(&w.as_str())) {
            return Class::Protocol(format!("the name reads as `{w}` state"));
        }
        for v in &values {
            if is_identity_shaped(v) {
                let alias = key(v);
                if alias != k {
                    let c = self.class_depth(&alias, depth + 1);
                    if c != Class::Unknown {
                        return c;
                    }
                }
            }
        }
        Class::Unknown
    }

    /// Whether `k` was made fresh by the test: `Keypair::new()`,
    /// `Pubkey::new_unique()`.
    pub fn is_fresh(&self, k: &str) -> bool {
        self.values(k).iter().any(|v| {
            let v = peel_unwrap(v);
            matches!(&v.kind, ExprKind::Call { callee, args, .. }
                if args.is_empty()
                    && matches!(&callee.kind, ExprKind::Ident(p) if p.split("::").count() >= 2
                        && FRESH_CALLS.contains(&p.rsplit("::").next().unwrap_or(""))))
        })
    }

    /// Why `k` is presumed to hold a role, if it is.
    pub fn privileged(&self, k: &str) -> Option<String> {
        self.privileged_depth(k, 0)
    }

    fn privileged_depth(&self, k: &str, depth: usize) -> Option<String> {
        if depth > 6 {
            return None;
        }
        if self.declared_privileged.contains(k) {
            return Some(format!("`{k}` is declared privileged in realism.yaml"));
        }
        if self.is_attacker(k) {
            return None;
        }
        if let Some(w) = role_of(k) {
            return Some(format!("the name `{k}` reads as the `{w}` role"));
        }
        if let Some(g) = self.grants.iter().find(|g| g.grantee == k) {
            return Some(format!(
                "the harness gives `{k}` a role: `{}`",
                crate::rules::snippet(&g.text, 80)
            ));
        }
        for v in self.values(k) {
            if is_identity_shaped(v) {
                let alias = key(v);
                if alias != k
                    && let Some(p) = self.privileged_depth(&alias, depth + 1)
                {
                    return Some(p);
                }
            }
        }
        None
    }

    /// Whether an expression names the program under test: a loaded
    /// program's id, `crate::ID`, `program_id`, or `my_program::ID`.
    pub fn is_program_under_test(&self, e: &Expr) -> bool {
        let k = key(e);
        if self.programs.contains(&k) {
            return true;
        }
        let path = match callee_name(e) {
            Some("id") => match &e.kind {
                ExprKind::Call { callee, .. } => key(callee),
                _ => k.clone(),
            },
            _ => k.clone(),
        };
        let segs: Vec<&str> = path.split("::").collect();
        match segs.as_slice() {
            ["program_id"] | ["ID"] | ["id"] | ["crate", "ID"] | ["crate", "id"] => true,
            [.., krate, "ID" | "id"] => !LIBRARY_CRATES.contains(krate),
            [one] => words(one).iter().any(|w| w == "program") && !LIBRARY_CRATES.contains(one),
            _ => false,
        }
    }

    /// Whether `e` is a compile-time constant in this test: a literal, a
    /// `const` item, an arithmetic combination of constants, or a local whose
    /// every assignment is a constant.
    pub fn is_constant(&self, e: &Expr) -> bool {
        self.is_constant_depth(e, 0)
    }

    fn is_constant_depth(&self, e: &Expr, depth: usize) -> bool {
        if depth > 6 {
            return false;
        }
        match &e.kind {
            ExprKind::Lit(_) => true,
            ExprKind::Unary(_, inner) => self.is_constant_depth(inner, depth + 1),
            ExprKind::Binary(l, _, r) => {
                self.is_constant_depth(l, depth + 1) && self.is_constant_depth(r, depth + 1)
            }
            ExprKind::Ident(path) => {
                let last = path.rsplit("::").next().unwrap_or(path);
                if self.consts.contains_key(path.as_str())
                    || (last.len() > 1
                        && last
                            .chars()
                            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_'))
                {
                    return true;
                }
                let values = self.values(path);
                !values.is_empty() && values.iter().all(|v| self.is_constant_depth(v, depth + 1))
            }
            _ => false,
        }
    }
}

/// Whether a struct or call name builds or reconfigures the protocol:
/// `Initialize`, `InitPool`, `UpdateConfig`, `SetAdmin`.
fn is_configuring(name: &str) -> bool {
    words(name).iter().any(|w| {
        matches!(
            w.as_str(),
            "init"
                | "initialize"
                | "initialise"
                | "config"
                | "configure"
                | "create"
                | "setup"
                | "set"
                | "update"
                | "change"
                | "transfer"
                | "new"
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rust::trace::{Scopes, build};

    fn with_env(src: &str, check: impl Fn(&Env<'_>)) {
        let p = Project::from_sources(&[("tests/t.rs", src)]);
        let scopes = Scopes::new(&p);
        let file = p.units[0].rust.as_ref().unwrap();
        let test = file.functions.iter().find(|f| f.test).unwrap();
        let trace = build(&scopes, 0, test);
        let env = Env::new(&p, &trace);
        check(&env);
    }

    #[test]
    fn classes_from_names_and_derivations() {
        with_env(
            r#"
            const POOL: Pubkey = pubkey!("11111111111111111111111111111111");
            #[test]
            fn test_a() {
                let attacker = Keypair::new();
                let (vault, _bump) = Pubkey::find_program_address(&[b"vault", attacker.pubkey().as_ref()], &program_id);
                let (position, _) = Pubkey::find_program_address(&[b"pos"], &program_id);
                let attacker_ata = get_associated_token_address(&attacker.pubkey(), &mint);
                let ata = get_associated_token_address(&alice.pubkey(), &mint);
                let thing = Pubkey::new_unique();
                let kp = attacker.insecure_clone();
            }
        "#,
            |env| {
                assert_eq!(env.class_of("attacker"), Class::Attacker);
                assert!(matches!(env.class_of("vault"), Class::Protocol(_)));
                assert!(matches!(env.class_of("position"), Class::Protocol(_)));
                assert_eq!(env.class_of("attacker_ata"), Class::Attacker);
                assert_eq!(env.class_of("ata"), Class::User);
                assert_eq!(env.class_of("thing"), Class::Unknown);
                assert!(env.is_fresh("thing"));
                assert_eq!(env.class_of("kp"), Class::Attacker);
                assert_eq!(env.class_of("payer"), Class::Payer);
                assert!(matches!(env.class_of("POOL"), Class::Protocol(_)));
            },
        );
    }

    #[test]
    fn roles_from_names_and_grants() {
        with_env(
            r#"
            #[test]
            fn test_a() {
                let ix = Initialize { admin: bob.pubkey(), payer: payer.pubkey() };
                set_authority(&spl_token::ID, &mint, Some(&carol.pubkey()), AuthorityType::MintTokens, &payer.pubkey(), &[]).unwrap();
                let signer = &admin;
            }
        "#,
            |env| {
                assert!(env.privileged("admin").is_some());
                assert!(env.privileged("upgrade_authority").is_some());
                assert!(env.privileged("user_authority").is_none());
                assert!(env.privileged("payer").is_none());
                assert!(
                    env.privileged("bob")
                        .unwrap()
                        .contains("admin: bob.pubkey()")
                );
                assert!(env.privileged("carol").is_some());
                assert!(env.privileged("signer").is_some());
            },
        );
    }

    #[test]
    fn the_program_under_test_is_recognised_by_its_id() {
        with_env(
            r#"
            #[test]
            fn test_a() { let mut pt = ProgramTest::new("vault", vault_program::ID, None); }
        "#,
            |env| {
                let e = |s: &str| {
                    crate::rust::parse::parse_source("x.rs", &format!("fn f() {{ {s}; }}"))
                };
                for (src, want) in [
                    ("vault_program::ID", true),
                    ("crate::ID", true),
                    ("program_id", true),
                    ("spl_token::ID", false),
                    ("system_program::id()", false),
                    ("my_program::id()", true),
                ] {
                    let unit = e(src);
                    let f = &unit.rust.as_ref().unwrap().functions[0];
                    let crate::ir::StmtKind::Expr(expr) = &f.body[0].kind else {
                        panic!()
                    };
                    assert_eq!(env.is_program_under_test(expr), want, "{src}");
                }
            },
        );
    }

    #[test]
    fn constants() {
        with_env(
            "const AMOUNT: u64 = 5; #[test] fn test_a() { let x = 100; let y = x * 2; let z = svm.get_balance(&a); }",
            |env| {
                let parsed = crate::rust::parse::parse_source(
                    "x.rs",
                    "fn f() { (AMOUNT, y, LAMPORTS_PER_SOL, z, 3 + AMOUNT); }",
                );
                let f = &parsed.rust.as_ref().unwrap().functions[0];
                let crate::ir::StmtKind::Expr(e) = &f.body[0].kind else {
                    panic!()
                };
                let ExprKind::Tuple(items) = &e.kind else {
                    panic!()
                };
                let got: Vec<bool> = items.iter().map(|i| env.is_constant(i)).collect();
                assert_eq!(got, vec![true, true, true, false, true]);
            },
        );
    }
}
