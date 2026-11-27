# P2R-2 analysis plan (committed before any V1, V2 or FINAL slice is opened)

Supplements `eval/P2R_PREREGISTRATION.md` and `eval/P2R_AMENDMENT_2.md`; where they speak, they govern.
This file fixes the cell definitions the amended verdict needs, so they cannot be chosen after a slice
is seen. Committed 2026-11-27, with V1, V2 and FINAL untouched in every corpus.

## 1. Which runs form a cell
- **Solana** cells pool `solana-poc-v3` and `solana-poc-v4` units of the same slice, run by the same
  frozen binary (`eval/solana-poc-v3/run.py`, `eval/solana-poc-v4/run.py`). One judged finding is one unit
  (duplicate submissions are skipped by both runners).
- **Solidity** cells pool `solidity-poc-v1` and `solidity-poc-v2` units of the same slice, same binary.
- Solana and Solidity are never pooled. Different binaries are never pooled. In-sample (DEV, opened
  slices) and fresh numbers are never pooled.

## 2. Tiers per cell (Amendment 2)
| Cell | Tiers | Enters the verdict |
|---|---|---|
| Readability, verdict | J + X | yes |
| Pooled precision, verdict | J + X | yes |
| Per-rule precision | J + X | yes (floor) |
| J-only cells | J | no — "descriptive: insufficient public judged data" (Solana) |
| Tier A | A | no — separate descriptive cell |
| Tier AJ | AJ | no — invalid-PoC negative class only (RQ3/RQ4) |
| Negative controls | N | RQ4 only |

`measure.py --tiers J,X` produces the verdict cells; `--tiers A` and `--tiers J` the descriptive cells.
A and AJ are refused in any pool with J, X or N.

## 3. "Rules shipped above `note`"
RG-A5 derives shipped severity from fresh cells by the floor (warn: Wilson LB ≥ 0.50 with n ≥ 10; error:
LB ≥ 0.80 with n ≥ 16). No committed cell is fresh for the P2 binaries, so every rule ships `note` until
FINAL. Therefore:
- At FINAL, each rule × language is first measured on FINAL J+X; its shipped severity is derived from that
  cell by the floor; **"rules shipped above note"** are the rule × language cells whose FINAL cell reaches
  `warn` or `error` under the floor.
- The verdict's pooled-precision row is computed over those cells' labelled flags. Because the same FINAL
  labels select the cells and measure them, this cell is selection-biased upward; it is reported **beside**
  the unselected **pooled-all J+X** precision with its interval, and the report states the bias. If no
  cell reaches the floor, "above note" is empty and the row is reported as "no rule ships above note".
- V1 and V2 report both pooled-all and per-rule cells; they never set a shipped severity.

## 4. Sampling, labels, statistics
- Sampling: each runner's seeded per-(source, rule) cap of 30 flags over J and X units; A units are sampled
  separately with the same cap. Seeds are the runners' constants (`rg-solana-poc-v3-2026-11-25`,
  `rg-solana-poc-v4-2026-11-27`, `rg-solidity-poc-v1-2026-11-26`, `rg-solidity-poc-v2-2026-11-27`).
- Labels: rubric v3 (`P2R_PREREGISTRATION.md`), two blinded AI readers (B blind to A and to severity),
  Cohen's kappa ≥ 0.70 or the cell is inconclusive, written adjudication of every disagreement.
- Wilson 95% on every k/n; n < 5 is a count; > 20% unclear is inconclusive; the pooled cell carries the
  10,000-draw source-clustered bootstrap (seed `p2r-bootstrap-2026-11-25`).
- Readability = units with ≥ 1 test read / units, by tier, source and kind.

## 5. Rounds
- Round 1 freeze: F1, F2, F3, F4, F5, F6 (RG-A5, RG-A6) merged and verified, CI green, binary sha256 and
  rubric version recorded in `eval/rounds/round-1.json` before V1 is opened. V1 is opened once per corpus.
- Round 2: class-level fixes from DEV + V1 (V1 becomes in-sample), RG-A4 `--execute`, freeze, V2 once.
- FINAL: `FINAL_PREREGISTRATION.md` committed, final freeze, FINAL once per tool.
- New data sealed after a slice opens follows `_orchestration/DATASET_EXTENSION_RULES_P2R2.md` §4.
