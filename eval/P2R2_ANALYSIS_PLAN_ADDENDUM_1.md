# P2R-2 analysis plan, addendum 1 (committed before any V1, V2 or FINAL slice is opened)

Supplements `eval/P2R2_ANALYSIS_PLAN.md`. Written 2026-11-27 by the orchestrator after the §3 verification
of the corpora merged in `2a9adcc` (PR #36). No V1, V2 or FINAL unit of any corpus has been run, labelled or
read; no Round-1 freeze exists yet.

## 1. Solana cells add `solana-poc-v5`
- Under `_orchestration/DATASET_EXTENSION_RULES_P2R2.md` §4, units sealed before the Round-1 freeze join
  V1/V2/FINAL by their salted split. `solana-poc-v5` was sealed at `0e58e02` (salt first at `1941b91`).
- Solana cells pool `solana-poc-v3`, `solana-poc-v4` and `solana-poc-v5` units of the same slice, run by
  the same frozen binary (`eval/solana-poc-v5/run.py` added). Sampling seed for v5:
  `rg-solana-poc-v5-2026-11-27`, same per-(source, rule) cap of 30.
- v5 contributes J 2 (DEV 1, V1 1), X 1 (DEV), A 3 (DEV 2, V1 1). **FINAL gains no unit**, so the amended
  Solana FINAL minimum stays unreachable (statement already recorded before Round 1).
- Pin change before any opening: the two Meteora units were pinned to a sha256 of the live Code4rena
  report page, which is re-rendered (the online re-fetch on 2026-11-27 gave a different page hash with the
  same blocks). The page hash is removed; each unit stays pinned by its block's normalised hash, as was done
  for the v4 dacian unit. Ids, splits and block hashes are unchanged.

## 2. `solidity-poc-v3` is not a verdict source (pending the operator's decision)
- All 35 counted units come from `twabi/DeFi-Reentrancy-Hacks-Repository`. Every one has a same-named test
  in the **opened** DeFiHackLabs tree `d4bd931` (the tuning population); line similarity after comment
  stripping: median 0.935, 22 of 35 ≥ 0.90, 30 of 35 ≥ 0.70 (`_orchestration/_verify/verify_s3_2026-11-27.*`).
  These are near-copies of in-sample data, not a new source cluster, and not fresh.
- Until the operator decides (exclude the corpus, or keep it as an in-sample descriptive cell), no
  `solidity-poc-v3` unit enters a V1, V2 or FINAL cell, and it has no runner and no `p2r_corpora` entry.