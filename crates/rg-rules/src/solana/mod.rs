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
//!
//! The parsers live in `rg-frontend-rust` and `rg-frontend-ts`, and the
//! environment, harness and trace in `rg-model`; this module re-exports them
//! under the paths the pipeline used before the workspace split.

pub mod rules;

pub use rg_frontend_rust::parse_rust;
pub use rg_frontend_ts::parse_ts;
pub use rg_ir::solana::ir;
pub use rg_model::solana::{env, harness, trace};

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
    program: Option<&rg_program::ProgramModel>,
) -> Option<Vec<Raw>> {
    if is_setup_for_later_tests(project, suite, test) {
        return None;
    }
    let tr = trace::build(scopes, suite.unit, test);
    let mut env = env::Env::new(project, &tr);
    env.declared_attackers = declared.attackers.clone();
    env.declared_privileged = declared.privileged.clone();
    env.declared_victims = declared.victims.clone();
    let mut ctx = rules::Ctx::new(project, &tr, &env, &suite.name, program, declared);
    ctx.control = rules::is_control(&tr);
    if !rules::uses_harness(&ctx) {
        return None;
    }
    Some(rules::check_test(&ctx))
}

/// Builds the exploit model of every checked test, for `--dump-model`.
///
/// The same tests `check` would model are included: a test that drives no
/// harness and calls no program function is not a PoC and has no model. The
/// order is the suite order, which is source order; the JSON is deterministic.
/// `base` is the invocation directory: every path the model carries is
/// rewritten relative to it, so an absolute input path never reaches the dump.
pub fn dump_model(
    project: &Project,
    declared: &Declared,
    program: Option<&rg_program::ProgramModel>,
    base: &std::path::Path,
) -> rg_model::exploit::ExploitModel {
    let scopes = trace::Scopes::new(project);
    let mut tests = Vec::new();
    for suite in suites(project) {
        for test in &suite.tests {
            if is_setup_for_later_tests(project, &suite, test) {
                continue;
            }
            let tr = trace::build(&scopes, suite.unit, test);
            let mut env = env::Env::new(project, &tr);
            env.declared_attackers = declared.attackers.clone();
            env.declared_privileged = declared.privileged.clone();
            env.declared_victims = declared.victims.clone();
            let declared_model = rg_model::exploit::Declared {
                attackers: declared.attackers.clone(),
                privileged: declared.privileged.clone(),
            };
            let model = rg_model::exploit::build(&rg_model::exploit::Input {
                project,
                trace: &tr,
                env: &env,
                suite: &suite.name,
                program,
                declared: &declared_model,
            });
            let drives_harness = rules::drives_harness(&tr);
            if drives_harness {
                tests.push(model);
            }
        }
    }
    let mut model = rg_model::exploit::ExploitModel {
        schema: rg_model::exploit::SCHEMA.to_string(),
        target: program.map(|p| p.target.clone()),
        tests,
    };
    rg_model::exploit::sanitize_paths(&mut model, base);
    model
}
