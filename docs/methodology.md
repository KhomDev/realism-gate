# Methodology: how realism-gate's precision is measured

> **Status for the release on `main`: DEV-ONLY.** Every number this release states for the P2 binaries
> comes from opened (DEV or tuning) data. No held-out precision is claimed. The held-out rounds come
> after main, under `eval/P2R2_PREREGISTRATION_POST_MAIN.md`.

## The question
For each rule and language: when realism-gate flags a test, how often is the flag's claim true?
Precision is TP / (TP + FP), computed over the flags that were labelled. Recall is not measured,
because there is no ground truth for unflagged defects.

## Units, corpora and tiers
- A **unit** is one PoC test, pinned by commit and content hash. The corpora store pointers, not text.
  `eval/<corpus>/fetch.py` rebuilds the cache, and `run.py --check` re-verifies every pin offline.
- Corpora: `solana-poc-v3`, `solana-poc-v4`, `solana-poc-v5` (Solana) and `solidity-poc-v1`,
  `solidity-poc-v2` (Solidity). `solidity-poc-v3` is **excluded**: its 35 units are near-copies of
  the opened DeFiHackLabs tree. Counts and hashes are in the pre-registration §2.
- Tiers: **J** is judged by a platform (Code4rena, CodeHawks, Hats, Immunefi, Audit Arena). **X** is
  an exploit with an external verdict. **A** is the author's own claim, reported separately and never
  pooled. **N** is a negative control (RQ4 only).

## Splits
`int(sha256(salt|id)[:8],16) % 100` per corpus: DEV 0-39, V1 40-59, V2 60-74, FINAL 75-99. Only DEV
has been opened. A runner refuses `--split V1|V2|FINAL` unless the round, version and binary sha256 are
real and match the binary.

## Pipeline
1. `run.py BINARY --split DEV` runs the frozen binary on every unit and writes
   `runs/<v>.jsonl` (readability), `flags/<v>.jsonl` (every active finding, as a hash rather than text),
   and `sample.jsonl` (the flags to label).
2. `labels.py` / `p2r_packet.py` build blind packets. They carry no severity and no other labeller's
   label.
3. Labellers follow [`labeling-rubric.md`](labeling-rubric.md). Kappa must be at least 0.70, and every
   disagreement gets a written adjudication.
4. `measure.py` turns labels into cells: Wilson 95% (z = 1.959964), n < 5 reported as a count, more
   than 20% `unclear` reported as inconclusive, and a 10,000-draw source-clustered bootstrap on pooled
   cells.

## From precision to shipped severity (RG-A5)
shipped = min(raised, floor cap) per rule × language. The cap is `error` with a fresh cell where
Wilson LB ≥ 0.80 and n ≥ 16, `warn` where LB ≥ 0.50 and n ≥ 10, and `note` otherwise. Only **fresh**
cells count. No committed cell is fresh for the P2 binaries, so **every rule ships `note`** on `main`.
Reports print the raised level next to the shipped one, and `--severity RULE=LEVEL` overrides it.
Rules with no fresh cell read "not yet measured on fresh data".

## What is never done
- Pooling Solana with Solidity, one binary with another, or DEV/opened data with fresh data.
- Tuning on a sealed slice, or running `--check` with a placeholder round, version or sha on one.
- Counting `unclear` as TP or FP, or dropping unreadable units.

## Where the numbers are
- DEV and in-sample tables: [`precision.md`](precision.md) (the DeFiHackLabs in-sample rows, and the
  1.4.0 Solana rows as history) and [`benchmark.md`](benchmark.md) (raised vs shipped over 658
  DeFiHackLabs findings).
- Limits: [`limitations.md`](limitations.md). Reproduce: [`reproducibility.md`](reproducibility.md).