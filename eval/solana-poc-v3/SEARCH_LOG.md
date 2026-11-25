# Search log — Solana PoC corpus v3

Collected 2026-11-25 by the P2R researcher (Workstream R, Phase 1). Every query
below was run on that date. `gh api` calls were spaced >= 8 s (10/min budget);
web reads used the built-in fetch/search tools. Licences: every source below is
unlicensed for our purposes (no licence file, or user-submitted contest text),
so the repository stores pointers and sha256 hashes only; the text lives in the
gitignored `.corpus/solana-poc-v3/`.

## Result in one paragraph, with the uncomfortable part first

The fresh public judged-Solana universe is **59 coded J units, not 60**, and it
is **80% one beginner contest**: 47 from CodeHawks First Flight #36 (RustFund),
11 from the Solana Audit Arena's remaining labelled issues, 1 from Code4rena
Jupiter Lend. X is 60 (all Arena unlabelled submissions) and N is 16. Because
the four candidate classes named in the prompt (Code4rena, Sherlock, Cantina,
CodeHawks, Hats, Immunefi, audit firms, Solana-specific, incident repos) were
each searched and the fresh coded supply is this thin, **the 40% source cap
cannot be satisfied on this corpus**: a strict origin-level cap admits at most 5
J units (2 Arena + 2 RustFund + 1 Code4rena), and the FINAL slice can reach at
most 11 + 18 = 29 J units even if every DEV J unit is moved to FINAL under the
committed enlargement rule — one short of §10's minimum of 30. The cap and the
FINAL minimum are reported here rather than engineered around.

## Source-class coverage

| Class the prompt required | What was done | Result |
|---|---|---|
| Code4rena findings repos | Listed all 881 `code-423n4` repos; grepped every Solana-ecosystem name. Read the three Solana report pages and the two findings repos that exist. | Solana contests: Anchor 2022 (76 issues, no test), Pump Science 2025-01 (P1-exhausted), Solana Foundation 2025-08 (8 Low/QA items, no test code), Jupiter Lend 2026-02 (15 items, **1** with a Rust test). Findings repos for the last two return 404 (private). **1 J unit.** |
| Sherlock `*-judging` | Listed all 462 `sherlock-audit` repos. Only `2024-08-woofi-solana-deployment-judging` and `2024-09-orderly-network-solana-contract-judging` are Solana; P1 screened all 259 issue files and took 8. | **0 fresh.** |
| Cantina public competition reports | Tensor competition page read: 234 submissions, $150k, completed; no finding or PoC is public to anonymous readers (no /findings, no report link). Competitions index shows no Solana competition currently. | **0.** |
| CodeHawks (Cyfrin) | All 45 `CodeHawks-Contests` repos listed; code search `filename:Anchor.toml` identifies exactly two Solana contests: RustFund (2025-03) and SSSwap (2025-05). Live site is behind a Vercel security checkpoint (429). **Wayback snapshots of both results payloads were found and parsed.** RustFund: 326 judged submissions, 54 with a test-harness block, 47 unique. SSSwap: 10 valid findings, **no** submission body with a test block (Rust snippets only). | **47 J units** from RustFund. |
| Hats Finance | `org:hats-finance dappslap` = 0 repos; `org:hats-finance solana` = 0. Dappslap (Aug 2024, Solana/Anchor) has no public findings repository; its submissions are on-chain only. | **0.** |
| Immunefi audit competitions + `immunefi-team/*` | Read the reports.immunefi.com index (56 competitions): the only Solana program competitions are Jito Restaking (all findings used/screened in v2) and Firedancer v0.1 (C validator, no program frontend). `immunefi-team/bugfix-reviews-pocs` is Foundry/EVM only. | **0 fresh.** |
| Immunefi bug-fix reviews (bounty outcomes) | Both Raydium reviews read (Critical, $505,000): snippets and narrative only, no runnable test. | **0.** |
| Audit-firm write-ups with code (OtterSec, Neodyme, Zellic, Sec3, Halborn, OpenZeppelin, Trail of Bits, Spearbit, Cyfrin, Pashov) | Re-checked the firm orgs and report indexes. Runnable Solana PoCs remain the P1 sources (Neodyme/OtterSec/Ackee/Halborn). Sec3 publishes PDFs/blogs without runnable tests; Zellic/Spearbit are PDFs. | **0 fresh.** |
| Solana-specific: Audit Arena | Cloned and read all 349 issues and labels. 51 labelled (42 Valid, 6 duplicate, 3 invalid), 36 carry a harness block, 25 were used as v2 J. **11 labelled issues with test code are fresh** (8 Valid, 2 duplicate, 1 invalid). 145 unlabelled issues carry a harness block; 19 were v2 X. | **11 J**, **60 X**. |
| Solana-specific: Superteam | Searched Superteam Earn listings; submissions and per-finding judgements are not public in a form with coded PoCs. | **0.** |
| Solana Foundation disclosures | The 2025-08 Code4rena report is the only published disclosure set; no test code. GitHub advisories are program source to deploy, not tests (v2 screen). | **0.** |
| CTF solves with an organiser judgement | Re-checked the solved public CTF collections (OtterSec DeCave, Neodyme, Solhana, Halborn, Ackee): no organiser-published per-submission judgement beyond what P1/v2 used; unsolved templates were screened out in v2. | **0 fresh.** |
| Incident reproductions (X): DeFiHackLabs, SunWeb3Sec, Rekt | DeFiHackLabs/SunWeb3Sec are EVM-only. Solana incident repos (Loopscale, 1inch rescue, Merovingio, Jet) are v2 X units. | **0 fresh.** |
| Not in the prompt but searched | Solodit (Cyfrin aggregator, would index Cantina/CodeHawks/C4 findings): Vercel security checkpoint 429 on every request. HackerOne/Bugcrowd public Solana disclosures: no Solana-program disclosures with runnable PoCs found. | **0.** |

## Queries and reads

### `gh api` (>= 8 s apart)

| Query | Result | Verdict |
|---|---|---|
| `orgs/code-423n4/repos` pages 1-9 | 881 repos | scanned for Solana names |
| `orgs/sherlock-audit/repos` pages 1-5 | 462 repos | only Orderly/WOOFi Solana |
| `repos/code-423n4/2026-02-jupiter-lend/contents` | README, out_of_scope, scope | sponsor repo only |
| `repos/code-423n4/2025-08-solana-foundation/contents` | agave, token-2022, zk-elgamal, scope | sponsor repo only |
| `repos/code-423n4/2026-02-jupiter-lend-findings` | 404 | findings private |
| `repos/code-423n4/2025-08-solana-foundation-findings` | 404 | findings private |
| `search/repositories?q=org:hats-finance+dappslap` | 0 | no repo |
| `search/repositories?q=org:hats-finance+solana` | 0 | no repo |
| `orgs/CodeHawks-Contests/repos`, `search/code filename:Anchor.toml`, `Cargo.toml solana-program` | 45 contests, 2 Solana | RustFund + SSSwap |
| `search/repositories?q=rustfund` | 28 | participant repos only, not used as outcome sources |
| `search/repositories?q=ssswap` | 5 | nothing |
| `repos/Frankcastleauditor/Solana-Audit-Arena/issues` pages 1-4 | 349 issues with bodies and labels | 11 J + 60 X |
| single-issue re-fetch 141, 270, 2 | body sha256 matches | pins verified |

### Web reads and searches

| Read | Result | Verdict |
|---|---|---|
| `code4rena.com/reports/2026-02-jupiter-lend` | 3 Medium + 12 Low; L-07 only item with an executable Rust test | **1 J unit** |
| `code4rena.com/reports/2025-08-solana-foundation` | no High/Medium, 8 Low/QA, no test code | reject |
| `reports.immunefi.com` index | 56 competitions, Jito + Firedancer only Solana | reject |
| `cantina.xyz/competitions/2178…` (Tensor) | 234 submissions, findings not public | reject |
| `cantina.xyz/competitions` | no Solana competition | reject |
| `codehawks.cyfrin.io/c/2025-03-rustfund/results` (live) | Vercel checkpoint 429 | blocked |
| `codehawks.cyfrin.io/c/2025-05-ssswap/results` (live) | Vercel checkpoint 429 | blocked |
| Wayback CDX `codehawks.cyfrin.io/c/2025-03-rustfund*` | 4 snapshots incl. results | **used** |
| Wayback CDX `codehawks.cyfrin.io/c/2025-05-ssswap*` | 3 snapshots incl. results | parsed, 0 test code |
| `web.archive.org/.../20260128075213id_/…rustfund/results` | 326 submissions, 11 report groups | **47 J units** |
| `web.archive.org/.../20260130032108id_/…ssswap/results` | 10 report groups, snippets only | reject |
| `immunefi.com/blog/.../raydium-…` (both reviews) | $505,000 Critical, snippets/narrative only | reject |
| `code4rena.com/reports` index | no report list rendered | no new reports |
| `solodit.cyfrin.io/issues?keywords=…` | Vercel checkpoint 429 | blocked |
| WebSearch: Immunefi Attackathon Solana; Cantina findings; CodeHawks Solana; Hats Solana; Secure3 Solana; Dappslap | leads above only | — |
| `site:codehawks.cyfrin.io Solana contest` | RustFund, SSSwap | — |

### Wayback CDX (archive.org)

| Snapshot | Bytes | sha256 |
|---|---:|---|
| `.../20260128075213id_/…rustfund/results` | 3,970,590 | `63beb9b1d12e9fee8268c19eaed08b8728c6fef798504f2d16b7fd1602ecaf24` |
| `.../20260130032108id_/…ssswap/results` | 551,457 | `3ad862bac886b789b4c5bff3e5ea48cc4c2bfd123cdf9a27fff28df735c6eb37` |

## Units taken (after content dedupe)

| Source | tier | Units | Notes |
|---|---|---:|---|
| Solana Audit Arena (`Frankcastleauditor/Solana-Audit-Arena`) | J | 11 | issues 141, 144, 147, 163, 187, 201, 203, 210, 270, 280, 281 |
| Solana Audit Arena | X | 60 | unlabelled issues, harness block present, not in v2, first 60 unique by content |
| Solana Audit Arena (target test suites) | N | 10 | secure-path tests of the five Arena programs |
| CodeHawks RustFund (`CodeHawks-Contests/2025-03-rustfund`) | J | 47 | judged submissions with a harness block; 54 before content dedupe |
| CodeHawks RustFund (own suite) | N | 1 | `tests/rustfund.ts` |
| Code4rena Jupiter Lend | J | 1 | L-07 (Low), the only executable test in the report |
| Zolldyk `anchor-pinochio-security-template` | N | 5 | secure-path comparison/edge tests; `exploit-demo.ts` files are v2 X and not reused |
| **Total counted** | | **135** | J 59, X 60, N 16 |

Split (all counted units): DEV 51, V1 28, V2 29, FINAL 27.
J by split: DEV 18, V1 14, V2 16, FINAL 11.
J share by source: RustFund 79.66%, Arena 18.64%, Code4rena 1.69%.
J share by platform: CodeHawks 79.66%, Arena 18.64%, Code4rena 1.69%.

## Cap and FINAL arithmetic (the blocker)

- §10 needs the FINAL slice to hold >= 30 J units from >= 3 sources with no
  source > 40%. This corpus has 59 J, 11 of them in FINAL. The committed
  enlargement rule may move DEV units only, and DEV holds 18 J: 11 + 18 = **29
  FINAL J at most**, one below the minimum.
- Even after moving all 18, FINAL would be RustFund 23, Arena 6 (no Code4rena
  unit lands in FINAL): **79% one source**, against the 40% rule.
- A strict 40%-per-origin cap over the whole J universe (origins: Arena 11,
  RustFund 47, Code4rena 1) admits at most 2 + 2 + 1 = **5 J units**, because
  three sources at 40% each cannot absorb a third source of size 1 (T <= 5).
- Therefore the target "J >= 60, >= 4 sources, no source > 40%" is **not
  reachable** from the public fresh data found. The corpus keeps every unit and
  reports the shares rather than trimming to a cap-compliant subset that would
  carry almost no data.

## Excluded candidates that looked promising

| Candidate | Looked like | Why rejected |
|---|---|---|
| Cantina Tensor (234 submissions, $150k) | largest Solana contest | no individual findings/PoCs public |
| CodeHawks SSSwap results | second Solana CodeHawks contest | 10 valid groups, no submission with test code |
| Code4rena Jupiter Lend M-01/M-02 and L-01..L-12 | 14 more judged items | source/fix snippets only; no test code |
| Code4rena Solana Foundation 2025-08 (8 Low/QA items) | judged, public | no test code in any item |
| Jito Restaking findings 36675, 36787 | judged | no code / non-exploit test (v2 screen) |
| Immunefi Raydium reviews | Critical bounty outcome | no runnable PoC |
| Hats Dappslap | Solana/Anchor contest | no public findings repo |
| Solodit | would index Cantina/CodeHawks | Vercel checkpoint, unreadable |
| CodeHawks live submission pages | per-submission PoC + verdict | Vercel checkpoint; only the archived results payload is readable |
| Arena labelled issues without a harness block (15) | judged findings | prose or source quotes only |
| Arena issue 278 | v2 candidate | compute-unit benchmark, not an exploit (v2 screen) |
| Anchor GitHub advisories | published severity | PoC is deployable program source, not test code |
| Firedancer v0.1 (18 findings) | judged | C validator, no Solana-program frontend |

## What "nothing more exists" does and does not mean

It is **not** claimed. The classes above were each searched to the point where
the public supply is exhausted or unreadable, and the log records the queries.
What remains unread: Cantina/Solodit per-finding pages (Vercel checkpoint),
CodeHawks per-submission pages (same), and logged-in-only archives. A later run
with a browser session could add per-submission CodeHawks PoCs that the
aggregated results payload does not include (the payload keeps only submitted
descriptions, which is what was mined).
