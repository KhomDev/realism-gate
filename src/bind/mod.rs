//! Binding claims to the PoC evidence that would show them.
//!
//! Each claim field has one predicate, built from the parsed PoC and from the
//! rules that already ran. The predicate table is docs/claims.md; this module
//! is its implementation, one function per field.
//!
//! Nothing here reads the network, reads the chain, or guesses. A field the
//! tool cannot bind from source is NOT CHECKABLE, and a field whose evidence
//! is absent is NOT SHOWN. Only evidence that says the opposite of the claim
//! makes it CONTRADICTED.

pub mod facts;

use crate::cite;
use crate::claims::{self, Attacker, Binding, Impact, Row, RowClaim, Status, Table};
use crate::declaration::Declaration;
use crate::engine::{Options, Report};
use crate::project::Project;
use facts::TestFacts;

/// The claim fields, in table order.
pub const FIELDS: &[&str] = &[
    "attacker",
    "impact",
    "asset",
    "amount",
    "preconditions",
    "cited",
    "fix",
];

/// Binds `claims` to the evidence in `project`.
pub fn bind(
    project: &Project,
    report: &Report,
    loaded: &claims::Loaded,
    citations: Option<&cite::Index>,
    declaration: Option<&Declaration>,
    opts: &Options,
) -> Table {
    let tests = facts::collect(project, report, declaration, opts);
    let sources: Vec<&str> = project.units.iter().map(|u| u.source.as_str()).collect();
    let row_claims = row_claims(loaded);
    let mut rows = Vec::new();
    for rc in row_claims {
        let (status, evidence) = match rc.field {
            "attacker" => attacker(&loaded.claims, &tests),
            "impact" => impact(&loaded.claims, &tests),
            "asset" => asset(&loaded.claims, &tests, &sources),
            "amount" => amount(&loaded.claims, &tests),
            "preconditions" => preconditions(&loaded.claims, &tests),
            "cited" => cited(&loaded.claims, citations),
            "fix" => fix(&loaded.claims),
            other => (
                Status::NotCheckable,
                format!("no predicate is defined for the claim field `{other}`"),
            ),
        };
        rows.push(Row {
            field: rc.field,
            claim: rc.claim,
            evidence,
            status,
        });
    }
    Table::new(loaded.binding, loaded.source.clone(), rows)
}

/// One row per claim field the block states. An inferred binding gets a row
/// for every field, because the reader has to see which fields the prose did
/// not carry.
fn row_claims(loaded: &claims::Loaded) -> Vec<RowClaim> {
    let stated = loaded.claims.rows();
    if loaded.binding == Binding::Structured {
        return stated;
    }
    FIELDS
        .iter()
        .map(|field| {
            stated
                .iter()
                .find(|r| r.field == *field)
                .cloned()
                .unwrap_or(RowClaim {
                    field,
                    claim: "(not stated)".to_string(),
                })
        })
        .collect()
}

/// The worse of two statuses for the purpose of "does the PoC show this".
/// A contradiction beats a miss, which beats "cannot tell".
fn worst(a: Status, b: Status) -> Status {
    fn rank(s: Status) -> u8 {
        match s {
            Status::Shown => 0,
            Status::NotCheckable => 1,
            Status::NotShown => 2,
            Status::Contradicted => 3,
        }
    }
    if rank(a) >= rank(b) { a } else { b }
}

fn fired_any(tests: &[TestFacts], rules: &[&str]) -> Option<&'static str> {
    tests.iter().find_map(|t| t.fired_any(rules))
}

fn attacker(claims: &claims::Claims, tests: &[TestFacts]) -> (Status, String) {
    let Some(a) = &claims.attacker else {
        return (Status::NotCheckable, not_stated());
    };
    match a {
        Attacker::AnyUser => {
            if let Some(rule) = fired_any(tests, &["RG01", "RG02"]) {
                return (
                    Status::Contradicted,
                    format!(
                        "{rule} fired: a role was used to reach the exploit, so the PoC does not show an unprivileged user doing it"
                    ),
                );
            }
            if let Some((key, why)) = tests
                .iter()
                .flat_map(|t| t.privileged_senders.iter().map(move |p| (t, p)))
                .map(|(_, (k, r))| (k.clone(), r.clone()))
                .next()
            {
                return (
                    Status::Contradicted,
                    format!("the exploit runs as `{key}`: {why}"),
                );
            }
            if let Some(rule) = fired_any(tests, &["RG04", "RG05", "RG06", "RG19"]) {
                return (
                    Status::NotShown,
                    format!(
                        "{rule} fired: the state the exploit relies on was injected, so the test does not show a user reaching it"
                    ),
                );
            }
            match sender_named(tests) {
                Some((test, key, label)) => {
                    let who = match label {
                        Some(label) => format!("`{key}` (makeAddr(\"{label}\"))"),
                        None => format!("`{key}`"),
                    };
                    (
                        Status::Shown,
                        format!(
                            "{who} sends the exploit in {test}; no role, no prank as protocol (RG01-RG03 clean)"
                        ),
                    )
                }
                None => (
                    Status::NotShown,
                    "no call in the exploit is made by an address the test controls".to_string(),
                ),
            }
        }
        Attacker::Role(name) => (
            Status::NotCheckable,
            format!(
                "the tool cannot confirm from the test that an attacker holds the `{name}` role; state that in realism.yaml and argue it in the report"
            ),
        ),
        Attacker::Address(addr) => {
            let used = tests.iter().any(|t| t.senders.iter().any(|s| s == addr));
            if used {
                (
                    Status::Shown,
                    format!("`{addr}` sends a call in the exploit"),
                )
            } else {
                (
                    Status::NotShown,
                    format!("`{addr}` appears as a claim but sends no call in the PoC"),
                )
            }
        }
    }
}

/// The test that shows most, and the first account other than the test
/// contract that sends one of its exploit steps. The test contract is the
/// deployer, not the attacker.
fn sender_named(tests: &[TestFacts]) -> Option<(&str, &str, Option<&str>)> {
    let best = tests
        .iter()
        .filter(|t| t.sender_order.iter().any(|s| s != "this"))
        .max_by_key(|t| t.asserts.len())?;
    let key = best.sender_order.iter().find(|s| s.as_str() != "this")?;
    Some((
        best.test.as_str(),
        key.as_str(),
        best.labels.get(key).map(String::as_str),
    ))
}

/// What kind of evidence an impact needs.
enum ImpactKind {
    /// Value has to move, and an assertion has to read it.
    Value,
    /// A call has to revert, and a sibling test has to show it not reverting.
    Revert,
    /// The impact text names neither.
    Unknown,
}

fn impact_kind(i: &Impact) -> ImpactKind {
    match i {
        Impact::Drain | Impact::MisAccounting => ImpactKind::Value,
        Impact::Freeze | Impact::Halt | Impact::Dos => ImpactKind::Revert,
        Impact::Other(text) => {
            let l = text.to_ascii_lowercase();
            let revert = [
                "revert",
                "block",
                "brick",
                "freeze",
                "frozen",
                "halt",
                "pause",
                "stop",
                "ceiling",
                "denial",
                "cannot",
                "unavailable",
            ];
            let value = [
                "drain", "steal", "loss", "lost", "transfer", "withdraw", "mint",
            ];
            if revert.iter().any(|w| l.contains(w)) {
                ImpactKind::Revert
            } else if value.iter().any(|w| l.contains(w)) {
                ImpactKind::Value
            } else {
                ImpactKind::Unknown
            }
        }
    }
}

fn impact(claims: &claims::Claims, tests: &[TestFacts]) -> (Status, String) {
    let Some(i) = &claims.impact else {
        return (Status::NotCheckable, not_stated());
    };
    match impact_kind(i) {
        ImpactKind::Value => {
            let measuring = tests
                .iter()
                .find(|t| !t.measure_asserts().is_empty())
                .or_else(|| {
                    tests
                        .iter()
                        .find(|t| !t.equality_asserts().is_empty() && i == &Impact::MisAccounting)
                });
            if let Some(t) = measuring {
                let a = t.measure_asserts().first().copied();
                let text = match a {
                    Some(a) => a.text.clone(),
                    None => t.equality_asserts()[0].text.clone(),
                };
                return (
                    Status::Shown,
                    format!("`{text}` in {} reads the value the impact is about", t.test),
                );
            }
            if let Some(t) = tests.iter().find(|t| !t.revert_asserts().is_empty()) {
                return (
                    Status::Contradicted,
                    format!(
                        "the only evidence is `{}` in {}: a revert shows the call failed, not that value moved",
                        t.revert_asserts()[0].text,
                        t.test
                    ),
                );
            }
            (
                Status::NotShown,
                "no assertion in the exploit reads a value, so nothing shows the impact"
                    .to_string(),
            )
        }
        ImpactKind::Revert => {
            // Every distinct revert the exploit asserts, in test order, and
            // one negative control. A revert with no control shows only that
            // the call reverts.
            let mut labels: Vec<String> = Vec::new();
            let mut control = None;
            for t in tests {
                for assertion in t.revert_asserts() {
                    let selector = assertion
                        .revert_selector()
                        .unwrap_or_else(|| "revert".to_string());
                    let label = format!(
                        "{}.expectRevert({selector}) in {}",
                        receiver_of(&assertion.text),
                        t.test
                    );
                    if !labels.contains(&label) {
                        labels.push(label);
                    }
                    if control.is_none() {
                        control = negative_control(tests, t);
                    }
                }
            }
            let Some(first) = labels.first() else {
                return (
                    Status::NotShown,
                    "no assertion in the exploit expects a revert".to_string(),
                );
            };
            let named = labels
                .iter()
                .take(2)
                .cloned()
                .collect::<Vec<_>>()
                .join("; ");
            match control {
                Some(control) => (
                    Status::Shown,
                    format!(
                        "{named}; negative control {} acts without the setup and does not revert",
                        control.test
                    ),
                ),
                None => (
                    Status::NotShown,
                    format!(
                        "{first} asserts a revert, but no sibling test performs the same call without the exploit and succeeds, so nothing shows the revert is caused by the exploit"
                    ),
                ),
            }
        }
        ImpactKind::Unknown => (
            Status::NotCheckable,
            format!(
                "the impact text `{}` names neither a revert nor a value movement, so there is no predicate to run",
                i.statement()
            ),
        ),
    }
}

/// `vm.expectRevert(X)` -> `vm`, so the evidence reads like the source.
fn receiver_of(text: &str) -> String {
    text.split('.')
        .next()
        .filter(|s| !s.contains('('))
        .unwrap_or("")
        .to_string()
}

/// A sibling test in the same suite that performs one of the exploit's calls
/// and asserts success without expecting a revert. That is the negative
/// control: without it, a revert shows only that the call reverts.
fn negative_control<'a>(tests: &'a [TestFacts], exploit: &TestFacts) -> Option<&'a TestFacts> {
    tests.iter().find(|t| {
        t.suite == exploit.suite
            && t.test != exploit.test
            && t.revert_asserts().is_empty()
            && !t.non_revert_asserts().is_empty()
            && !t.external_methods.is_disjoint(&exploit.external_methods)
    })
}

fn asset(claims: &claims::Claims, tests: &[TestFacts], sources: &[&str]) -> (Status, String) {
    let Some(a) = &claims.asset else {
        return (Status::NotCheckable, not_stated());
    };
    let needle = a.to_ascii_lowercase();
    let mentioned = sources
        .iter()
        .any(|s| s.to_ascii_lowercase().contains(&needle))
        || claims
            .cited
            .iter()
            .any(|c| c.to_ascii_lowercase().contains(&needle));
    if !mentioned {
        return (
            Status::NotCheckable,
            format!("the PoC never names `{a}`, so the tool cannot tie the impact to that asset"),
        );
    }
    if let Some(t) = tests.iter().find(|t| !t.measure_asserts().is_empty()) {
        return (
            Status::Shown,
            format!(
                "`{}` in {} reads a value and the test names `{a}`",
                t.measure_asserts()[0].text,
                t.test
            ),
        );
    }
    if let Some((t, assertion)) = best_equality(tests, &claim_words(claims)) {
        return (
            Status::Shown,
            format!(
                "`{}` in {} compares two quantities of `{a}`",
                assertion.text, t.test
            ),
        );
    }
    (
        Status::NotShown,
        format!("the PoC names `{a}` but no assertion reads or compares a value of it"),
    )
}

/// The best equality assertion over every test, and the test it is in.
fn best_equality<'a>(
    tests: &'a [TestFacts],
    claim: &std::collections::BTreeSet<String>,
) -> Option<(&'a TestFacts, &'a facts::AssertFact)> {
    let mut best: Option<(&TestFacts, &facts::AssertFact, (usize, bool, bool))> = None;
    for t in tests {
        for a in t.equality_asserts() {
            let score = facts::equality_score(a, claim);
            if best.as_ref().is_none_or(|(_, _, s)| score > *s) {
                best = Some((t, a, score));
            }
        }
    }
    best.map(|(t, a, _)| (t, a))
}

/// The words the claim itself uses about the quantity: the amount, the asset
/// and the impact text. An assertion that names them is better evidence than
/// one that happens to compare two other values.
fn claim_words(claims: &claims::Claims) -> std::collections::BTreeSet<String> {
    let mut text = String::new();
    let impact = claims.impact.as_ref().map(|i| i.statement());
    for part in [
        claims.amount.as_deref(),
        claims.asset.as_deref(),
        impact.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        text.push(' ');
        text.push_str(part);
    }
    text.split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| w.len() > 2)
        .map(|w| w.to_ascii_lowercase())
        .collect()
}

fn amount(claims: &claims::Claims, tests: &[TestFacts]) -> (Status, String) {
    let Some(_amount) = &claims.amount else {
        return (Status::NotCheckable, not_stated());
    };
    if let Some((t, a)) = best_equality(tests, &claim_words(claims)) {
        return (
            Status::Shown,
            format!(
                "`{}` in {} compares the two quantities the claim is about",
                a.text, t.test
            ),
        );
    }
    if let Some(t) = tests
        .iter()
        .find(|t| t.baseline_read && !t.measure_asserts().is_empty())
    {
        return (
            Status::Shown,
            format!(
                "{} reads the value before and after the exploit, so the amount is a difference",
                t.test
            ),
        );
    }
    (
        Status::NotShown,
        "no assertion compares two quantities, and no value is read before and after the exploit"
            .to_string(),
    )
}

/// What a precondition is about, as far as the PoC can show it.
enum Precondition {
    /// No role is needed.
    Privilege,
    /// No oracle, price, fork or chain state is read.
    ChainState,
    /// Nobody else has to act, in another transaction or in a chosen order.
    Timing,
    /// The entry point is open to any account.
    PublicEntry,
    /// The tool has no predicate for it.
    Other,
}

fn classify_precondition(text: &str) -> Precondition {
    let l = text.to_ascii_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| l.contains(w));
    if has(&[
        "privileged",
        "privilege",
        "role",
        "admin",
        "owner",
        "governance",
        "minter",
        "pauser",
    ]) {
        return Precondition::Privilege;
    }
    if has(&[
        "keeper",
        "relayer",
        "timing",
        "order",
        "front-run",
        "frontrun",
        "same block",
        "off-chain",
        "external trigger",
        "victim",
        "another transaction",
    ]) {
        return Precondition::Timing;
    }
    if has(&[
        "oracle",
        "price",
        "fork",
        "chain",
        "block",
        "snapshot",
        "rpc",
        "state",
        "liquidity",
    ]) {
        return Precondition::ChainState;
    }
    if has(&[
        "any user",
        "anyone",
        "any account",
        "unprivileged",
        "public",
        "permissionless",
    ]) {
        return Precondition::PublicEntry;
    }
    Precondition::Other
}

fn preconditions(claims: &claims::Claims, tests: &[TestFacts]) -> (Status, String) {
    if claims.preconditions.is_empty() {
        return (Status::NotCheckable, not_stated());
    }
    let mut status = Status::Shown;
    let mut notes = Vec::new();
    for p in &claims.preconditions {
        let (s, note) = match classify_precondition(p) {
            Precondition::Privilege => match fired_any(tests, &["RG01", "RG02"]) {
                Some(rule) => (
                    Status::Contradicted,
                    format!("`{p}`: {rule} fired, a role was used"),
                ),
                None => (Status::Shown, format!("`{p}`: RG01-RG03 clean")),
            },
            Precondition::ChainState => {
                let live = tests.iter().any(|t| t.live_chain);
                match fired_any(tests, &["RG11"]) {
                    Some(rule) => (Status::NotShown, format!("`{p}`: {rule} fired")),
                    None if live => (
                        Status::NotShown,
                        format!("`{p}`: the test reads a fork or a live cluster"),
                    ),
                    None => (
                        Status::Shown,
                        format!("`{p}`: no fork, RPC or chain read in the PoC"),
                    ),
                }
            }
            Precondition::Timing => match fired_any(tests, &["RG02", "RG12", "RG13"]) {
                Some(rule) => (
                    Status::NotShown,
                    format!("`{p}`: {rule} fired, someone else has to act"),
                ),
                None => (
                    Status::Shown,
                    format!("`{p}`: every exploit step is the same sender's own call"),
                ),
            },
            Precondition::PublicEntry => match fired_any(tests, &["RG01", "RG02"]) {
                Some(rule) => (
                    Status::Contradicted,
                    format!("`{p}`: {rule} fired, the entry point was not open"),
                ),
                None => (Status::Shown, format!("`{p}`: RG01-RG03 clean")),
            },
            Precondition::Other => (
                Status::NotCheckable,
                format!("`{p}`: no predicate exists for this precondition"),
            ),
        };
        status = worst(status, s);
        notes.push(note);
    }
    (status, notes.join("; "))
}

fn cited(claims: &claims::Claims, index: Option<&cite::Index>) -> (Status, String) {
    if claims.cited.is_empty() {
        return (Status::NotCheckable, not_stated());
    }
    let Some(index) = index else {
        return (
            Status::NotCheckable,
            "no target tree: pass --target DIR --target-commit REV, or the citations cannot be checked"
                .to_string(),
        );
    };
    let missing: Vec<&str> = claims
        .cited
        .iter()
        .filter(|c| !in_target(index, c))
        .map(String::as_str)
        .collect();
    if missing.is_empty() {
        (
            Status::Shown,
            format!(
                "all {} cited names are in the target tree (RG20 clean)",
                claims.cited.len()
            ),
        )
    } else {
        (
            Status::NotShown,
            format!("not in the target tree: {}", missing.join(", ")),
        )
    }
}

fn in_target(index: &cite::Index, entry: &str) -> bool {
    let e = entry.trim().trim_matches(['\'', '"']);
    if let Some((path, function)) = e.rsplit_once(':')
        && (path.contains('/') || path.ends_with(".sol"))
        && !path.starts_with("0x")
    {
        return file_in(index, path) && index.functions.contains(function);
    }
    if e.contains('/') && e.contains('.') {
        return file_in(index, e);
    }
    if let Some((contract, function)) = e.split_once('.')
        && index
            .contracts
            .get(contract)
            .is_some_and(|fns| fns.contains(function))
    {
        return true;
    }
    index.functions.contains(e) || index.contracts.contains_key(e)
}

fn file_in(index: &cite::Index, path: &str) -> bool {
    let path = path.trim_start_matches("./").replace('\\', "/");
    index.files.contains(&path)
        || index
            .files
            .iter()
            .any(|f| f == &path || f.ends_with(&format!("/{path}")))
}

fn fix(claims: &claims::Claims) -> (Status, String) {
    if claims.fix.is_none() {
        return (Status::NotCheckable, not_stated());
    }
    (
        Status::NotShown,
        "not run: `check` never applies a patch. Run `realism-gate verify-fix` on the patch and quote its verdict"
            .to_string(),
    )
}

fn not_stated() -> String {
    "not stated in the report, so there is nothing to bind".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preconditions_are_classified() {
        assert!(matches!(
            classify_precondition("no privileged role"),
            Precondition::Privilege
        ));
        assert!(matches!(
            classify_precondition("no oracle or price movement"),
            Precondition::ChainState
        ));
        assert!(matches!(
            classify_precondition("no keeper and no off-chain trigger"),
            Precondition::Timing
        ));
        assert!(matches!(
            classify_precondition("the entry point is public"),
            Precondition::PublicEntry
        ));
        assert!(matches!(
            classify_precondition("the moon is full"),
            Precondition::Other
        ));
    }

    #[test]
    fn statuses_rank_a_contradiction_first() {
        assert_eq!(
            worst(Status::NotCheckable, Status::NotShown),
            Status::NotShown
        );
        assert_eq!(
            worst(Status::Contradicted, Status::Shown),
            Status::Contradicted
        );
        assert_eq!(worst(Status::Shown, Status::Shown), Status::Shown);
    }

    #[test]
    fn citations_resolve_against_the_index() {
        let index = cite::Index {
            commit: "abc".to_string(),
            files: ["src/Vault.sol".to_string()].into_iter().collect(),
            contracts: [(
                "Vault".to_string(),
                ["open".to_string()].into_iter().collect(),
            )]
            .into_iter()
            .collect(),
            functions: ["open".to_string(), "frob".to_string()]
                .into_iter()
                .collect(),
        };
        assert!(in_target(&index, "src/Vault.sol:open"));
        assert!(in_target(&index, "Vault.open"));
        assert!(in_target(&index, "frob"));
        assert!(!in_target(&index, "src/Vault.sol:close"));
        assert!(!in_target(&index, "src/Psm.sol:sellStable"));
    }
}
