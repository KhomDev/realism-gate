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
