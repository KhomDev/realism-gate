# realism-gate claim-binding precision

Does the proof prove the claim? This corpus takes 120 public exploit
reproductions, gives realism-gate a report and a structured claim derived from
the public incident record, and asks the tool which claims the PoC shows. It
then compares the tool's verdict with a frozen mechanical rubric. Everything is
re-derivable from the committed pointers, pairs, labels and run.py.

Status: final for realism-gate **1.0.0**, measured 2026-11-13 against the green
tree (rg-core: fmt/clippy clean, cargo test --locked 236 passed), binary sha256
06eb73f29b9eddf267f024dd14bfb5c79e45c774bbdf178a13b354c3e2a600de (the value is
read from results.json.tool.sha256). The claims CLI used is the
EXECUTION-BRIEF 4.1 contract (--report/--claims/--format json, rows in
snake_case). The rerun on the previous 1.0.0 tree, sha ece30fd0..., reproduced
the same numbers; rg-core says the only change since then is a clippy-only
rewrite (same digests) plus tests and docs.

Earlier passes are recorded here for provenance only and are not quoted:
pre-1.0 tree sha 642b9e38... (version string 0.6.1) and 1.0.0 sha d994e0cb...
both measured 91/100 = 91.0% for impact; the nine misses were profit-like local
assertions (profit = after - before; assertGt(profit, ...)) that the binder did
not recognise. rg-core fixed one-hop locals derived from balance reads, and the
rerun improved to 90/91 = 98.9%, which the final build reproduces.

## Corpus

| | |
|---|---|
| PoC source | SunWeb3Sec/DeFiHackLabs @ 2c99b565ae24ea2006adf181da20c4419b3edc30, src/test |
| candidates | 603 PoC files whose header carries a loss figure and either an attacker address or an analysis link |
| sample | 120, drawn by sha256(seed + ':' + id) from the 603, seed rg-claims-2026-11-12 |
| claim source | the public incident record in the PoC header (@KeyInfo: Total Lost, Attacker, Attack Tx) and the linked @Analysis write-ups as pointers |
| licence | DeFiHackLabs is Apache-2.0. Only pointers and derived labels are committed; PoC text lives in eval/cache/, which is gitignored |

Each pair is one PoC plus the incident record that makes a claim about it. The
record is rendered into a small report file and a claims.yaml at run time; the
linked write-ups are stored as URLs, never vendored. So the claim source is a
public incident summary whose links point at the write-up, not the write-up
text. That is weaker than a full report and it is why the labels are field-level
and structural (see below), not a reading of the article.

## Labels: frozen rubric, not truth

eval/claims/label.py is the frozen rule (rubric version 1, written before the
first tool run):

- impact drain: shown if the PoC asserts a balance/amount comparison
  (assertGt/assertGe/assertEq with balance, profit or amount); not_shown if the
  file contains no assert at all; not_checkable otherwise.
- attacker: shown if the claimed attacker address literal occurs in the PoC
  source; not_shown if the record names an attacker and the PoC does not;
  not_checkable if the record names none.
- amount: not_checkable by design. The record states a fiat loss; a fork PoC
  asserts token balances. Conflating the two would be a fabricated label.
- pair label: shown if impact is shown and attacker is not not_shown;
  contradicted only if a test function has expectRevert and no balance
  assertion.

Distribution over the 120 pairs (labels.yaml): impact shown 24, not_shown 92,
not_checkable 4; attacker shown 118, not_checkable 2; amount not_checkable 120;
pair shown 24, not_shown 92, not_checkable 4.

What that means: 92 of 120 public exploit reproductions have no assertion at
all (they execute the attack and log, or assert nothing). That is a property of
this corpus and it is the reason the impact precision below has a large
denominator of tool 'not_shown' verdicts.

Known label defect, published rather than hidden: the attacker label checks only
that the address literal appears. realism-gate binds identity to how the PoC
drives the call. The raw measured disagreement is 0/120, which is the defect of
that label, not a tool error rate; results.json marks the field
'not_interpretable'. Re-label identity by caller binding before quoting it.

No independent second-reader agreement pass was run for this corpus. The rubric
is mechanical (python eval/claims/label.py --sample prints the evidence per
pair), and the one weak field is marked. Treat the labels as auditable, not as
ground truth.

## Measurement

Claim-binding precision = of the claim rows realism-gate marks CONTRADICTED or
NOT SHOWN, the share where the frozen rubric also says the evidence does not
show it. Wilson 95% intervals, z = 1.959964. The denominator is the rows the
tool flagged, as in docs/precision.md.

**impact (drain): 90/91 = 98.9% [94.0%, 99.8%]** on 91 flagged rows.
- 1 row the tool flagged as not shown where the rubric is not_checkable:
  AzukiDAO_exp.sol, whose only assertion is assertEq(signature.length, 65) - not
  a balance or profit claim. That is a label gap, not a demonstrated tool error.
- 2 rows the tool called shown while the rubric sees no evidence: PikeFinance_exp.sol
  and SWAPPStaking_exp.sol (both listed in results.json). These are the recall
  cost, and they are not in the precision ratio; re-read them before treating
  the 98.9% as an upper bound on correctness.
- 91 flagged of 120 pairs: the other 29 were left not_checkable by the tool, so
  they do not enter the precision ratio.

The previous 1.0.0 build (sha d994e0cb) measured 91/100 = 91.0% [83.8%, 95.2%];
the difference is entirely the nine profit-local misses that rg-core fixed.
The 91.0% is the out-of-sample number: the nine misses were found on this set
and then fixed, so the 98.9% is in-sample. Publish both, or the 91.0%; never the
98.9% alone.

**attacker: raw 0/120 disagreements, NOT a precision number.** See the label
defect above. The tool's own evidence is concrete ('... appears as a claim but
sends no call in the PoC'), and it may well be right; this corpus cannot say.

**amount: not scored.** All 120 amount rows are not_checkable by the rubric.

**Overall pair outcome is not scored.** Per-field precision is the measurement;
an aggregate 'outcome' would mix not_checkable fields into a denominator that
the labels cannot support.

## Regression fixtures (realism-gate-killed rejection rows)

fixtures/rejections.yaml names the three rows of the rejection log whose
earliest_gate is REALISM: R-034, R-035, R-036 (from
clean-rejection-taxonomy-0.6.0/data/rejections.json). CONSOLIDATION-PLAN 1.4
requires each to produce CONTRADICTED or NOT SHOWN. The finding texts are
private and are not in this repository, so the fixtures are pointers plus the
expected status and are **not run**: status is not_run, input is private. A
private input directory can be supplied by the owner; this report does not claim
the current tool reproduces them.

## Freeze and reproducibility

run.py --split dev writes freeze.json (pairs sha256, labels sha256, harness
sha256, seed). --split heldout and --split all refuse without it and refuse if
the labels changed. The published run used --split all, so results.json has
heldout_opened: true. The rubric was frozen before the first tool run; no label
was changed after seeing tool output. The only post-run change is the
attacker field's status, from 'measured' to 'not_interpretable', with the reason
recorded in results.json.

    cd realism-gate
    python eval/claims/fetch.py --pairs          # offline if the corpus is present; writes pairs.yaml
    python eval/claims/label.py --write          # writes labels.yaml from the frozen rubric
    python eval/claims/run.py --gate target/debug/realism-gate.exe --split dev   # writes freeze.json
    python eval/claims/run.py --gate target/debug/realism-gate.exe --split all   # writes results.json
    python eval/claims/run.py --self-test        # fixture fake gate; no binary needed
    python eval/claims/run.py --check            # corpus facts still reproduce

fetch.py uses the network once (git sparse checkout of the pinned commit) and
falls back to the existing frozen clone. Nothing else here touches the network.

## Terms check (2026-11-13)

DeFiHackLabs is Apache-2.0 (LICENSE in the repository); PoC text is fetched into
eval/cache/ only and never committed. Incident records are summarised from the
PoC headers, which the repository publishes under that licence; the linked
write-ups are stored as URLs only. The rejection-taxonomy row ids come from the
public v0.6.0 export (data CC BY 4.0). No third-party prose is committed.

## What this does not claim

- Not a measurement of realism-gate's full rule set; only claim binding is
  scored, and only on the two fields the corpus can label.
- Not truth about the incidents: the labels say whether the PoC source
  structurally evidences the claim, not whether the hack happened.
- Not a report-corpus measurement: the claim source is the incident record, not
  the full write-up.
- Not a final claim about every future 1.0.x build: it is one tree at one
  sha256, which results.json records.
- Not a claim that the three private rejection rows reproduce; they are
  pointers and expected statuses only.
