# Search log — solana-poc-v4

P2R-2 §3.1 researcher, 2026-11-26. Scaffold and salt committed first (`60698e0`), before any
unit was collected. `gh api` calls spaced >= 7 s. Every source is unlicensed for our purposes,
so the repository stores pointers and sha256 hashes only; text lives in the gitignored
`.corpus/solana-poc-v4/`. No realism-gate run and no label was made on any unit.

## Result, uncomfortable part first

**The amended Solana FINAL minimum (>= 60 labelled flags from >= 4 source clusters) is
unreachable on public data.** v4 adds 4 distinct judged findings (J, 6 submissions) and 7
auditor-verified findings (tier A, separate cell). None of the 4 J findings hashes into FINAL
(DEV 2, V1 2). FINAL J+X therefore stays at v3's 16 distinct units (X arena 13, J rustfund 2,
J arena 1) from 2 clusters: at the v3 DEV rate of 33 flags / 50 units that is about 11 flags.
Even the committed enlargement rule applied to every DEV J+X unit of v3 and v4 gives 53 units
(about 35 flags) from 3 clusters, 87% Arena. The verdict row cannot be met; say so before
Round 1.

## Leads

| Lead | Expected | Done | Yield |
|---|---|---|---|
| Code4rena Lavarage Invitational `code-423n4/2024-04-lavarage-findings` | 6 submissions = 4 findings | Cloned findings repo @ `108abd5`; `gh api .../issues?state=all` (34 issues, labels + bodies). v3 `select_block` on each body. | **6 submissions = 4 findings, as expected**: #16 (H-02) + #28 (duplicate-16); #26 (H-01) + #13 (duplicate-26); #25 (QA, satisfactory, grade-a); #2 (QA primary, Q-05). 28 other issues carry no qualifying block. Sponsor snapshot `code-423n4/2024-04-lavarage`: `git ls-remote` → "Repository not found"; `model_target.commit` is null. |
| Sherlock ZetaChain Cross-Chain 2025 Solana gateway | 0-3 | `gh api repos/sherlock-audit/2025-04-zetachain-cross-chain-judging` → 404 (private). Final report read in the mirror (finder only): 43 issues, 12 Solana-related. | **0.** No Solana finding carries test code; the only test block (M-1) is Sui Move. |
| Warden portfolios (Jupiter Lend, Pump Science, Orderly, WOOFi, Lavarage, RustFund) | a few | `AnticsDecoded/audits` tree read: `reports/poc/jupiter-lend-flashloan-dust.rs`, claimed "Validated severity: Low". The public Code4rena Jupiter Lend report (v3 cache) names no such item and never names the warden. `gh search repos` "lavarage", "pump-science", "rustfund", "lavarage poc", "pump science code4rena poc", "rustfund poc test", "orderly solana sherlock poc", "woofi solana poc"; `gh search code` "lavarage-findings", "2025-01-pump-science". | **0.** AnticsDecoded credit not verifiable → rejected. Pump Science portfolios (e.g. `evokid/code4rena-audits`) cover P1-opened findings. No Lavarage/RustFund/Orderly/WOOFi portfolio with a PoC for a credited finding found. |
| Cantina private reviews of Solana programs (tier A) | unknown | Mirror `index/findings/cantina` (sparse checkout of `DicksonWu654/web3-audit-findings` @ `8bc87a9`): 436 findings with `chains_json` containing solana, in 24 reports. Downloaded all 24 original PDFs from `cdn.cantina.xyz` (13 MB), `pdftotext -layout`, grepped for test drivers. | **7 A** from `cantina_solana_foundation_mar2026.pdf` (14 findings, 8 with a "Proof of Concept" test; 3.1.1 has no test-driver marker). The other 23 reports carry no test code (two mention LiteSVM in prose only). |
| CodeHawks AI First Flight RustFund re-runs (tier AJ) | many | Wayback CDX `codehawks.cyfrin.io/c/*` (3,650 URL keys, collapse=urlkey) read once: the only `ai-` contest archived is `ai-mycut-...` (Solidity). Later CDX calls ("ai-" prefix) returned "Internet Archive: Temporarily Offline". Live site not touched (Vercel checkpoint per v3 log). | **0.** No archived `ai-rust-fund-*` page exists in the CDX listing read. |
| Mirror index as finder for other Solana contests | — | Used for ZetaChain and Cantina above; Frank Castle / Pashov / Olas 2023-12 / DODO 2025-05 already known dead ends (not re-read). | — |

## Content-based selection notes

- Lavarage: blocks are TypeScript Anchor tests (`program.methods` / `describe(`).
- Cantina PDF: a PDF has no fences. The block is the text from "Proof of Concept:" to
  "Recommendation:", cut where a test log begins (the log is a separate block in the original;
  v3's `LOG_LINE`), then run through v3 `select_block`. Extraction is pinned to
  `pdftotext -layout -enc UTF-8` (xpdf 4.06) and re-verified by `fetch.py`.

## Units taken

| Tier | Source | Submissions | Distinct findings | Split (distinct) DEV/V1/V2/FINAL |
|---|---|---|---|---|
| J | code4rena-lavarage | 6 | 4 | 2 / 2 / 0 / 0 |
| A | cantina-solana-foundation-multidelegator | 7 | 7 | 3 / 1 / 1 / 2 |

Dedupe: 0 content-hash or URL matches against every opened and sealed corpus
(`eval/corpus.json`, `eval/solana-poc*/`, `eval/solidity-poc-v1/`, `eval/near-poc-v1/`).

## What remains

- CodeHawks AI First Flight re-runs: retry the Wayback CDX when the archive is back; if no
  `ai-rust-fund` key exists, AJ cannot be built from public data.
- Cantina competition reports (71) were not screened for Solana beyond the chains filter.

## Verification

`python -I eval/solana-poc-v4/build_inventory.py --check` → `inventory.json ok`;
`python -I eval/solana-poc-v4/fetch.py --check` → all unit pins verified (offline).
