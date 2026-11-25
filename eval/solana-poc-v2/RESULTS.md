# P1-C: realism-gate 1.2.0 on the sealed fresh Solana PoC corpus (v2)

Run 2026-11-25. Corpus committed at `006f5e4`; re-frozen code at `f51ed17`;
binary `target/release/realism-gate.exe` sha256
`de19d1f5e6adc6cd723b2b51de3b105a30134a3a2c1f8ebf8d556286b40ccc70`.
Preregistration: `PREREGISTRATION.md` (rubric decisions, split, sampling,
bootstrap, targets). Every table below is **out-of-sample**: the corpus was
collected, pinned and hashed before the first `src/` change, and the author
did not open or label a v2 flag before the freeze.

## RQ1: coverage

A unit is readable when the tool checked at least one test in it.

| Tier | Readable | Wilson 95% |
|---|---|---|
| J, judged | 25/28 = 89.3% | [72.8-96.3] |
| X, exploit-by-construction | 48/53 = 90.6% | [79.8-95.9] |
| **J+X** | **73/81 = 90.1%** | **[81.7-94.9]** |

By kind (recorded at collection, before the 1.2.0 readers existed):

| Kind | Readable |
|---|---|
| test-file | 40/43 |
| test-fragment | 15/19 |
| rust-unit-test | 1/1 |
| binary-main | 17/18 |
| script | none in the counted set |
| ctf-solve-program | none in the counted set |

Unreadable units and the reason (the inventory's `kind` is the collection-time
classification; the run's parse error is reported when there is one):

| Unit | Kind | Reason |
|---|---|---|
| J-arena-167 | test-fragment | the extracted `it()` block is truncated: it does not parse as a test body |
| J-arena-200 | test-fragment | the extracted block does not parse |
| J-arena-255 | test-fragment | the extracted block does not parse |
| X-arena-40 | test-file | does not parse |
| X-arena-186 | test-file | parses, but drives no harness and calls no program function |
| X-arena-277 | test-fragment | does not parse (indented fragment) |
| X-loopscale | binary-main | `fn main` uses Mollusk directly, which the PoC-framework reader does not recognise |
| X-1inch-escrow | test-file | the test is generated inside a `run_for_tokens!` macro, so no top-level `#[test]` is seen |

These are coverage misses, not dropped units. The three unparseable fragments
are extraction artefacts (the issue's block boundary), not reader defects; the
two test-file misses are named limitations in `docs/limitations.md`.

## Flags

The run raised **77 flags on the 81 units** (all on J/X; none on a negative
control, of which there are none). They are sampled (77 of 77) but **not
labelled**: RQ2 precision, RQ3 outcome and the per-rule tables are not in this
file. The P1-C prompt's fresh headline (precision with n, Wilson and a
source-clustered bootstrap) is therefore **not yet measured**; only RQ1 is.

## In-sample re-measurement (the P1 PoC corpus)

Same frozen 1.2.0 binary, the P1 corpus (`eval/solana-poc/`), every table
**in-sample**: the fixes were designed from this corpus's error analysis.

| Measure | 1.0.4 (P1) | 1.2.0 (P1-C) |
|---|---|---|
| Judged PoCs readable | 1/12 | **6/12 = 50.0% [25.4-74.6]** |
| Exploit-by-construction readable | 72/82 | **82/82 = 100% [95.5-100]** |
| All units readable | 73/94 | 93/99 = 93.9% [87.4-97.2] |
| Active flags | 205 | 144 |
| Flags that disappeared | — | 86 (P1 labels: 77 false-positive, 9 true-positive) |
| New flags | — | 25 (RG08 19, RG02 2, RG03 2, RG11 2) |

The judged-readability target was **>= 8/12 on the 10 fixable judged PoCs**;
the measured value is **6/12**. The four Rust unit tests (066, 112, 115, H02)
and one fragment (078) are now read. Still unreadable: 080 (a bare fragment
that does not parse as a test body), WOOFi 070 (an `it()` block with no
imports; the file-level Solana gate rejects it), 085 and 140 (scripts whose
Solana import is one level away in `utils.ts`), Pump M-03 (a pure fee
simulation that drives no client), and OnRe (its harness is behind helpers the
portfolio does not include). **The target is missed.**

The 86 disappeared flags are the precision gain: 77 were P1-labelled false
positives (the attacker's own key, secure-path tests, fixture writes, a
difference read as an after-only value). Nine were P1-labelled true positives:

- six RG08 on setup/init steps, which rubric v2 scopes out of RG08 (the rule
  asserts the exploit's outcome, not setup's);
- three RG01 on the attacker draining its own vault, which the class-1
  identity fix reads as the attacker's own account.

The second group is a **conflict inside the P1-C instructions**: the class-1
fix says the attacker's own key is not privileged, while the TP list says "an
unsigned transfer proven on the attacker's own account (RG01, d45fa249)" must
stay flagged. The fix was implemented as written; those three flags no longer
fire. This is reported as a regression against the P1 TP set, not hidden.

## Reproduction

```bash
bash eval/solana-poc-v2/fetch.sh --check
python eval/solana-poc-v2/extract.py && python eval/solana-poc-v2/inventory.py --check
python eval/solana-poc-v2/run.py target/release/realism-gate.exe
python eval/solana-poc-v2/rq1.py --version 1.2.0
python eval/solana-poc/recompare.py --json      # in-sample, 1.0.4 vs 1.2.0
```

## Round 2 (1.3.0): fresh held-out precision after two dev-split fixes

After the 1.2.0 run, the **dev split only** was read (37 flags; the held-out 40
were not) and two false positives were fixed: an import-only helper was being
wrapped as `it("fragment")` and flagged RG08; and a test whose title says the
attack is blocked (or carries the secure suite's ✓/✔) is a control test. The
held-out split was then labelled **first**, under the rubric, with the 1.3.0
binary (`795211ec…`, code `fcba51d`). 63 flags were raised (32 held-out, 31
dev); the held-out labels are in `labels/heldout.jsonl`.

| Rule | Held-out precision |
|---|---|
| RG02 privileged-step-in-exploit | 0/16 = 0.0% [0.0-19.4] |
| RG03 deployer-is-attacker | 0/5 = 0.0% [0.0-43.4] |
| RG08 no-assertion | 10/10 = 100% [72.2-100] |
| RG09 tautological-assertion | 0/0; one unclear, a count |
| **Pooled** | **10/31 = 32.3% [18.6-49.9]** |

Source-clustered bootstrap for the pooled cell (6 sources, 10,000 draws, seed
`rg-solana-poc-v2-bootstrap-2026-11-25`): **[10.0-72.7]** — wider than the
Wilson interval, as the P1 study predicted for clustered flags.

What the labels say, rule by rule. RG08 is right on every held-out flag: the
PoC-framework binaries print balances and assert nothing. RG02 and RG03 are
wrong on every held-out flag: the signer is the fee payer of an instruction
whose account metas mark the role account non-signing (the Merovingio
`authority` cases), the attacker's own fixture key, or a setup helper. The
static reader cannot yet tell a payer from a role signer. A rule at `warn`
with 0/16 and 0/5 fresh precision is noise, so 1.4.0 moves RG01, RG02, RG03,
RG04 and RG06 to `note` on Solana until a reader can make that distinction.

The judged-readability target for the P1 set remains missed (6/12); nothing in
round 2 changes coverage (73/81 v2, 93/99 in-sample).

The dev split has **no committed label file**: it was read to find the two
fixes and is not a measurement. Only the held-out labels are committed
(`labels/heldout.jsonl`); `measure.py` reads nothing else.

No human has labelled this corpus. `labels/HUMAN_PACKET.md` is the blind
packet for the 32 held-out flags and `score_human.py` scores a returned CSV for
agreement and Cohen's kappa; until a human returns it, the labels are one AI
reader's and the article says so.
