# Solidity PoC corpus v2 — additive P2R-2 corpus (sealed at collection)

Collected by the P2R-2 researcher under §3.2 of
`_orchestration/PROMPT_P2R2_FINISH_AND_MEASURE_2026-11-26.md` and the §3.3
protocol of `_orchestration/PROMPT_PRECISION_REBUILD_2026-11-25.md`, under
`eval/P2R_AMENDMENT_2.md`. It is additive to `eval/solidity-poc-v1/`: v1's units
and split assignments are never edited, and no v1 unit is reused. Its purpose
is platform diversity (Code4rena ≤ 60% of J across v1+v2 combined) and X.
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
`rg-solidity-v2-salt-bb3d119768be2a49b8417195654a5d66`

FINAL enlargement, if needed at collection close: move units from DEV in
ascending `sha256(salt|unit_id)` order; V1/V2 are never raided.

## Collection rules

The v1/v3 rules apply unchanged (search-log row per candidate; source URL, pin,
licence, outcome quote ≤ 15 words from the original source, tier, kind read at
collection time; content-based block selection with v1's `select_sol_block`;
one counted unit per judged finding; pointer plus hash for unlicensed text;
dedupe by hash and URL against every opened and sealed corpus, including
`eval/solidity-poc-v1/` and the opened DeFiHackLabs pin `d4bd931`). No
realism-gate run and no labelling on any unit at collection time.
