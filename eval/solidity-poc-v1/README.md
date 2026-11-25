# Solidity PoC corpus v1 — sealed fresh corpus for the P2 precision rebuild

Collected and pinned by the P2R researcher (§3 of
`_orchestration/PROMPT_PRECISION_REBUILD_2026-11-25.md`). This README, the
search log and the inventory are committed before any `p2/*` code reads a unit.
Solidity is measured on its own slices and never pooled with Solana.

## Status

**Collection closed 2026-11-26 (platform-diversity pass).** Counted units:
**J 520 distinct judged findings** — Code4rena 477 (91.73%), CodeHawks EVM 41
(7.88%), Hats Finance 2 (0.38%) — **X 36** (18 real-incident reproductions, 18
CTF teaching exploits), **N 30** (secure-path tests). Split: DEV 234, V1 111,
V2 95, FINAL 146.

J meets the target at source level (520 >= 120, 117 sources, max source 4.04%
of J). The platform share is no longer 100% Code4rena, but Code4rena still
dominates at 91.73%; both the source-level and platform-level shares are in
`inventory.json` and `SEARCH_LOG.md`. The CodeHawks EVM units are 41 judged
report groups from four Wayback-pinned contest results (2025-07-orderbook 5,
2025-12-multisig-timelock 13, 2026-02-stratax-contracts 12,
2026-04-snarkeling 11; 9 High, 14 Medium, 18 Low). Hats Finance contributes 2
findings from the Euro-Dollar competition (High, Low): the other valid findings
carry no Foundry test, and Mento yields none.

X misses 150: DeFiHackLabs and DeFiVulnLabs are opened data (only 9 new
DeFiHackLabs incident files and 0 new DeFiVulnLabs files since their pinned
commits), Immunefi publishes 9 Foundry PoCs, and the remaining reachable
material is 18 distinct CTF teaching exploits. The arithmetic is in
`SEARCH_LOG.md`.

Every J unit carries a `codebase_id` into the top-level `codebases` map: 507 of
520 pinned (97.5%). The 13 nulls are 11 Code4rena units from six contests with
no reachable sponsor repo and the 2 Hats units (the Hats repository names no
audited codebase repo). The four CodeHawks contests pin their contest repos and
commits. Units with `codebase_id: null` are collected but the program model
cannot run on them.

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

Sources searched: Code4rena findings repos (cloned; 2,457 High/Medium report
sections, 477 with Foundry test code), Sherlock judging repos, Cantina public
competition reports, CodeHawks, Hats, Immunefi audit competitions,
audit-firm PoC write-ups,
DeFiHackLabs/SunWeb3Sec for the X tier.
