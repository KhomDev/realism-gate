# Search log — Solana PoC corpus v2 (P1-C)

Collected 2026-11-25. Every query below was run on that date. GitHub API calls
went through `gh api` with a 10 req/min budget for code search and >=7 s
spacing; web searches used the built-in search tool with US results.

Units taken are counted after inventory dedupe and screening.

## Source-class coverage

| Class the prompt required | What was done | Result |
|---|---|---|
| Code4rena and Sherlock Solana contests/judging (2024-2026) | Listed the `code-423n4` org (881 repos) and the `sherlock-audit` org (462 repos); read the Code4rena reports index; opened the two Code4rena Solana-related findings repos | Code4rena Solana contests are Pump Science (P1), Solana Foundation 2025-08 (no H/M, no coded PoC), Anchor 2022 (76 issues, no tests), Jupiter Lend 2026-02 (scope only). Sherlock has only Orderly and WOOFi, both exhausted in P1. **0 units.** |
| Cantina competitions publicly readable | Read the Tensor competition page; tried `/findings` | Findings and PoCs are not published to anonymous readers (404). **0 units.** |
| Immunefi bug-fix reviews and public Solana bounty PoCs | Read the official `immunefi-team/bugfix-reviews-pocs` repo; read the Immunefi Audit Competition report index and every Jito Restaking finding; searched for Raydium/Jito write-ups | The official PoC repo is Foundry/EVM only. The Jito Restaking competition publishes full reports with test blocks: **3 units**. Firedancer is a C validator client: 18 findings screened out. Raydium bug-fix posts have no runnable PoC. |
| Security-firm blogs with linked PoC repos (OtterSec, Neodyme, Zellic, Sec3, Ackee, Halborn, Accretion) | Searched each name with "Solana proof of concept repository"; read the OtterSec org and the poc-framework docs; read the Accretion verified-builds post | Zellic/Sec3/Accretion publish PDFs and post-mortems, not runnable PoCs. OtterSec/Neodyme runnable PoCs are P1 sources or the framework itself. Halborn CTF solutions included: **2 units**. |
| Solana CTF solve repositories (2023-2026) | Searched GitHub for CTF solve collections; cloned and read `lkmidas/Web3-CTF-Collection`, `paradigmxyz/paradigm-ctf-2022`, `De1ik/Solana-Warrior-Path`, `0xmoostorm/Halborn-Solana-CTF-Solutions`; checked the XCTF Lilac and TCP1P write-up links | lkmidas/paradigm/De1ik solves are unsolved Anchor templates ("Add your test here"); the XCTF link (`n-WN/SubgroupX`) is a web app, not a solve repo. Halborn's two solves are included. **2 units.** |
| Auditors' public portfolios with stated outcomes | Read `web3-master/audit-reports` (P1, exhausted), `vaultvult/web3-security-research`, `TheGrandPew/jet-governance-pocs`, `princedotrs/solana-audit`, `Tejanadh/1inch-solana-rescue-theft-research`, `publicqi/loopscale-hack` | vaultvult's PoCs are print-only simulations; princedotrs is a template/tool repo. Jet Governance PoCs (5) and the 1inch/Loopscale reproductions (2) are X units because no per-finding outcome is published. |
| Curated lists (`awesome-solana-security`, `blockthreat/blocksec-ctfs`) | Read both lists through web search results and their fork indexes | They point to Neodyme/Ackee/Paradigm material already covered or already P1 sources. **0 new units.** |
| Not in the prompt but found | Solana Audit Arena (weekly judged competition with public labelled issues and final reports) | Judged source: **25 J units**, plus **19 X units** from unjudged weeks. |

## Queries and reads

### Web searches

1. `Cantina competition Solana findings proof of concept code` — Tensor competition
   found; findings not public (dead end).
2. `github solana poc-framework exploit test Neodyme` — `neodyme-labs/solana-poc-framework`
   (framework, not PoCs); workshop already a P1 source.
3. `Immunefi Solana bug fix review proof of concept code` — found
   `immunefi-team/bugfix-reviews-pocs` (EVM only); Raydium reviews have no code.
4. `awesome-solana-security github CTF exploit repositories` — lists and
   `MerovingioOps/solana-hacks` (included), `sannykim/solsec` (list).
5. `Solana CTF solve github anchor exploit test 2025` — XCTF Lilac write-up,
   TCP1P write-ups, `Don-Uwe/solana-security-pattern` (404 later),
   `Zolldyk/anchor-pinochio-security-template` (included), `k--chow/solana-security`.
6. `OtterSec blog Solana vulnerability proof of concept github repository` —
   OtterSec org and poc-framework; audit PDFs, no public PoC repos.
7. `CodeHawks Solana contest findings proof of concept public` — no Solana
   contest in the archive; EVM findings only.
8. `Hats Finance Solana audit competition findings public` — reports go to the
   sponsor; Dappslap report not found.
9. `"solana" "security advisory" github advisory proof of concept anchor program` —
   two Anchor advisories; their PoCs are program source to deploy, not tests.
10. `site:immunefi.com Solana bugfix review` — Raydium (no code), Immunefi
    Audit Competition report site.
11. `Hats Finance Dappslap Solana audit competition report findings` — no result.
12. `Solana bug bounty writeup paid critical vulnerability "proof of concept" github test` —
    `vaultvult` (print-only), Superteam threads, no runnable paid PoC.
13. `reports.immunefi.com Solana audit competition` — Jito Restaking and
    Firedancer competitions.
14. `"Solana Audit Arena" findings public github` — the Arena repository and its
    weekly write-ups (judged source).
15. `Zellic OR Sec3 OR Accretion Solana vulnerability writeup proof of concept repository` —
    audit PDFs and the Accretion verified-builds post; no PoC repos.
16. `blockthreat blocksec-ctfs solana challenge solutions repository` — curated
    list; points to Neodyme/N1CTF material.
17. `Ackee Blockchain Solana CTF solutions github exploit` — bootcamp/workshop
    already P1 sources; `solana-common-attack-vectors` catalogues patterns, not PoCs.

### GitHub repository and code searches

| Query | Total hits | Repos | Outcome |
|---|---:|---:|---|
| `org:code-423n4 solana in:name` | 1 | 1 | `2025-08-solana-foundation`; no H/M, no coded PoC |
| `org:sherlock-audit solana in:name` | 4 | 4 | Orderly + WOOFi only, both P1 sources |
| Org listings, `code-423n4` (881 repos) and `sherlock-audit` (462 repos) | — | — | Grepped for Solana-ecosystem names; four Code4rena Solana repos and two Sherlock ones as above |
| `LiteSVM attacker exploit language:Rust` | 77 | 34 | P1's screen; remaining hits are P1-screened repos plus Arena |
| `ProgramTest attacker exploit language:Rust` | 20 | 14 | Same; no new solve |
| `bankrun attacker exploit language:TypeScript` | 26 | 13 | Same; no new solve |
| `anchor.workspace attacker exploit language:TypeScript` | 58 | 37 | Same; Arena appears here (included) |
| `Mollusk exploit language:Rust` | 89 | 63 | `francis-codex/solana-security-patterns` (included), `psyto/solinv` (tool) |
| `litesvm attacker language:Rust` | 788 | 55 | `publicqi/loopscale-hack` (included), `corey888773/solana-amm-attacks-analysis` (included), Arena |
| `program_test steal language:Rust` | 41 | 32 | `Tejanadh/1inch-solana-rescue-theft-research` (included), otherwise forks/suites |
| `poc_framework language:Rust` | 109 | 35 | `TheGrandPew/jet-governance-pocs` (included), CTF archives, Neodyme copies |
| `"proof of concept" anchor test` | 44,565 | 96 | Unscoped phrase query; noise apart from the Arena |
| GitHub advisories, `ecosystem=rust&affects=` `anchor-lang`, `solana-program`, `spl-token`, `spl-associated-token-account`, `solana-sdk`, `solana_rbpf` | 2 + 3 | — | Anchor advisories (source-only PoCs), three 2022 rbpf advisories (no test) |

### Pages read directly

- `code4rena.com/reports` (embedded report index) and the Pump Science report
  (`H-01`, `M-01`, `M-02` code blocks are source quotes, not tests).
- `reports.immunefi.com/jito-restaking` and all five finding pages.
- `reports.immunefi.com/firedancer-v0.1` (C validator, screened out).
- `cantina.xyz` Tensor competition page and `/findings` (404).
- `immunefi-team/bugfix-reviews-pocs` (EVM only).
- `Frankcastleauditor/Solana-Audit-Arena` repository and its 346 issues.
- `code-423n4/2022-02-anchor-findings` (76 issues, no tests).

## Units taken

| Source | Units |
|---|---:|
| Solana Audit Arena (25 judged + 19 unjudged; issue 278 dropped) | 44 |
| Jito Restaking (Immunefi Audit Competition) | 3 |
| MerovingioOps/solana-hacks | 10 |
| TheGrandPew/jet-governance-pocs | 5 |
| francis-codex/solana-security-patterns | 6 |
| Zolldyk/anchor-pinochio-security-template | 6 |
| 0xmoostorm/Halborn-Solana-CTF-Solutions | 2 |
| publicqi/loopscale-hack | 1 |
| Tejanadh/1inch-solana-rescue-theft-research | 1 |
| supernewbiecoder/DEMO_solve_escrow | 1 |
| corey888773/solana-amm-attacks-analysis | 2 |
| **Counted total** | **81** |

## Dead ends, stated plainly

- **Judged tier.** The 15-judged target is met (28) only by adding the Solana
  Audit Arena and the Jito Restaking competition. Neither was known before this
  search. Without them the fresh judged tier would have been 0: every judged
  Solana source P1 used is exhausted, and the remaining public platforms either
  do not publish findings (Cantina, Hats, CodeHawks) or have no Solana contest
  with coded PoCs (Code4rena, Sherlock).
- **X tier.** The CTF repositories that looked richest (lkmidas, paradigm-ctf,
  De1ik) are solved only in the challenge author's private repo; the public
  copies are unsolved Anchor templates. The 60-unit target is met with 53 X
  units only because the Arena's unjudged weeks supply 19 real practitioner
  PoCs.
- **Nothing more exists is not claimed.** The huge archives
  (`sajjadium/ctf-archives`, `VAD37/security-research-archive`,
  `scriptCTF/scriptCTF2026-OfficialWriteups`) were not exhaustively read; the
  reason is recorded in `inventory.json` under `code_search_screen`. A later
  search can add to them.
