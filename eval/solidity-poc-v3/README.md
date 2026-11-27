# Solidity PoC corpus v3 — additive P2R-2 corpus (sealed at collection)

Collected under `eval/P2R_AMENDMENT_2.md` and the dataset-extension rules of
2026-11-27. It is additive to `eval/solidity-poc-v1/` and
`eval/solidity-poc-v2/`: those units and split assignments are never edited,
and no earlier unit is reused. This corpus is for Solidity **X** (real-incident
reproductions). It does not add Code4rena Solidity J.

Solidity is measured on its own slices and never pooled with Solana.

## Status

**Scaffold committed before any unit was collected.** The salt below is fixed by
this commit. Collection results, counts and the seal are recorded in
`SEARCH_LOG.md` and `inventory.json` by later commits.

## Tiers

J, X, N as in v1; A and AJ as defined in Amendment 2 (separate cells, never
pooled with J or X).

## Split

Identical rule to v1/v3: `int(sha256(salt + "|" + unit_id)[:8], 16) % 100`:
0–39 DEV, 40–59 V1, 60–74 V2, 75–99 FINAL.

Salt (committed before any unit is collected):
`rg-solidity-v3-salt-e87258db84526f58a694424d3fae3c63`

FINAL enlargement, if needed at collection close: move units from DEV in
ascending `sha256(salt|unit_id)` order; V1/V2 are never raided.

On 2026-11-27, `origin/p2/integration` has no `FINAL_PREREGISTRATION.md`, and
the tip commit says no slice has been opened. Units sealed on this branch
therefore take the split above as written.

## Collection rules

The v1/v3 rules apply unchanged (search-log row per candidate; source URL, pin,
licence, outcome quote ≤ 15 words from the original source, tier, kind read at
collection time; content-based block selection with v1's `select_sol_block`;
one counted unit per incident file; pointer plus hash for unlicensed text;
dedupe by hash and URL against every opened and sealed corpus, including
`eval/solidity-poc-v1/`, `eval/solidity-poc-v2/` and the opened DeFiHackLabs
pin `d4bd931`). No realism-gate run and no labelling on any unit at collection
time.
