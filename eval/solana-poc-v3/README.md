# Solana PoC corpus v3 — sealed fresh corpus for the P2 precision rebuild

Collected and pinned by the P2R researcher (§3 of
`_orchestration/PROMPT_PRECISION_REBUILD_2026-11-25.md`). This README, the
search log and the inventory are committed before any `p2/*` code reads a unit.

## Status

**Collecting.** No unit is sealed or run yet. The first seal is the Round-1
freeze; any unit added after a round starts joins a later slice only.

## Tiers and targets

| Tier | Meaning | Target |
|---|---|---|
| J | coded PoC with a published platform/sponsor outcome | ≥ 60 units, ≥ 4 independent sources, no source > 40% |
| X | exploit-by-construction reproduction, no judgement | ≥ 60 units |
| N | negative controls (secure-path tests, judge-rejected PoCs) | ≥ 15 units |

## Split

Assigned by `int(sha256(salt + "|" + unit_id)[:8], 16) % 100`:
0–39 DEV, 40–59 V1, 60–74 V2, 75–99 FINAL. Salt (committed before collection
ends): `rg-solana-v3-salt-2b03962612050c100daeb8128706254e`.

FINAL enlargement, if below the §10 minimum at collection close: move units
from DEV in ascending `sha256(salt|unit_id)` order; V1/V2 are never raided.

## Collection rules

1. Every candidate gets a row in `SEARCH_LOG.md`: query, date, URL, verdict
   (taken/rejected + reason), licence.
2. Every unit records: source URL; pinned commit or issue-body sha256; licence
   (copy or pointer-only); the published outcome with its URL quoted in ≤ 15
   words; tier; kind read from the file alone at collection time.
3. Dedupe against every opened corpus (`eval/solana-poc/`,
   `eval/solana-poc-v2/`) by content hash and source URL.
4. Third-party unlicensed text is stored as a pointer plus hash, never copied.
5. `fetch.sh --check` must verify every pin offline before a round opens.
6. Source classes: Code4rena, Sherlock, Cantina, CodeHawks, Hats, Immunefi,
   Audit Arena (≤ 40%), Superteam, audit-firm write-ups with code, CTF solves
   with a published judgement, DeFiHackLabs/SunWeb3Sec for the X tier.

## What is already exhausted (do not reuse as fresh)

`eval/solana-poc/` and `eval/solana-poc-v2/` (opened; now DEV), and the
sources recorded in their search logs. The P1-C Arena source may not exceed
40% of J here.
