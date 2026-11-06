//! The Rust frontend: Solana program tests.
//!
//! A Rust test is checked when it drives a Solana test harness
//! (`solana-program-test`, LiteSVM or Mollusk): it builds one, sends a
//! transaction, or writes, loads or airdrops to an account. Other Rust tests
//! in the same tree are not proofs of concept and are skipped.
//!
//! The pipeline mirrors the Solidity one: [`parse`] lowers each file with
//! `syn`, [`trace`] rebuilds the order a test runs in with its helpers
//! inlined, [`env`] decides who each keypair and account is, and [`rules`]
//! reports what makes the PoC prove less than it claims.

pub mod env;
pub mod harness;
pub mod ir;
pub mod parse;
pub mod rules;
pub mod trace;

use crate::project::Project;
use crate::rules::Raw;
use std::collections::BTreeSet;

/// A group of Rust tests: the tests of one module of one file.
#[derive(Debug, Clone)]
pub struct Suite<'p> {
    pub unit: usize,
    /// The file stem, then the module path: `exploit`, `lib::tests`.
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

/// Every module of every Rust file that defines at least one test.
pub fn suites(project: &Project) -> Vec<Suite<'_>> {
    let mut out: Vec<Suite<'_>> = Vec::new();
    for (unit, u) in project.units.iter().enumerate() {
        let Some(file) = &u.rust else { continue };
        let stem = u
            .path
            .rsplit('/')
            .next()
            .unwrap_or(&u.path)
            .trim_end_matches(".rs");
        for f in file.functions.iter().filter(|f| f.test) {
            let name = if f.module.is_empty() {
                stem.to_string()
            } else {
                format!("{stem}::{}", f.module)
            };
            match out.iter_mut().find(|s| s.unit == unit && s.name == name) {
                Some(s) => s.tests.push(f),
                None => out.push(Suite {
                    unit,
                    name,
                    tests: vec![f],
                }),
            }
        }
    }
    out
}

/// Runs the rules on one test. `None` when the test does not drive a Solana
/// harness and so is not a PoC.
pub fn check_test(
    project: &Project,
    scopes: &trace::Scopes<'_>,
    suite: &Suite<'_>,
    test: &ir::Function,
    declared: &Declared,
) -> Option<Vec<Raw>> {
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
