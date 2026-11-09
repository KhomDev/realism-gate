//! Who is who in a test: attacker, privileged role, ordinary user, protocol.
//!
//! The analysis is name- and data-flow-based. It reads identifier names,
//! `makeAddr`/`vm.label` labels, what each variable is assigned from, which
//! addresses the harness grants roles to, and which contracts it deploys. It
//! cannot see a deployed contract's access control, so every classification is
//! a presumption the rule pages state, and a `realism.yaml` declaration
//! overrides it.

use crate::ir::{Expr, ExprKind, Lit};
use crate::project::Project;
use crate::trace::{CallClass, OpKind, Trace};
use std::collections::{BTreeMap, BTreeSet};

/// Words that mark an address as holding a privileged role.
pub const ROLE_WORDS: &[&str] = &[
    "admin",
    "administrator",
    "governance",
    "governor",
    "gov",
    "timelock",
    "multisig",
    "msig",
    "guardian",
    "pauser",
    "minter",
    "operator",
    "curator",
    "manager",
    "keeper",
    "upgrader",
    "deployer",
    "dao",
    "council",
    "committee",
    "authority",
    "root",
    "superuser",
    "emergency",
    "executor",
    "proposer",
    "canceller",
    "strategist",
    "allocator",
    "sequencer",
];

/// Words that turn "owner" into a holder of something rather than a role:
/// `tokenOwner`, `nftOwner`, `positionOwner`.
const HOLDER_WORDS: &[&str] = &[
    "token", "nft", "position", "account", "user", "lp", "share", "shares", "note", "ticket",
    "holder", "wallet", "asset", "deposit", "item", "card", "domain", "of", "is",
];

pub const ATTACKER_WORDS: &[&str] = &[
    "attacker",
    "attack",
    "exploiter",
    "exploit",
    "hacker",
    "hack",
    "malicious",
    "evil",
    "adversary",
    "mallory",
    "eve",
    "thief",
    "griefer",
];

/// Words that mark an ordinary externally owned account.
pub const USER_WORDS: &[&str] = &[
    "alice",
    "bob",
    "carol",
    "charlie",
    "dave",
    "david",
    "erin",
    "frank",
    "grace",
    "user",
    "users",
    "victim",
    "victims",
    "whale",
    "lp",
    "depositor",
    "borrower",
    "lender",
    "trader",
    "holder",
    "recipient",
    "receiver",
    "eoa",
    "account",
    "someone",
    "random",
    "actor",
    "sender",
    "staker",
    "buyer",
    "seller",
    "player",
    "bidder",
    "voter",
    "member",
    "investor",
    "beneficiary",
    "customer",
    "bystander",
    "innocent",
    "honest",
    "solver",
    "maker",
    "taker",
    "liquidator",
];

const MOCK_WORDS: &[&str] = &["mock", "fake", "dummy", "stub", "harness"];

/// Words that mark the PoC's own scaffolding contracts.
const HELPER_WORDS: &[&str] = &["helper", "shim", "template", "scaffold"];

const TOKEN_HINTS: &[&str] = &[
    "erc20", "erc721", "erc1155", "token", "weth", "usdc", "usdt", "dai", "wbtc", "steth",
];

/// Methods whose call grants the address passed to them a role.
const GRANT_EXACT: &[&str] = &[
    "grantRole",
    "_grantRole",
    "_setupRole",
    "grantRoles",
    "transferOwnership",
    "_transferOwnership",
    "acceptOwnership",
    "setOwner",
    "changeOwner",
    "changeAdmin",
    "transferAdmin",
    "rely",
    "addAuth",
    "setAuthorized",
    "setAuthority",
];

const GRANT_VERBS: &[&str] = &[
    "set", "add", "grant", "change", "update", "appoint", "nominate", "transfer",
];

/// Splits an identifier into lowercase words: `USDCWhale` -> `usdc whale`,
/// `getRoleMember` -> `get role member`, `ADMIN_ROLE` -> `admin role`.
/// Trailing digits are dropped from each word, so `user1` reads as `user`.
pub fn words(name: &str) -> Vec<String> {
    let mut out = Vec::new();
    for part in name.split(|c: char| !c.is_ascii_alphanumeric()) {
        let chars: Vec<char> = part.chars().collect();
        let mut start = 0;
        for i in 1..chars.len() {
            let (prev, cur) = (chars[i - 1], chars[i]);
            let next_lower = chars.get(i + 1).is_some_and(|c| c.is_ascii_lowercase());
            let boundary = (prev.is_ascii_lowercase() && cur.is_ascii_uppercase())
                || (prev.is_ascii_uppercase() && cur.is_ascii_uppercase() && next_lower)
                || (prev.is_ascii_digit() && cur.is_ascii_alphabetic());
            if boundary {
                out.push(chars[start..i].iter().collect::<String>());
                start = i;
            }
        }
        if start < chars.len() {
            out.push(chars[start..].iter().collect::<String>());
        }
    }
    out.into_iter()
        .map(|w| {
            w.to_ascii_lowercase()
                .trim_end_matches(|c: char| c.is_ascii_digit())
                .to_string()
        })
        .filter(|w| !w.is_empty())
        .collect()
}

fn has_any(ws: &[String], vocab: &[&str]) -> Option<String> {
    ws.iter().find(|w| vocab.contains(&w.as_str())).cloned()
}

/// The role word in `name`, if `name` reads as a privileged role.
pub fn role_word(name: &str) -> Option<String> {
    let ws = words(name);
    if let Some(w) = has_any(&ws, ROLE_WORDS) {
        return Some(w);
    }
    if ws.iter().any(|w| w == "owner") && has_any(&ws, HOLDER_WORDS).is_none() {
        return Some("owner".to_string());
    }
    None
}

pub fn is_attacker_name(name: &str) -> bool {
    has_any(&words(name), ATTACKER_WORDS).is_some()
}

pub fn is_user_name(name: &str) -> bool {
    has_any(&words(name), USER_WORDS).is_some()
}

pub fn is_mock_name(name: &str) -> bool {
    has_any(&words(name), MOCK_WORDS).is_some()
}

pub fn is_token_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    TOKEN_HINTS.iter().any(|h| lower.contains(h))
}

/// A method that returns a privileged address: `owner()`, `getAdmin()`,
/// `governance()`, `getRoleMember(role, i)`.
pub fn is_privileged_getter(method: &str) -> bool {
    if method == "getRoleMember" {
        return true;
    }
    if method == "ownerOf" {
        return false;
    }
    let ws = words(method);
    let first = ws.first().map(String::as_str).unwrap_or("");
    if matches!(
        first,
        "is" | "has" | "only" | "set" | "grant" | "revoke" | "renounce" | "transfer" | "accept"
    ) {
        return false;
    }
    role_word(method).is_some()
}

/// Whether calling `method` grants a role to an address argument.
pub fn is_grant_method(method: &str) -> bool {
    if GRANT_EXACT.contains(&method) {
        return true;
    }
    let ws = words(method);
    let Some(first) = ws.first() else {
        return false;
    };
    GRANT_VERBS.contains(&first.as_str()) && role_word(&ws[1..].join("_")).is_some()
}

/// The canonical identity of an address-valued expression. Casts are peeled:
/// `address(vault)`, `payable(vault)` and `IVault(vault)` are all `vault`.
pub fn key(e: &Expr) -> String {
    match &e.kind {
        ExprKind::Ident(name) => name.clone(),
        ExprKind::Call { callee, args, .. } if args.len() == 1 => {
            let is_cast = match &callee.kind {
                ExprKind::Type(t) => t == "address" || t == "payable" || t == "address payable",
                ExprKind::Ident(n) => n.starts_with(|c: char| c.is_ascii_uppercase()),
                _ => false,
            };
            if is_cast {
                key(&args[0])
            } else {
                compact(&e.text)
            }
        }
        _ => compact(&e.text),
    }
}

/// Source text with all whitespace removed, for comparing expressions.
pub fn compact(text: &str) -> String {
    text.split_whitespace().collect()
}

/// What an address is presumed to be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Class {
    /// The test contract itself.
    This,
    Attacker,
    /// An externally owned account: a user, a victim, a whale.
    Eoa,
    Mock,
    /// A contract written next to the test: the PoC's own code.
    Helper,
    Token,
    /// A contract under review (deployed by the harness or addressed on a fork).
    Protocol(String),
    Unknown,
}

/// Evidence that an address holds a privileged role.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Privilege {
    pub reason: String,
    pub line: Option<u32>,
}

/// A role grant made by the harness.
#[derive(Debug, Clone)]
pub struct Grant {
    pub grantee: String,
    pub call_text: String,
    pub op: usize,
}

/// Facts about the identities a test uses, gathered from its whole trace.
#[derive(Debug, Default)]
pub struct Env<'p> {
    pub assigns: BTreeMap<String, Vec<&'p Expr>>,
    pub types: BTreeMap<String, String>,
    pub labels: BTreeMap<String, String>,
    pub grants: Vec<Grant>,
    pub constants: BTreeSet<String>,
    pub params: BTreeSet<String>,
    /// Contract types defined in the suite's own file.
    pub helper_types: BTreeSet<String>,
    pub declared_attackers: BTreeSet<String>,
    pub declared_privileged: BTreeSet<String>,
    pub declared_victims: BTreeSet<String>,
    /// Contract and actor names covered by `deployment.evidence`.
    pub deployment_evidence: BTreeSet<String>,
}

impl<'p> Env<'p> {
    pub fn new(project: &'p Project, suite_unit: usize, trace: &Trace<'p>) -> Env<'p> {
        let mut env = Env {
            helper_types: project.units[suite_unit]
                .contracts
                .iter()
                .map(|c| c.name.clone())
                .collect(),
            ..Env::default()
        };
        for (name, _) in &trace.test.params {
            env.params.insert(name.clone());
        }
        for op in &trace.ops {
            match &op.kind {
                OpKind::Assign { target, ty, value } => {
                    if let Some(ty) = ty {
                        env.types.insert(target.clone(), ty.clone());
                    }
                    if let Some(value) = value {
                        env.assigns.entry(target.clone()).or_default().push(value);
                    }
                }
                OpKind::Call(c) => {
                    let label_call = (c.class == CallClass::Cheat && c.method == "label")
                        || (c.class == CallClass::StdCheat && c.method == "label");
                    if label_call
                        && c.args.len() >= 2
                        && let ExprKind::Lit(Lit::Str(s)) = &c.args[1].kind
                    {
                        env.labels.insert(key(&c.args[0]), s.clone());
                    }
                }
                OpKind::Emit { .. } => {}
            }
        }
        // A second pass, once every declared type is known: a role constant is
        // a `bytes32`, an address constant is not, and both are written in
        // capitals.
        for (i, op) in trace.ops.iter().enumerate() {
            let Some(c) = op.call() else { continue };
            if c.class != CallClass::External || !is_grant_method(&c.method) {
                continue;
            }
            for arg in c.args {
                if let Some(grantee) = identity_arg(arg, &env.types) {
                    env.grants.push(Grant {
                        grantee,
                        call_text: c.expr.text.clone(),
                        op: i,
                    });
                }
            }
        }
        for located in &trace_contracts(project, suite_unit, trace) {
            for var in &located.vars {
                if var.constant {
                    env.constants.insert(var.name.clone());
                }
            }
        }
        env
    }

    /// Every value assigned to `k`, following plain aliases (`a = b`).
    fn values(&self, k: &str) -> Vec<&'p Expr> {
        self.assigns.get(k).cloned().unwrap_or_default()
    }

    /// The label given to `k` by `makeAddr("...")`, `vm.addr` wrappers or
    /// `vm.label(k, "...")`.
    pub fn label(&self, k: &str) -> Option<String> {
        if let Some(l) = self.labels.get(k) {
            return Some(l.clone());
        }
        self.values(k).into_iter().find_map(make_addr_label)
    }

    pub fn is_attacker(&self, k: &str) -> bool {
        if self.declared_attackers.contains(k) {
            return true;
        }
        if !self.declared_attackers.is_empty() {
            return false;
        }
        is_attacker_name(k)
            || self.label(k).is_some_and(|l| is_attacker_name(&l))
            || self
                .types
                .get(k)
                .is_some_and(|t| is_attacker_name(t) && self.helper_types.contains(t))
    }

    /// Whether the expression evaluates to a privileged address.
    pub fn privileged_expr(&self, e: &Expr) -> Option<Privilege> {
        if let Some(p) = getter_privilege(e) {
            return Some(p);
        }
        self.privileged(&key(e))
    }

    pub fn privileged(&self, k: &str) -> Option<Privilege> {
        self.privileged_depth(k, 0)
    }

    fn privileged_depth(&self, k: &str, depth: usize) -> Option<Privilege> {
        if depth > 6 {
            return None;
        }
        if self.declared_privileged.contains(k) {
            return Some(Privilege {
                reason: format!("`{k}` is declared privileged in realism.yaml"),
                line: None,
            });
        }
        let base = k.rsplit('.').next().unwrap_or(k);
        if let Some(w) = role_word(base) {
            return Some(Privilege {
                reason: format!("the name `{k}` reads as the `{w}` role"),
                line: None,
            });
        }
        if let Some(label) = self.label(k)
            && let Some(w) = role_word(&label)
        {
            return Some(Privilege {
                reason: format!("`{k}` is labelled \"{label}\" (`{w}` role)"),
                line: None,
            });
        }
        if let Some(g) = self.grants.iter().find(|g| g.grantee == k) {
            return Some(Privilege {
                reason: format!("the harness grants `{k}` a role: `{}`", g.call_text),
                line: None,
            });
        }
        for value in self.values(k) {
            if getter_privilege(value).is_some() {
                return Some(Privilege {
                    reason: format!("`{k}` is assigned from `{}`", compact(&value.text)),
                    line: Some(value.pos.line),
                });
            }
            let alias = key(value);
            if alias != k
                && is_identity_shaped(value)
                && let Some(p) = self.privileged_depth(&alias, depth + 1)
            {
                return Some(p);
            }
        }
        None
    }

    /// The type a key is declared with, peeling `IERC20 constant X = ...`.
    pub fn type_of(&self, k: &str) -> Option<&str> {
        self.types.get(k).map(String::as_str)
    }

    pub fn deployed_type(&self, k: &str) -> Option<String> {
        self.values(k).into_iter().find_map(deployed_type)
    }

    pub fn class(&self, e: &Expr) -> Class {
        self.class_of(&key(e))
    }

    pub fn class_of(&self, k: &str) -> Class {
        if k == "this" {
            return Class::This;
        }
        if self.is_attacker(k) {
            return Class::Attacker;
        }
        let ty = self.type_of(k).map(str::to_string);
        let deployed = self.deployed_type(k);
        let type_name = deployed.clone().or(ty.clone()).unwrap_or_default();
        if is_mock_name(k) || is_mock_name(&type_name) {
            return Class::Mock;
        }
        if deployed
            .as_ref()
            .is_some_and(|t| self.helper_types.contains(t.trim()))
            || has_any(&words(k), HELPER_WORDS).is_some()
        {
            return Class::Helper;
        }
        let label_user = self.label(k).is_some_and(|l| is_user_name(&l));
        let made_eoa = self.values(k).into_iter().any(is_eoa_constructor);
        if is_user_name(k) || label_user || made_eoa || self.declared_victims.contains(k) {
            return Class::Eoa;
        }
        if is_token_name(k) || is_token_name(&type_name) {
            return Class::Token;
        }
        if let Some(t) = deployed {
            return Class::Protocol(t);
        }
        if let Some(t) = &ty {
            let contract_typed = t.starts_with(|c: char| c.is_ascii_uppercase());
            if contract_typed && !self.helper_types.contains(t.trim()) {
                return Class::Protocol(t.clone());
            }
            if t.starts_with("address")
                && self.constants.contains(k)
                && self.values(k).into_iter().any(is_address_literal)
            {
                return Class::Protocol("address".to_string());
            }
        }
        Class::Unknown
    }

    /// Whether `e` is a compile-time constant in this test: a literal, an
    /// arithmetic combination of constants, or a local whose every assignment
    /// is a constant.
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
            ExprKind::Member(base, _) => matches!(base.kind, ExprKind::Type(_)),
            ExprKind::Call { callee, args, .. } => {
                matches!(&callee.kind, ExprKind::Type(t) if t != "address" && t != "payable")
                    && args.len() == 1
                    && self.is_constant_depth(&args[0], depth + 1)
            }
            ExprKind::Ident(name) => {
                if self.params.contains(name) {
                    return false;
                }
                let values = self.values(name);
                !values.is_empty() && values.iter().all(|v| self.is_constant_depth(v, depth + 1))
            }
            _ => false,
        }
    }
}

fn trace_contracts<'p>(
    project: &'p Project,
    unit: usize,
    trace: &Trace<'p>,
) -> Vec<&'p crate::ir::Contract> {
    let mut out = Vec::new();
    let mut units: BTreeSet<usize> = trace.ops.iter().map(|o| o.unit).collect();
    units.insert(unit);
    for u in units {
        out.extend(project.units[u].contracts.iter());
    }
    out
}

/// An argument that names an address: an identifier, `address(x)`, or a
/// member access. Role identifiers (`ADMIN_ROLE`, `vault.KEEPER_ROLE()`, any
/// `bytes32`), literals and booleans are not identities.
fn identity_arg(arg: &Expr, types: &BTreeMap<String, String>) -> Option<String> {
    if !is_identity_shaped(arg) {
        return None;
    }
    let k = key(arg);
    let base = k.rsplit('.').next().unwrap_or(&k);
    let role_constant = base.ends_with("Role")
        || base.ends_with("ROLE")
        || types.get(&k).is_some_and(|t| t.starts_with("bytes"))
        || (base.ends_with("()") && words(base).iter().any(|w| w == "role"));
    if role_constant {
        return None;
    }
    Some(k)
}

fn is_identity_shaped(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Ident(n) => n != "true" && n != "false",
        ExprKind::Member(..) => true,
        ExprKind::Call { callee, args, .. } => {
            args.len() == 1
                && matches!(&callee.kind, ExprKind::Type(t) if t == "address" || t == "payable")
        }
        _ => false,
    }
}

fn getter_privilege(e: &Expr) -> Option<Privilege> {
    if let ExprKind::Call { callee, .. } = &e.kind
        && let ExprKind::Member(_, method) = &callee.kind
        && is_privileged_getter(method)
    {
        return Some(Privilege {
            reason: format!("`{}` returns a privileged address", compact(&e.text)),
            line: Some(e.pos.line),
        });
    }
    None
}

fn make_addr_label(e: &Expr) -> Option<String> {
    if let ExprKind::Call { callee, args, .. } = &e.kind {
        let name = match &callee.kind {
            ExprKind::Ident(n) => n.as_str(),
            ExprKind::Member(_, m) => m.as_str(),
            _ => return None,
        };
        if matches!(name, "makeAddr" | "makeAccount" | "makeAddrAndKey")
            && let Some(ExprKind::Lit(Lit::Str(s))) = args.first().map(|a| &a.kind)
        {
            return Some(s.clone());
        }
    }
    None
}

fn is_eoa_constructor(e: &Expr) -> bool {
    if let ExprKind::Call { callee, args, .. } = &e.kind {
        let name = match &callee.kind {
            ExprKind::Ident(n) => n.as_str(),
            ExprKind::Member(_, m) => m.as_str(),
            ExprKind::Type(t) if t == "address" => {
                // address(uint160(n)) and address(0x1234): a synthetic EOA.
                return args.first().is_some_and(|a| {
                    matches!(&a.kind, ExprKind::Call { callee, .. }
                        if matches!(&callee.kind, ExprKind::Type(t) if t.starts_with("uint")))
                        || matches!(a.kind, ExprKind::Lit(Lit::Number))
                });
            }
            _ => return false,
        };
        return matches!(
            name,
            "makeAddr" | "makeAccount" | "makeAddrAndKey" | "addr" | "createWallet"
        );
    }
    false
}

fn is_address_literal(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Lit(Lit::Address) => true,
        ExprKind::Call { args, .. } if args.len() == 1 => is_address_literal(&args[0]),
        _ => false,
    }
}

fn deployed_type(e: &Expr) -> Option<String> {
    match &e.kind {
        ExprKind::Call { callee, args, .. } => match &callee.kind {
            ExprKind::New(t) => Some(t.clone()),
            _ if args.len() == 1 => deployed_type(&args[0]),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_split_into_words() {
        assert_eq!(words("USDCWhale"), vec!["usdc", "whale"]);
        assert_eq!(words("getRoleMember"), vec!["get", "role", "member"]);
        assert_eq!(words("ADMIN_ROLE"), vec!["admin", "role"]);
        assert_eq!(words("user1"), vec!["user"]);
        assert_eq!(words("_owner"), vec!["owner"]);
    }

    #[test]
    fn owner_is_a_role_unless_it_owns_a_thing() {
        assert_eq!(role_word("owner").as_deref(), Some("owner"));
        assert_eq!(role_word("protocolOwner").as_deref(), Some("owner"));
        assert_eq!(role_word("tokenOwner"), None);
        assert_eq!(role_word("nftOwner"), None);
        assert_eq!(
            role_word("governanceMultisig").as_deref(),
            Some("governance")
        );
        assert_eq!(role_word("alice"), None);
    }

    #[test]
    fn getters_and_grants() {
        assert!(is_privileged_getter("owner"));
        assert!(is_privileged_getter("getAdmin"));
        assert!(is_privileged_getter("getRoleMember"));
        assert!(!is_privileged_getter("ownerOf"));
        assert!(!is_privileged_getter("isOwner"));
        assert!(is_grant_method("grantRole"));
        assert!(is_grant_method("setKeeper"));
        assert!(is_grant_method("addOperator"));
        assert!(!is_grant_method("setFee"));
        assert!(!is_grant_method("transfer"));
    }

    #[test]
    fn casts_are_peeled_from_keys() {
        let p = crate::project::Project::from_sources(&[(
            "t.sol",
            "contract T { function f() public { g(address(payable(vault))); g(IVault(vault)); } }",
        )]);
        let body = p.units[0].contracts[0].functions[0].body.clone().unwrap();
        for stmt in &body {
            let crate::ir::StmtKind::Expr(e) = &stmt.kind else {
                panic!()
            };
            let ExprKind::Call { args, .. } = &e.kind else {
                panic!()
            };
            assert_eq!(key(&args[0]), "vault");
        }
    }
}
