//! Solana program tests, in Rust and in Anchor's TypeScript.
//!
//! A test is checked when it drives a Solana test harness: in Rust
//! `solana-program-test`, LiteSVM, Mollusk, anchor-client or anchor-litesvm;
//! in TypeScript an Anchor program client, web3.js, bankrun or LiteSVM. It
//! builds a harness, sends a transaction, or writes, loads or airdrops to an
//! account. Other tests in the same tree are not proofs of concept and are
//! skipped.
//!
//! The pipeline mirrors the Solidity one: [`parse_rust`] (with `syn`) and
//! [`parse_ts`] (with oxc) lower each file into one model, [`trace`] rebuilds
//! the order a test runs in with its helpers inlined, [`env`] decides who
//! each keypair and account is, and [`rules`] reports what makes the PoC
//! prove less than it claims. Nothing after the parsers depends on the
//! language.

pub mod env;
pub mod harness;
pub mod ir;
pub mod parse_rust;
pub mod parse_ts;
pub mod rules;
pub mod trace;

use crate::project::Project;
use crate::rules::Raw;
use std::collections::BTreeSet;

/// A group of tests: the tests of one Rust module or one `describe` block of
/// one file.
#[derive(Debug, Clone)]
pub struct Suite<'p> {
    pub unit: usize,
    pub dialect: ir::Dialect,
    /// For Rust, the file stem, then the module path: `exploit`,
    /// `lib::tests`. For TypeScript, the `describe` titles
    /// (`vault > withdraw`), or the file stem outside any.
    pub name: String,
    pub tests: Vec<&'p ir::Function>,
}

/// Identities a `realism.yaml` declaration names.
#[derive(Debug, Default, Clone)]
pub struct Declared {
    pub attackers: BTreeSet<String>,
    pub privileged: BTreeSet<String>,
    pub victims: BTreeSet<String>,
}

/// Every module or `describe` block of every Solana file that defines at
/// least one test.
pub fn suites(project: &Project) -> Vec<Suite<'_>> {
    let mut out: Vec<Suite<'_>> = Vec::new();
    for (unit, u) in project.units.iter().enumerate() {
        let Some(file) = &u.solana else { continue };
        let file_name = u.path.rsplit('/').next().unwrap_or(&u.path);
        let stem = file_name.split('.').next().unwrap_or(file_name);
        for f in file.functions.iter().filter(|f| f.test) {
            let name = match (file.dialect, f.module.is_empty()) {
                (_, true) => stem.to_string(),
                (ir::Dialect::Rust, false) => format!("{stem}::{}", f.module),
                (ir::Dialect::TypeScript, false) => f.module.clone(),
            };
            match out.iter_mut().find(|s| s.unit == unit && s.name == name) {
                Some(s) => s.tests.push(f),
                None => out.push(Suite {
                    unit,
                    dialect: file.dialect,
                    name,
                    tests: vec![f],
                }),
            }
        }
    }
    out
}

fn is_setup_for_later_tests(project: &Project, suite: &Suite<'_>, test: &ir::Function) -> bool {
    let Some(file) = &project.units[suite.unit].solana else {
        return false;
    };
    let Some(index) = file.functions.iter().position(|f| std::ptr::eq(f, test)) else {
        return false;
    };
    file.functions
        .iter()
        .any(|f| f.before.iter().any(|p| p.function == index && p.setup))
}

/// Runs the rules on one test. `None` when the test is not a PoC: it drives
/// no Solana harness, or it is a TypeScript test whose title says it sets up
/// (`it("is initialized")`) and that later tests build on. Such a test is
/// checked as part of the tests after it, as their setup.
pub fn check_test(
    project: &Project,
    scopes: &trace::Scopes<'_>,
    suite: &Suite<'_>,
    test: &ir::Function,
    declared: &Declared,
) -> Option<Vec<Raw>> {
    if is_setup_for_later_tests(project, suite, test) {
        return None;
    }
    let tr = trace::build(scopes, suite.unit, test);
    let mut env = env::Env::new(project, &tr);
    env.declared_attackers = declared.attackers.clone();
    env.declared_privileged = declared.privileged.clone();
    env.declared_victims = declared.victims.clone();
    let ctx = rules::Ctx::new(&tr, &env);
    if !rules::uses_harness(&ctx) {
        return None;
    }
    Some(rules::check_test(&ctx))
}
