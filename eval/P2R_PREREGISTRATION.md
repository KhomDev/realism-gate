# Preregistration — P2 precision rebuild (realism-gate)

Committed 2026-11-25 on `p2/integration`, **before any `p2/*` source change and
before any unit of `eval/solana-poc-v3/` or `eval/solidity-poc-v1/` was run,
read or labelled.** The verdict criteria in §1 are copied from the session
prompt (`_orchestration/PROMPT_PRECISION_REBUILD_2026-11-25.md` §10) and are not
edited after this file is committed. A later change is a new file and a new
version.

Working title: *Does the semantic program model and exploit model make
realism-gate precise on fresh judged PoCs?*

## 1. Verdict criteria (copied verbatim; do not edit)

"Precise and valuable" means **all** of the following hold on the **FINAL**
holdout, measured by the final frozen binaries, with labels from two blinded AI
readers with kappa ≥ 0.70. The human packet is delivered; human numbers are not
required to call the verdict, but are required before any public claim of
"human-validated".

**realism-gate, Solana** (FINAL minimum: ≥ 30 J units from ≥ 3 sources, ≥ 60
labelled flags):

| Criterion | Requirement |
|---|---|
| Judged readability | ≥ 85% point; Wilson lower bound ≥ 70%; no single source > 40% of the J units |
| Pooled precision, all rules shipped above `note` | Wilson lower bound ≥ 60% |
| Every rule shipped at `warn` | its own FINAL lower bound ≥ 0.50 with n ≥ 10, else it ships as `note` |
| Every rule shipped at `error` | FINAL lower bound ≥ 0.80 with n ≥ 16 |
| RG02 and RG03 | each ≥ 10 FINAL flags, point precision ≥ 70%, or they stay `note` with the reason published |
| No regression | every P1/P1-C TP pattern still fires (synthetic regression suite) |

**realism-gate, Solidity:** same criteria, but FINAL needs ≥ 60 J units from
≥ 4 sources and ≥ 100 labelled flags.

**priorart** (FINAL: Dup-Bench contests after T2 plus the new sealed pairs;
≥ 60 paid-unique findings and ≥ 300 judged duplicates):

| Criterion | Requirement |
|---|---|
| Duplicate recall@5 | ≥ 60% point, Wilson lower bound ≥ 50% |
| FAR on paid-unique findings, at the shipped threshold | ≤ 15% point, Wilson upper bound ≤ 25% |
| Warning | ships ON only if FINAL precision at its threshold has a lower bound ≥ 0.80; otherwise OFF, and the verdict says so |
| Abstention rate | reported; ≤ 25% |

**Engineering** (must all hold): CI green as in §9, coverage ≥ 85% on new
crates, zero unexplained mutants, fuzz weekly job green, no schema-breaking
change, offline default.

The verdict is one of:
- **PRECISE AND VALUABLE:** every row holds;
- **PARTIAL:** list the rows that hold and those that don't;
- **NOT YET:** the core rows (readability, pooled precision, recall@5, FAR) miss.

Report the verdict with every number, its n, interval and bootstrap. Write the
dev-to-final gap for each tool: when in-sample precision rose but FINAL did
not, that is the headline.

## 2. Data tiers and split (both corpora)

- **J — judged:** a coded PoC submitted to a contest or bounty with a published
  platform or sponsor outcome (valid / invalid / duplicate / severity).
- **X — exploit-by-construction:** a reproduction of a real incident or a
  teaching exploit with no platform judgement. Never counted in the verdict.
- **N — negative controls:** secure-path tests, or tests a judge rejected for
  the PoC itself being wrong. RQ4 only.

Splits are assigned by `int(sha256(salt + "|" + unit_id)[:8], 16) % 100`:
0–39 DEV, 40–59 V1, 60–74 V2, 75–99 FINAL. Salts are committed in each
corpus README and inventory before collection ends:

- Solana v3: `rg-solana-v3-salt-2b03962612050c100daeb8128706254e`
- Solidity v1: `rg-solidity-v1-salt-e3c5336cedf1f185f432914f5f2ab1ec`

Enlargement rule (pre-registered): after collection closes, if FINAL does not
meet §10's minimum n, units move from DEV to FINAL in ascending
`sha256(salt|unit_id)` order before the first Round-1 run. V1 and V2 are never
raided. Once FINAL is opened, no enlargement.

Source caps are enforced at collection: no source may exceed 40% of Solana J
units or 30% of Solidity J units in any slice. A source at its cap contributes
no further units.

## 3. Label protocol

- **Two independent AI readers** label every flag. Reader A labels from the
  flag, the rule definition (`docs/rules/`) and the source file. Reader B
  labels from the same inputs, **blinded to reader A's label, to the rule's
  shipped severity and to every round result**. Labels are `true-positive`,
  `false-positive` or `unclear`.
- **Agreement** is reported as raw agreement and Cohen's kappa over the two
  labels before adjudication. The verdict requires kappa ≥ 0.70; below that the
  cell is inconclusive and cannot support a severity.
- **Adjudication** is a third read by the orchestrator that sees both labels
  and writes a reason naming the rubric decision applied. The adjudicated label
  is the one counted; the pre-adjudication pair is what kappa is measured on.
- **Blinding of the human packet:** every FINAL sample gets a human packet and
  a scorer, produced before the FINAL labels are revealed in the report. No
  human label exists unless a human returns one; no number in this study is
  called human-validated.
- **Severity blindness:** labels never cite or know the shipped severity. A
  label that depends on severity is a protocol defect.

## 4. Rubric v3 (base: P1-C rubric v2, with the changes below)

Base: the eight decisions in
`eval/solana-poc-v2/PREREGISTRATION.md` §"Rubric decisions", commit `006f5e4`,
which remain in force for Solana and apply to Solidity by analogy (a
Foundry test is the unit; `deal`/`prank`/`store` are the mutations). Changes:

1. **Unreadable units are counted, never dropped.** Readable = `summary.tests >
   0` on the frozen binary. Unreadable units get a reason and appear in RQ1's
   denominator. Their flags do not exist, so they cannot be labelled.
2. **Judged readability is a J-unit measure.** X units are reported separately
   and never pooled into the verdict's readability row.
3. **Source-capped collection.** The collection stops adding a source that
   would exceed its cap (40% Solana, 30% Solidity of J units).
4. **A unit's kind and tier are recorded at collection time**, before any
   reader of the new model exists, from the file alone (§3.3 of the prompt).
5. **`unclear` never counts as TP or FP.** A rule/language cell with more than
   20% `unclear` labels is reported inconclusive; it cannot ship above `note`.
6. **The judged outcome is recorded but is not a label.** A platform's
   `invalid` verdict never turns a flag into a false positive; the rubric
   decides the flag. RQ3 is reported only if it is informative.
7. **A flag on a control test is a false positive for the identity/state rules**
   (RG01/RG02/RG03/RG04/RG06), and is judged on its own defect for RG08/RG09,
   as in v2 decision 4.
8. **Program-model evidence outranks the reader.** When the program model and
   the test harness disagree about authority (for example, a payer marked as a
   role signer in the test but not required by the program), the label follows
   the program source at the pinned commit, and the label reason cites the
   program span.

A rubric change after labels exist is a new version, committed before the next
round's labels; it never re-labels an opened round.

## 5. Analysis plan

- Every number is k/n with a Wilson 95% interval (z = 1.959964).
- A cell with n < 5 is a count, never a percentage headline.
- Every pooled cell also gets a **source-clustered bootstrap**: 10,000 draws
  resampling sources, seed `p2r-bootstrap-2026-11-25`, committed with the
  script.
- Never pool Solana with Solidity, never pool two binaries, never pool
  in-sample with fresh numbers. In-sample (DEV and opened slices) and fresh
  (V1/V2/FINAL at their own freeze) sit in separate tables.
- Reported per §8.3: RQ1 readability by tier/source/kind; RQ2 precision by
  rule with bootstrap; RQ3 judged-outcome association only if informative;
  RQ4 secure vs exploit controls; RQ5 `--execute` agreement.
- Severity floor at build time: `error` only when the fresh Wilson lower bound
  is ≥ 0.80 with n ≥ 16; `warn` when ≥ 0.50 with n ≥ 10; else `note`. The
  floor reads `measured.json`, generated by the committed `measure.py` from
  the frozen slice's labels.

## 6. Per-round targets (written before the round)

| Round | Slice | Targets |
|---|---|---|
| 1 | V1 | RG02/RG03 each ≥ 70% point with n ≥ 10, else inconclusive; pooled precision ≥ 50% point (exploratory); J readability ≥ 85% point; every P1/P1-C TP pattern still fires |
| 2 | V2 | the same, after Round-1 error-class fixes; `--execute` agreement reported where a runner exists |
| Final | FINAL | the §1 verdict table, once, by the frozen binaries |

## 7. Stopping rule (copied from §8.4)

There is no further round after FINAL. If a validation round misses, you may
run at most **one** extra validation round (V3), carved from FINAL only if
FINAL still meets its minimum n afterwards; preregister this before carving.
The FINAL holdout is never used for anything but the final verdict.

## 8. Freeze record (filled at each round, never before)

| Round | Code commit | Binary sha256 | Rubric version | Corpus commit | Salt |
|---|---|---|---|---|---|
| 1 | — | — | v3 | — | see §2 |
| 2 | — | — | v3 | — | see §2 |
| Final | — | — | v3 | — | see §2 |
