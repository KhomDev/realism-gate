# Solidity PoC corpus v1 — sealed fresh corpus for the P2 precision rebuild

Collected and pinned by the P2R researcher (§3 of
`_orchestration/PROMPT_PRECISION_REBUILD_2026-11-25.md`). This README, the
search log and the inventory are committed before any `p2/*` code reads a unit.
Solidity is measured on its own slices and never pooled with Solana.

## Status

**Collecting.** No unit is sealed or run yet.

## Tiers and targets

| Tier | Meaning | Target |
|---|---|---|
| J | coded PoC (Foundry test) with a published platform/sponsor outcome | ≥ 120 units, ≥ 6 independent sources, no source > 30% |
| X | exploit-by-construction reproduction (DefiHackLabs, incident write-ups with Foundry PoCs) | ≥ 150 units |
| N | negative controls (secure-path tests, judge-rejected PoCs) | ≥ 30 units |

## Split

Assigned by `int(sha256(salt + "|" + unit_id)[:8], 16) % 100`:
0–39 DEV, 40–59 V1, 60–74 V2, 75–99 FINAL. Salt (committed before collection
ends): `rg-solidity-v1-salt-e3c5336cedf1f185f432914f5f2ab1ec`.

FINAL enlargement, if below the §10 minimum at collection close: move units
from DEV in ascending `sha256(salt|unit_id)` order; V1/V2 are never raided.

## Collection rules

The Solana v3 rules apply unchanged: a search-log row per candidate; per unit
the source URL, pinned commit or body hash, licence, the outcome quote in ≤ 15
words, tier, and the kind read at collection time; dedupe by hash and URL;
pointer-plus-hash for unlicensed text; the existing `eval/` corpora are DEV
and are not reused as fresh.

Sources: Code4rena findings repos (labels include `selected for report`,
`sufficient quality report`, `satisfactory`, `unsatisfactory`, `invalid`,
`duplicate-N`), Sherlock judging repos, Cantina public competition reports,
CodeHawks, Hats, Immunefi audit competitions, audit-firm PoC write-ups,
DeFiHackLabs/SunWeb3Sec for the X tier.
