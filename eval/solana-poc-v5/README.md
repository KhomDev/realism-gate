# Solana PoC corpus v5 — additive P2R-2 corpus (sealed at collection)

Collected under `eval/P2R_AMENDMENT_2.md` and the dataset-extension rules of
2026-11-27. It is additive to `eval/solana-poc-v3/` and `eval/solana-poc-v4/`:
those units and split assignments are never edited, and no earlier unit is
reused. RustFund and the Solana Audit Arena are not sources for this corpus.

## Status

**Collection closed 2026-11-27, sealed, not run.** Counted units: J 2
(Code4rena Meteora Dynamic Bonding Curve), X 1 (Cashio infinite-mint
reproduction), A 3 (OtterSec Port Sundial; separate cell, not pooled).
J+X source clusters: 2. The amended FINAL minimum (≥ 60 labelled flags from
≥ 4 clusters) is not met by this corpus; the search log lists the dead ends.
Salted split of the 6 counted units: DEV 4, V1 2, V2 0, FINAL 0. FINAL was
not enlarged. The salt below was fixed by the scaffold commit before any
unit was collected.

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
`rg-solana-v5-salt-72b381bcd3a9c789cbdf56fa727ae773`

FINAL enlargement, if needed at collection close: move units from DEV in
ascending `sha256(salt|unit_id)` order; V1/V2 are never raided.

On 2026-11-27, `origin/p2/integration` has no `FINAL_PREREGISTRATION.md`, and
the tip commit says no slice has been opened. Units sealed on this branch
therefore take the split above as written.

## Collection rules (P2R §3.3, unchanged)

1. Every candidate gets a row in `SEARCH_LOG.md`: query, date, URL, verdict
   (taken/rejected + reason), licence.
2. Every unit records: source URL; pinned commit or body/page sha256; licence
   (copy or pointer-only); the published outcome with its URL quoted in ≤ 15
   words, taken from the original source (never from a list or mirror); tier;
   kind read from the artifact alone at collection time.
3. Blocks are selected by content with the v3 selector (`select_block` in
   `eval/solana-poc-v3/build_inventory.py`), never by fence label.
4. Grouping by judged finding: duplicates carry `group_id`/`duplicate_of`; one
   counted unit per judged finding.
5. Dedupe by content hash and source URL against every opened and sealed
   corpus: `eval/solana-poc/`, `eval/solana-poc-v2/`, `eval/solana-poc-v3/`,
   `eval/solana-poc-v4/`, `eval/solidity-poc-v1/`, `eval/solidity-poc-v2/`,
   `eval/corpus.json`, `eval/near-poc-v1/`.
6. Third-party unlicensed text is stored as a pointer plus hash only; the text
   lives in the gitignored `.corpus/solana-poc-v5/`.
7. `fetch.py --check` verifies every pin offline before a round opens.
8. No realism-gate run and no labelling on any unit at collection time.
