# Solana PoC corpus v3 — sealed fresh corpus for the P2 precision rebuild

Collected and pinned by the P2R researcher (§3 of
`_orchestration/PROMPT_PRECISION_REBUILD_2026-11-25.md`). This README, the
search log and the inventory are committed before any `p2/*` code reads a unit.

## Status

**Collection closed 2026-11-25, not sealed, not run.** Counted units: **J 58
submissions = 20 distinct judged findings** (Arena 11, CodeHawks RustFund 8
groups, Code4rena 1), X 60, N 16. Split: DEV 50, V1 27, V2 29, FINAL 28. The 46
RustFund submissions are 8 groups and 79% of J by submission, so the 40% source
rule and §10's FINAL minimum of 30 J cannot both hold; under the corrected unit
discipline the whole corpus holds only 20 distinct J findings, so a 30-unit
FINAL minimum is unreachable outright. The arithmetic is in `SEARCH_LOG.md`.

Blocks are selected by content, not by fence label: TOML, shell, diff and
test-log blocks are rejected, the first block with a test-driver marker is
taken (preferring one that names the program under test), `lang` is inferred
from the content and the original fence label is kept as `fence_label`. Two
previously selected units lost their block under this rule and were dropped;
63 units changed block index or inferred language. Three hundred
ninety-eight candidate issues carried no qualifying block and are listed as
screened out, none kept as a unit.

Every unit carries `group_id`, `group_size` and `duplicate_of` when it belongs
to a multi-submission judging group (RustFund: one representative per group,
first by sorted unit id; the other 38 point at it). Arena and Code4rena J units
are their own finding group (`group_size` 1). `severity_source` records where
the J severity came from. The published judge/platform severity always wins over
author text: Arena uses the maintainer severity label (`issue-label`; only when
no severity label exists does it fall back to the title bracket as
`issue-title-bracket-fallback`), RustFund uses the results group field, and
Code4rena uses the report heading. `severity_title` keeps the submitter's title
bracket and `severity_conflict` marks the six Arena units where it differs from
the judge (144, 147, 201, 203, 210, 280); `outcome_evidence` quotes both when
they conflict.

Files: `inventory.json` (units, pins, splits, outcomes), `build_inventory.py`
(rebuilds and `--check`s it from the cache), `fetch.py` (fetches every pin;
`--check` verifies offline), `SEARCH_LOG.md` (every query and rejection).

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
