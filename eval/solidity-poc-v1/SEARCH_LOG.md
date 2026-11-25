# Search log — Solidity PoC corpus v1

Collected 2026-11-25 by the P2R researcher (Workstream R, Phase 2). `gh api`
calls were spaced >= 7 s; the bulk of the collection is `git clone` of the
findings repositories, which does not consume the API budget. All sources are
unlicensed for our purposes (contest findings, user submissions, repos with no
licence file), so the repository stores pointers and sha256 hashes only; the
text lives in the gitignored `.corpus/solidity-poc-v1/`.

## Result, uncomfortable part first

J meets the target at **source** level and fails it at **platform** level.
477 distinct judged findings come from 112 Code4rena contests (target 120 from
6+ sources, max source 4.4%). Every one of them is Code4rena, so platform share
is 100%: if "independent source" means platform, the corpus has one, not six.
**X misses badly: 36 of 150.** DeFiHackLabs and DeFiVulnLabs are opened data in
this workspace; only 9 new DeFiHackLabs incident files exist after the pinned
`later` commit (`d4bd931`), DeFiVulnLabs has 0 new files, Immunefi publishes 9
Foundry PoCs, and the remaining reachable fresh material is 18 distinct CTF
teaching exploits. N is 30 of 30.

## What was searched

| Class | What was done | Result |
|---|---|---|
| Code4rena findings repos | Listed all 881 `code-423n4` repos; 376 are `*-findings`. Cloned 210 (all 2023+ plus a 48-contest sample of 2021-2022). Parsed every `report.md`. | 2,457 High/Medium report sections; **477 contain a Foundry test** and became J units. |
| Code4rena codebases | `git ls-remote` for all 209 contributing contests; 203 resolve to a commit. The `code-423n4/<slug>` snapshot is the codebase pin. | 209 codebases recorded; 466/477 J units pinned, 11 null. |
| Sherlock judging repos | Cloned `2024-11-hats-protocol-judging` (58 submissions, final report in `README.md`, metadata in `comments.csv`). Only 2 of 58 submission bodies carry test code. | Screened out of J this run (yield too low); no Invalid-with-test N found in the sample. |
| Cantina | Findings remain private to anonymous readers (Phase-1 check); competitions index has no Solidity finding pages. | 0. |
| CodeHawks (EVM) | Results payloads exist only via Wayback (live site Vercel-checkpointed); the Phase-1 parser applies, but the run budget went to the larger C4 supply. | Not mined this run. |
| Hats Finance | Competition repos carry issue bodies and judge labels; not reached this run. | Not reached. |
| Immunefi audit competitions / bug-fix PoCs | `immunefi-team/bugfix-reviews-pocs` cloned: 9 real Foundry PoCs (Alchemix, Astar, Balancer, BeanStalk, DFX, MEV, Silo, Wormhole, Yield). | **9 X units.** |
| DeFiHackLabs (X) | Compared `d4bd931` (the opened `later` corpus) with HEAD `8a2b37f`: 9 added `.sol` files (2026-10/11), 4 modified files (opened; not reused). | **9 X units.** |
| DeFiVulnLabs (X) | HEAD equals the opened `fresh` commit `f61f6ee`; 0 new files. | 0 fresh. |
| CTF solve repos (X) | GitHub search for Foundry solutions; cloned `faheelsattar/damn-vuln-defi-foundry` (14 solution tests) and `catcatcat8/mixbytes-ctf` (4 attack tests). Distinct challenges only; other solve repos repeat the same DVDefi levels and were not counted. | **18 X units** (teaching exploits, `real_incident: false`). |
| Secure-path tests (N) | The audited contest codebases carry their own Foundry suites. Cloned `code-423n4/2024-08-wildcat` (25 `.t.sol`), fetched 5 `code-423n4/2024-01-salty` test files raw (the full salty clone fails on Windows long paths in `discord-export/`). | **30 N units.** |

## Queries and reads

- `gh api orgs/code-423n4/repos` pages 1-9 (881 repos, Phase 1, reused here).
- `git clone --depth 1 https://github.com/code-423n4/<slug>-findings.git` for 210 slugs.
- `git ls-remote https://github.com/code-423n4/<slug>.git HEAD` for 209 slugs.
- `gh api repos/code-423n4/2024-07-dittoeth/...` tree attempt (404 on a malformed local pin; not needed in the end).
- `gh api repos/SunWeb3Sec/DeFiHackLabs/compare/d4bd931...HEAD` (13 changed files, 9 added).
- `gh api repos/SunWeb3Sec/DeFiVulnLabs/compare/f61f6ee...HEAD` (0 files).
- `search/repositories?q=damn-vulnerable-defi+solutions+foundry` and `...smart+contract+ctf+solutions+foundry` (leads above).
- `gh api repos/theredguild/damn-vulnerable-defi/contents` (challenge repo, no official solutions; solve repos used instead).

## Units taken

| Tier | Source | Units | Notes |
|---|---|---|---|
| J | Code4rena, 112 contests | 477 | one report section = one finding group; max contest 21 (2024-07-loopfi, 4.4%); 466 codebase-pinned |
| X | DeFiHackLabs (new files) | 9 | real incidents, 2026-10/11 |
| X | Immunefi bug-fix PoC repo | 9 | real incidents |
| X | DVDefi / MixBytes CTF solutions | 18 | teaching exploits, `real_incident: false` |
| N | `code-423n4/2024-08-wildcat` tests | 25 | secure-path tests of the audited code |
| N | `code-423n4/2024-01-salty` tests | 5 | raw-fetched at the pinned commit |
| **Total** | | **543** | J 477, X 36, N 30 |

Split: DEV 218, V1 103, V2 86, FINAL 136.
J source share: max 4.4%; 112 sources. J platform share: Code4rena 100%.
Codebase pin coverage: 466/477 J units (97.7%); 11 null.

## Cap and target arithmetic

- J target at source level: met (477, 112 sources, max 4.4% <= 30%).
- J at platform level: **not met** — one platform holds 100% of J. The unit
  rule and the target language do not define "source" beyond the per-contest
  unit; both readings are reported so the orchestrator can decide.
- X target 150: **missed at 36**. Fresh incident material is the binding
  constraint, not time: the two large incident corpora in this workspace are
  opened (DeFiHackLabs 944 files, DeFiVulnLabs), and their post-pin growth is
  9 + 0 files. CTF solutions are plentiful but only ~18 distinct challenges are
  reachable without counting the same challenge solved by many people.
- N target 30: met exactly.

## Excluded candidates that looked promising

| Candidate | Why rejected |
|---|---|
| 1,980 C4 H/M findings with no Foundry test in the report | source/fix snippets only; a report section without test code is not a coded PoC |
| Sherlock `2024-11-hats-protocol` (58 submissions) | only 2 submission bodies carry test code; too thin to represent the class |
| CodeHawks EVM results | not mined this run (Wayback parser available; budget went to C4) |
| Hats Finance competition repos | not reached this run |
| Cantina competitions | findings not public |
| DeFiHackLabs modified files (Opyn, README, scripts) | opened data; modification is drift, not a fresh unit |
| DeFiVulnLabs HEAD | equals the opened `fresh` commit |
| Multiple DVDefi solve repos | repeat the same challenge set; counted once by challenge |
| Salty sponsor repo full clone | Windows long-path failure in `discord-export/`; only 5 test files were needed, fetched raw at the commit |
| 6 contests whose sponsor repo is gone (`MISSING`) | units kept with `codebase_id: null` |

## Verification

`fetch.py --check` verifies all 543 units offline; the default online run
re-read every findings checkout at its pinned commit, every report-section PoC
block hash, every incident file hash and the five raw salty files, and printed
`all 543 unit pins verified`. `build_inventory.py --check` matches
`inventory.json`. A random 10% pin re-fetch is left to the orchestrator.
