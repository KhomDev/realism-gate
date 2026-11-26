# Solana PoC corpus v4 — additive P2R-2 corpus (sealed at collection)

Collected by the P2R-2 researcher under §3 of
`_orchestration/PROMPT_P2R2_FINISH_AND_MEASURE_2026-11-26.md` and the §3.3
protocol of `_orchestration/PROMPT_PRECISION_REBUILD_2026-11-25.md`, under
`eval/P2R_AMENDMENT_2.md`. It is additive to `eval/solana-poc-v3/`: v3's units
and split assignments are never edited, and no v3/v2/v1 unit is reused.

## Status

**Collection closed 2026-11-26, sealed, not run.** Distinct findings: J 4 (6 submissions, Code4rena Lavarage), A 7 (Cantina, separate cell). FINAL J+X gains 0 units; the amended Solana FINAL minimum is unreachable (see SEARCH_LOG.md). The salt below was fixed by the scaffold commit before any unit was collected.

## Tiers

| Tier | Meaning | Reported |
|---|---|---|
| J | coded PoC with a published platform/sponsor outcome | pooled with X for the FINAL verdict |
| X | exploit-by-construction reproduction of a real incident, no judgement | pooled with J |
| N | negative controls (secure-path tests, judge-rejected PoCs) | separately |
| A | auditor-verified: an audit firm's finding with code and a sponsor status (Amendment 2) | separate cell, never pooled |
| AJ | judged by an AI judge (e.g. CodeHawks AI First Flights); invalid-PoC negative class only (Amendment 2) | separate cell, never pooled |

## Split

Identical rule to v3: `int(sha256(salt + "|" + unit_id)[:8], 16) % 100`:
0–39 DEV, 40–59 V1, 60–74 V2, 75–99 FINAL.

Salt (committed before any unit is collected):
`rg-solana-v4-salt-d43343efde75d096d1005d85de7aef82`

FINAL enlargement, if needed at collection close: move units from DEV in
ascending `sha256(salt|unit_id)` order; V1/V2 are never raided.

## Collection rules (P2R §3.3, unchanged)

1. Every candidate gets a row in `SEARCH_LOG.md`: query, date, URL, verdict
   (taken/rejected + reason), licence.
2. Every unit records: source URL; pinned commit or body/page sha256; licence
   (copy or pointer-only); the published outcome with its URL quoted in ≤ 15
   words, taken from the original source (never from a list or mirror); tier;
   kind read from the artifact alone at collection time.
3. Blocks are selected by content with the v3 selector (`select_block` in
   `eval/solana-poc-v3/build_inventory.py`), never by fence label.
4. Grouping by judged finding (the RustFund rule): duplicates carry
   `group_id`/`duplicate_of`; one counted unit per judged finding.
5. Dedupe by content hash and source URL against every opened and sealed
   corpus: `eval/solana-poc/`, `eval/solana-poc-v2/`, `eval/solana-poc-v3/`,
   `eval/solidity-poc-v1/`, `eval/corpus.json`, `eval/near-poc-v1/`.
6. Third-party unlicensed text is stored as a pointer plus hash only; the text
   lives in the gitignored `.corpus/solana-poc-v4/`.
7. `fetch.py --check` verifies every pin offline before a round opens.
8. No realism-gate run and no labelling on any unit at collection time.
