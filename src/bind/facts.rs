//! What one PoC test does, in the terms the claim predicates ask about.
//!
//! The rules already rebuild each test's trace. Binding a claim needs the same
//! and a little more: who sends each exploit call, which assertions the
//! exploit makes, which values it reads, and which rules fired on it. This
//! module collects exactly that and nothing that needs the network.

use crate::declaration::Declaration;
use crate::engine::{Options, Report};
use crate::identity::{Env, key};
use crate::project::{Project, Suite};
use crate::rules::Sender;
use crate::rules::evidence::{Subject, reads_in};
use crate::trace::{self, CallClass, OpKind, Trace};
use std::collections::{BTreeMap, BTreeSet};

/// One assertion the exploit makes.
#[derive(Debug, Clone)]
pub struct AssertFact {
    /// `assertEq`, `expectRevert`, ...
    pub method: String,
    /// The call as written.
    pub text: String,
    /// Each argument as written.
    pub args: Vec<String>,
    /// Whether an argument reads a balance, a total supply or another
    /// measured value.
    pub reads_value: bool,
}

impl AssertFact {
    /// Whether this assertion says "this call reverts".
    pub fn is_revert(&self) -> bool {
        let m = self.method.as_str();
        m.starts_with("expectRevert") || m.starts_with("expectPartialRevert")
    }

    /// The revert selector, when the assertion names one:
    /// `expectRevert(ReserveBackingShortfall)` -> `ReserveBackingShortfall`.
    pub fn revert_selector(&self) -> Option<String> {
        if !self.is_revert() {
            return None;
        }
        let arg = self.args.first()?;
        let arg = arg.trim();
        if arg.is_empty() || arg == "bytes" {
            return None;
        }
        Some(arg.to_string())
    }

    /// An equality between two quantities the test wrote: `assertEq(drawn,
    /// locked)`. Constants on both sides are not a claim about impact.
    pub fn is_equality_pair(&self) -> bool {
        match self.method.as_str() {
            "assertEq" | "assertEqDecimal" => {
                self.args.len() >= 2 && self.args[0].trim() != self.args[1].trim()
            }
            "assertTrue" | "assert" => self.args.first().is_some_and(|a| {
                split_comparison(a).is_some_and(|(op, l, r)| op == "==" && l != r)
            }),
            _ => false,
        }
    }
}

/// Splits a comparison written in a condition: `a == b` -> `("==", "a", "b")`.
pub fn split_comparison(text: &str) -> Option<(&str, &str, &str)> {
    for op in ["==", ">=", "<=", "!=", ">", "<"] {
        if let Some((l, r)) = text.split_once(op) {
            let l = l.trim();
            let r = r.trim();
            if !l.is_empty() && !r.is_empty() {
                return Some((op, l, r));
            }
        }
    }
    None
}

/// Everything one test shows, as far as the claim predicates need it.
#[derive(Debug, Clone, Default)]
pub struct TestFacts {
    pub suite: String,
    pub file: String,
    pub test: String,
    /// Canonical keys of the accounts that send the exploit's calls.
    pub senders: BTreeSet<String>,
    /// The same senders, in the order the exploit first uses them.
    pub sender_order: Vec<String>,
    /// The `makeAddr`/`vm.label` label of a sender, when it has one.
    pub labels: BTreeMap<String, String>,
    /// Senders that hold a role, with the reason the analysis gives.
    pub privileged_senders: Vec<(String, String)>,
    /// Assertions the exploit makes, in order.
    pub asserts: Vec<AssertFact>,
    /// External methods the exploit calls.
    pub external_methods: BTreeSet<String>,
    /// Measurement reads anywhere in the exploit.
    pub value_reads: Vec<String>,
    /// A value was read before the exploit changed state.
    pub baseline_read: bool,
    /// Rule ids that fired on this test.
    pub findings: BTreeSet<&'static str>,
    /// The test creates a fork or reads a live cluster (RG11's shape).
    pub live_chain: bool,
}

impl TestFacts {
    /// Assertions that read a balance, a total supply or another measured
    /// value: the evidence a value impact needs.
    pub fn measure_asserts(&self) -> Vec<&AssertFact> {
        self.asserts.iter().filter(|a| a.reads_value).collect()
    }

    /// Assertions other than "it reverts".
    pub fn non_revert_asserts(&self) -> Vec<&AssertFact> {
        self.asserts.iter().filter(|a| !a.is_revert()).collect()
    }

    /// Revert assertions.
    pub fn revert_asserts(&self) -> Vec<&AssertFact> {
        self.asserts.iter().filter(|a| a.is_revert()).collect()
    }

    /// Assertions that compare two different quantities.
    pub fn equality_asserts(&self) -> Vec<&AssertFact> {
        self.asserts
            .iter()
            .filter(|a| a.is_equality_pair())
            .collect()
    }

    /// The assertion that best compares two named quantities. A side that
    /// repeats a word from the claim counts most: `assertEq(drawn, locked)`
    /// beats `assertEq(shipped, SURPLUS)` for a claim about what was drawn
    /// and locked. Then both sides an identifier, then no arithmetic.
    pub fn best_equality_assert(&self, claim: &BTreeSet<String>) -> Option<&AssertFact> {
        self.equality_asserts()
            .into_iter()
            .max_by_key(|a| equality_score(a, claim))
    }

    /// Whether a rule fired on this test.
    pub fn fired(&self, rule: &str) -> bool {
        self.findings.contains(rule)
    }

    /// Whether any of `rules` fired on this test.
    pub fn fired_any(&self, rules: &[&str]) -> Option<&'static str> {
        self.findings.iter().find(|r| rules.contains(*r)).copied()
    }
}

/// How good an equality assertion is as evidence for an amount claim: how
/// many of the claim's own words it names (0-2), then both sides an
/// identifier, then neither side arithmetic.
pub fn equality_score(a: &AssertFact, claim: &BTreeSet<String>) -> (usize, bool, bool) {
    let sides: Vec<String> = a
        .args
        .iter()
        .take(2)
        .map(|s| s.to_ascii_lowercase())
        .collect();
    let named = sides
        .iter()
        .filter(|side| claim.iter().any(|word| side.contains(word.as_str())))
        .count();
    let both_idents = sides.len() == 2 && a.args.iter().take(2).all(|s| is_plain_ident(s));
    let no_arithmetic = sides.iter().all(|s| !s.contains(['-', '+', '*', '/']));
    (named, both_idents, no_arithmetic)
}

fn is_plain_ident(text: &str) -> bool {
    let t = text.trim();
    !t.is_empty()
        && t.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
        && t.chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
}

fn reads_in_text(e: &crate::ir::Expr) -> BTreeMap<String, Subject> {
    let mut out = BTreeMap::new();
    reads_in(e, &mut out);
    out
}

/// Every identifier an expression mentions.
fn idents_in(e: &crate::ir::Expr, out: &mut BTreeSet<String>) {
    if let crate::ir::ExprKind::Ident(name) = &e.kind {
        out.insert(name.clone());
    }
    for child in e.children() {
        idents_in(child, out);
    }
}

/// Locals that hold a measured value, directly or through one other local:
/// `uint256 profit = after - before` is a measurement even though the
/// assertion only names `profit`.
fn measured_locals(trace: &Trace<'_>) -> BTreeMap<String, bool> {
    let mut measured: BTreeMap<String, bool> = BTreeMap::new();
    for op in &trace.ops {
        let OpKind::Assign {
            target,
            value: Some(value),
            ..
        } = &op.kind
        else {
            continue;
        };
        let mut reads = BTreeMap::new();
        reads_in(value, &mut reads);
        let mut ids = BTreeSet::new();
        idents_in(value, &mut ids);
        let derived = !reads.is_empty()
            || ids
                .iter()
                .any(|i| measured.get(i).copied().unwrap_or(false));
        measured.insert(target.clone(), derived);
    }
    measured
}

/// Collects one test's facts.
fn one(
    project: &Project,
    report: &Report,
    declaration: Option<&Declaration>,
    suite: &Suite<'_>,
    test: &crate::ir::Function,
) -> TestFacts {
    let tr: Trace<'_> = trace::build(project, suite, test);
    let mut env = Env::new(project, suite.unit, &tr);
    if let Some(d) = declaration {
        env.declared_attackers = d.attackers().collect();
        env.declared_victims = d.victims().collect();
        env.declared_privileged = d.privileged_identities().collect();
        env.deployment_evidence = d.deployment_names().collect();
    }
    let (senders, _) = crate::rules::replay_senders(&tr);
    let measured = measured_locals(&tr);
    let suite_name = suite.contract.name.as_str();
    let mut facts = TestFacts {
        suite: suite_name.to_string(),
        file: project.units[suite.unit].path.clone(),
        test: test.name.clone(),
        ..TestFacts::default()
    };
    let mut read_before_write = false;
    let mut first_write_seen = false;
    for (i, op) in tr.ops.iter().enumerate() {
        if !op.phase.is_exploit() {
            continue;
        }
        if let Some(sender) = senders.get(i).copied().flatten() {
            let k = match sender {
                Sender::This => "this".to_string(),
                Sender::Switched(e) => key(e),
            };
            if let Some(p) = env.privileged(&k) {
                facts.privileged_senders.push((k.clone(), p.reason));
            }
            if let Some(label) = env.label(&k) {
                facts.labels.insert(k.clone(), label);
            }
            if facts.senders.insert(k.clone()) {
                facts.sender_order.push(k.clone());
            }
        }
        match &op.kind {
            OpKind::Call(c) => {
                match &c.class {
                    CallClass::Assert => {
                        let mut reads_value = false;
                        for a in c.args {
                            for r in reads_in_text(a).into_keys() {
                                reads_value = true;
                                if !facts.value_reads.contains(&r) {
                                    facts.value_reads.push(r);
                                }
                            }
                            let mut ids = BTreeSet::new();
                            idents_in(a, &mut ids);
                            if ids
                                .iter()
                                .any(|i| measured.get(i).copied().unwrap_or(false))
                            {
                                reads_value = true;
                            }
                        }
                        facts.asserts.push(AssertFact {
                            method: c.method.clone(),
                            text: c.expr.text.clone(),
                            args: c.args.iter().map(|a| a.text.clone()).collect(),
                            reads_value,
                        });
                    }
                    CallClass::External => {
                        facts.external_methods.insert(c.method.clone());
                        if crate::rules::evidence::is_measure(&c.method) {
                            if !first_write_seen {
                                read_before_write = true;
                            }
                        } else {
                            first_write_seen = true;
                        }
                        if matches!(c.method.as_str(), "createSelectFork" | "createFork") {
                            facts.live_chain = true;
                        }
                    }
                    _ => {}
                }
                if matches!(
                    c.method.as_str(),
                    "exists" | "getAccountInfo" | "createSelectFork" | "createFork"
                ) {
                    facts.live_chain = true;
                }
            }
            OpKind::Assign { value, .. } => {
                if let Some(v) = value {
                    for r in reads_in_text(v).into_keys() {
                        if !facts.value_reads.contains(&r) {
                            facts.value_reads.push(r);
                        }
                    }
                }
            }
            OpKind::Emit { .. } => {}
        }
    }
    facts.baseline_read = read_before_write;
    for f in &report.findings {
        if f.suite == suite_name && f.tests.iter().any(|t| t == &test.name) {
            facts.findings.insert(f.rule);
        }
    }
    facts
}

/// Facts for every test the run checks, in source order.
pub fn collect(
    project: &Project,
    report: &Report,
    declaration: Option<&Declaration>,
    opts: &Options,
) -> Vec<TestFacts> {
    let mut out = Vec::new();
    for suite in project.suites() {
        let suite_file = project.units[suite.unit].path.as_str();
        if opts.exclude.iter().any(|re| re.is_match(suite_file)) {
            continue;
        }
        if let Some(re) = &opts.match_contract
            && !re.is_match(&suite.contract.name)
        {
            continue;
        }
        for test in &suite.tests {
            if let Some(re) = &opts.match_test
                && !re.is_match(&test.name)
            {
                continue;
            }
            out.push(one(project, report, declaration, &suite, test));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comparisons_split() {
        assert_eq!(
            split_comparison("a.balanceOf(x) == drawn"),
            Some(("==", "a.balanceOf(x)", "drawn"))
        );
        assert_eq!(split_comparison("nothing"), None);
        assert_eq!(split_comparison(">= 1"), None);
    }

    #[test]
    fn the_trace_supplies_the_facts() {
        let project = Project::from_sources(&[(
            "test/Poc.t.sol",
            r#"contract Poc {
                function setUp() public { alice = makeAddr("alice"); }
                function test_a() public {
                    vm.startPrank(alice);
                    uint256 before = token.balanceOf(address(vault));
                    vault.drain();
                    assertEq(token.balanceOf(address(vault)), before);
                    assertEq(drawn, locked);
                    vm.stopPrank();
                }
                function test_control() public { vault.distribute(); assertGt(token.balanceOf(address(this)), 0); }
            }"#,
        )]);
        let report = crate::engine::Report {
            summary: Default::default(),
            declaration: None,
            config: None,
            changed_since: None,
            findings: Vec::new(),
            parse_errors: Vec::new(),
            ignored_policy: Vec::new(),
            claims: None,
            claim_binding: None,
        };
        let all = collect(&project, &report, None, &Options::default());
        let a = all.iter().find(|f| f.test == "test_a").unwrap();
        assert!(a.senders.contains("alice"));
        assert!(a.measure_asserts().len() == 1, "{:?}", a.asserts);
        assert!(a.equality_asserts().len() == 2, "{:?}", a.asserts);
        assert!(a.baseline_read);
        assert!(a.external_methods.contains("drain"));
        let c = all.iter().find(|f| f.test == "test_control").unwrap();
        assert!(!c.baseline_read);
    }
}
