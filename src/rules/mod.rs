//! The rules, and the machinery they share.

pub mod catalog;
mod evidence;
mod injection;
mod privilege;
mod realism;

use crate::identity::{Env, key};
use crate::ir::{Expr, Pos};
use crate::project::{Project, Suite};
use crate::trace::{CallClass, Phase, Trace};
pub use catalog::{Lang, MoveNotes, RULES, RuleInfo, SolanaNotes, rule};

use crate::finding::Severity;

/// A finding before it is placed in a file and attached to a test.
#[derive(Debug, Clone)]
pub struct Raw {
    pub rule: &'static str,
    pub severity: Severity,
    pub unit: usize,
    pub pos: Pos,
    pub message: String,
    pub evidence: String,
}

/// Who sends a call.
#[derive(Debug, Clone, Copy)]
pub enum Sender<'p> {
    /// The test contract itself.
    This,
    /// The address the most recent prank, hoax or broadcast switched to.
    Switched(&'p Expr),
}

/// Everything a rule can look at for one test.
pub struct Ctx<'p, 'a> {
    pub project: &'p Project,
    pub suite: &'a Suite<'p>,
    pub trace: &'a Trace<'p>,
    pub env: &'a Env<'p>,
    /// For each op, the sender of that op if it is an external call or a
    /// deployment.
    pub senders: Vec<Option<Sender<'p>>>,
    /// Ops that switch sender, with the address switched to.
    pub switches: Vec<(usize, &'p Expr)>,
    /// The file the test function is written in.
    pub test_unit: usize,
}

impl<'p> Ctx<'p, '_> {
    pub fn op_unit(&self, i: usize) -> usize {
        self.trace.ops[i].unit
    }

    pub fn sender_key(&self, i: usize) -> Option<String> {
        match self.senders.get(i).copied().flatten()? {
            Sender::This => Some("this".to_string()),
            Sender::Switched(e) => Some(key(e)),
        }
    }
}

/// Replays the trace's sender switches. A one-shot prank applies to the next
/// external call or deployment; a persistent one lasts until it is stopped or
/// replaced. Each phase starts as the test contract, which is how Foundry runs
/// `setUp()`, the test, and a callback the protocol triggers.
fn replay_senders<'p>(trace: &Trace<'p>) -> (Vec<Option<Sender<'p>>>, Vec<(usize, &'p Expr)>) {
    let mut senders = vec![None; trace.ops.len()];
    let mut switches = Vec::new();
    let mut once: Option<&'p Expr> = None;
    let mut persistent: Option<&'p Expr> = None;
    let mut phase = Phase::Setup;
    for (i, op) in trace.ops.iter().enumerate() {
        if op.phase != phase {
            phase = op.phase;
            once = None;
            persistent = None;
        }
        let Some(call) = op.call() else { continue };
        let first = call.args.first();
        match (&call.class, call.method.as_str()) {
            (CallClass::Cheat, "prank") | (CallClass::StdCheat, "hoax") => {
                if let Some(a) = first {
                    once = Some(a);
                    switches.push((i, a));
                }
            }
            (CallClass::Cheat, "startPrank")
            | (CallClass::Cheat, "changePrank")
            | (CallClass::StdCheat, "changePrank")
            | (CallClass::StdCheat, "startHoax") => {
                if let Some(a) = first {
                    persistent = Some(a);
                    switches.push((i, a));
                }
            }
            (CallClass::Cheat, "broadcast") => {
                once = first;
                if let Some(a) = first {
                    switches.push((i, a));
                }
            }
            (CallClass::Cheat, "startBroadcast") => {
                persistent = first;
                if let Some(a) = first {
                    switches.push((i, a));
                }
            }
            (CallClass::Cheat, "stopPrank") | (CallClass::Cheat, "stopBroadcast") => {
                persistent = None;
            }
            (CallClass::External, _) | (CallClass::Deploy(_), _) => {
                senders[i] = Some(match once.take().or(persistent) {
                    Some(e) => Sender::Switched(e),
                    None => Sender::This,
                });
            }
            _ => {}
        }
    }
    (senders, switches)
}

/// Runs every rule on one test. Rules that read only the declaration run
/// elsewhere, once per run.
pub fn check_test<'p>(
    project: &'p Project,
    suite: &Suite<'p>,
    trace: &Trace<'p>,
    env: &Env<'p>,
) -> Vec<Raw> {
    let (senders, switches) = replay_senders(trace);
    let test_unit = suite
        .function(&trace.test.name)
        .map(|(pos, _)| suite.linear[pos].unit)
        .unwrap_or(suite.unit);
    let ctx = Ctx {
        project,
        suite,
        trace,
        env,
        senders,
        switches,
        test_unit,
    };
    let mut out = Vec::new();
    privilege::attacker_holds_privilege(&ctx, &mut out);
    privilege::privileged_step_in_exploit(&ctx, &mut out);
    privilege::setup_hides_storage_write(&ctx, &mut out);
    privilege::deployer_is_attacker(&ctx, &mut out);
    injection::storage_write(&ctx, &mut out);
    injection::code_replacement(&ctx, &mut out);
    injection::balance_injection(&ctx, &mut out);
    injection::mocked_call(&ctx, &mut out);
    evidence::no_assertion(&ctx, &mut out);
    evidence::tautological_assertion(&ctx, &mut out);
    evidence::no_baseline(&ctx, &mut out);
    evidence::unpinned_fork(&ctx, &mut out);
    realism::profit_ignores_costs(&ctx, &mut out);
    realism::implausible_amount(&ctx, &mut out);
    realism::unevidenced_setup(&ctx, &mut out);
    realism::revert_only(&ctx, &mut out);
    out
}

/// Shortens source text for display: one line, at most `max` characters.
pub fn snippet(text: &str, max: usize) -> String {
    let one_line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.chars().count() <= max {
        one_line
    } else {
        let cut: String = one_line.chars().take(max.saturating_sub(1)).collect();
        format!("{cut}…")
    }
}

/// The first sentence of a rule's fix for findings in `lang`, used as the
/// one-line help.
pub fn help(r: &RuleInfo, lang: Lang) -> &'static str {
    let fix = r.fix_for(lang);
    match fix.find(". ") {
        Some(end) => &fix[..=end],
        None => fix,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_is_the_first_sentence() {
        let r = rule("RG06").unwrap();
        assert_eq!(
            help(r, Lang::Solidity),
            "Fund the attacker with `deal`, then `transfer` from the attacker to the contract."
        );
        assert_eq!(
            help(r, Lang::Rust),
            "Fund the attacker's own accounts, then transfer from the attacker to the program's account."
        );
        // A rule Rust does not check keeps its Solidity help.
        let rg07 = rule("RG07").unwrap();
        assert_eq!(help(rg07, Lang::Rust), help(rg07, Lang::Solidity));
    }

    #[test]
    fn snippets_are_single_line_and_bounded() {
        assert_eq!(snippet("a(\n   b,\n   c)", 80), "a( b, c)");
        assert_eq!(snippet("abcdef", 4), "abc…");
    }
}
