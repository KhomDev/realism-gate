# Search log — solidity-poc-v2

P2R-2 §3.2 researcher, 2026-11-26. Scaffold and salt committed first (`60698e0`), before any
unit was collected. `gh api` calls were spaced >= 7 s; bulk collection is `git clone`, which
does not use the API budget. Sources are unlicensed for our purposes unless stated, so the
repository stores pointers and sha256 hashes only; text lives in the gitignored
`.corpus/solidity-poc-v2/`. No realism-gate run and no label was made on any unit.

## Result, uncomfortable part first

- **Platform diversity: met.** Code4rena is 56.32% of J across v1+v2 (477 of 847), under the
  amended 60% cap. Sherlock supplies 322 judged report sections with Foundry test code
  (38.02%); five bounty write-ups add 0.59%.
- **X: missed at 78 of 150.** DeFiHackLabs has **0** fresh incident files: HEAD `8a2b37f`
  equals the v1 pin (`compare 8a2b37f...HEAD`: 0 commits) and all 953 `src/test` files are in
  the opened corpus (944 at `d4bd931` + the 9 v1 X units). The only fresh incident source found
  is `coinspect/learn-evm-attacks` (MIT, 42 `*.attack.sol` reproductions). v1 36 + v2 42 = 78.
- **The operator-supplied finder list yields 5 J units from 148 rows** (4 whitehat clusters).
  Most rows are tweets (22), unreachable pages (25; the Trust Security blog moved and the
  Internet Archive was offline all session), or pages without a complete coded PoC.

## Sources

| Class | What was done | Result |
|---|---|---|
| Sherlock `*-judging` | `gh api orgs/sherlock-audit/repos` pages 1-5 (460 repos, 230 judging). Cloned all 230 at depth 1 (`2023-06-unstoppable-judging` failed: Windows path). Parsed each final report `README.md` (`# Issue H-n/M-n`), issue body before `## Discussion` only, v1 `select_sol_block`. | 213 repos carry a final report; 2,666 H/M sections; **322 with Foundry test code** in 97 contests (2022: 7, 2023: 59, 2024: 206, 2025: 50). 314 codebase-pinned via `git ls-remote sherlock-audit/<contest>`; 3 contest repos missing (2022-09-harpie, 2024-10-debita, 2024-12-babylon). Max contest 4.89% of v2 J. |
| Sherlock group directories / `invalid/` | Inspected (`2024-06-mellow-judging`): group directories are pre-escalation and do not match the final report's grouping (8 dirs vs 4 report issues). | Not used; only final-report sections count. Invalid-with-test submissions not collected (Solidity N not in scope of this brief). |
| Cantina competitions | Mirror index `index/findings/cantina` (9,044 findings; 1,150 in 71 competition reports). | Not pursued for Solidity in the time box; the Solana subset was screened (see solana-poc-v4 log). |
| CodeHawks EVM via Wayback | Wayback CDX for `codehawks.cyfrin.io/c/*` (3,650 URL keys, collapse=urlkey) was read once; later CDX calls returned "Temporarily Offline". | No new EVM contest pinned this session (v1 already holds the four reachable results payloads). |
| DeFiHackLabs (X) | `gh api repos/SunWeb3Sec/DeFiHackLabs/compare/8a2b37f...HEAD`: 0 commits. Tree at `8a2b37f`: 957 `.sol`, 953 under `src/test`. | **0 fresh** (all opened or v1). |
| Other incident repos (X) | `gh search repos` (5 queries: "hack reproduction foundry poc", "exploit poc foundry incidents", "defi hacks foundry reproduce", "defi-hacks foundry", "exploit-poc solidity", "hacks-poc"): no relevant hits. `coinspect/learn-evm-attacks` known lead cloned @ `77342c4`. | **42 X** (MIT; pointer + hash kept). Same-incident overlap with opened DeFiHackLabs is possible (different code) and is noted per unit. |
| Immunefi finder list | `sayan011/Immunefi-bug-bounty-writeups-list` @ `b8277a4` (no licence; pointer only). 148 table rows parsed; every non-tweet link fetched once with curl (1 s apart) into the cache; GitHub-hosted reports fetched raw; linked PoC repos/gists pinned by commit/version. Outcome taken only from the original page. | **5 J** (per-row verdicts below). |

## Finder-list row verdicts

Fetch column: HTTP status of the original write-up link on 2026-11-26 ("not fetched" = tweet).
| row | protocol | link host | fetch | verdict |
|---|---|---|---|---|
| 1 | Wormhole | medium.com | 200 | rejected: PoC repo immunefi-team/wormhole-uninitialized is an Immunefi-team reproduction of an incident already in v1 X (bugfix-reviews-pocs Wormhole); not the whitehat's judged PoC |
| 2 | Aurora | medium.com | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 3 | Optimism | medium.com | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 4 | Moonbeam | pwning.mirror.xyz | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 5 | Polkadot Frontier EVM | pwning.mirror.xyz | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 6 | Interlay | pwning.mirror.xyz | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 7 | Sherlock Yield Strategy | mirror.xyz | 200 | TAKEN J-bounty-sherlock-yield-gothicshanon: linked Foundry Attack.sol @ f5fee94; page states $250k paid |
| 8 | Belt | medium.com | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 9 | Fei | medium.com | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 10 | Arbitrum | medium.com | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 11 | Balancer | mirror.xyz | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 12 | LEVEL Finance | twitter.com | not fetched | rejected: write-up is a tweet (login wall); no code page |
| 13 | BeanStalk | medium.com | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 14 | Sense | medium.com | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 15 | Fluidity | www.trust-security.xyz | 404 | rejected: original page unreachable (404) |
| 16 | Oasis | www.trust-security.xyz | 404 | rejected: original page unreachable (404) |
| 17 | Fringe.fi | www.trust-security.xyz | 404 | rejected: original page unreachable (404) |
| 18 | O3 | www.trust-security.xyz | 404 | rejected: original page unreachable (404) |
| 19 | Morpho | www.trust-security.xyz | 404 | rejected: original page unreachable (404) |
| 20 | Compound | www.trust-security.xyz | 404 | rejected: original page unreachable (404) |
| 21 | ANKR/Stader | www.trust-security.xyz | 404 | rejected: original page unreachable (404) |
| 22 | Iron Bank | www.trust-security.xyz | 404 | rejected: original page unreachable (404) |
| 23 | Iron Bank | www.trust-security.xyz | 404 | rejected: original page unreachable (404) |
| 24 | ANKR | www.trust-security.xyz | 404 | rejected: original page unreachable (404) |
| 25 | Brahma | www.trust-security.xyz | 404 | rejected: original page unreachable (404) |
| 26 | Brahma | www.trust-security.xyz | 404 | rejected: original page unreachable (404) |
| 27 | Brahma | www.trust-security.xyz | 404 | rejected: original page unreachable (404) |
| 28 |  | (no link) | not fetched | rejected: row has no write-up link |
| 29 | Oasis | www.trust-security.xyz | 404 | rejected: original page unreachable (404) |
| 30 | Tokemak | www.trust-security.xyz | 404 | rejected: original page unreachable (404) |
| 31 | Thena | zzykxx.com | not fetched | rejected: write-up is a tweet (login wall); no code page |
| 32 | Alchemist | dacian.me | 200 | TAKEN J-bounty-alchemist-dacian: Foundry test block on the page; headline states $28K bounty |
| 33 | Warden Swap | github.com | 200 | rejected: Foundry fragment present but the page states only the report's own severity, no outcome |
| 34 | Hourglass (old) | github.com | 200 | rejected: outcome stated (acknowledged, not paid) but no Foundry block passes the content selector |
| 35 | Thena | zzykxx.com | not fetched | rejected: write-up is a tweet (login wall); no code page |
| 36 | Angle | medium.com | 200 | TAKEN J-bounty-angle-deliriusz-vault and -erc777: linked Foundry tests @ 14f522a; page states both submissions were rejected |
| 37 | Tranchess | www.kalos.xyz | 404 | rejected: original page unreachable (404) |
| 38 | Hyperlane | github.com | 200 | rejected: no Foundry block passes the content selector (page says duplicate, not paid) |
| 39 | Ocean | mirror.xyz | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 40 | Betverse | mirror.xyz | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 41 | Cronos | medium.com | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 42 | Port | medium.com | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 43 | 88mph | medium.com | 200 | rejected: linked Immunefi gist is a helper contract (DummyMPHMinter.sol), not a test |
| 44 | Ondo | iosiro.com | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 45 | Alchemix | medium.com | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 46 | pxMythics | ashiq.co.za | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 47 | abwagmi | ashiq.co.za | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 48 | Polygon | ashiq.co.za | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 49 | OpenZeppelin | ashiq.co.za | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 50 | Charged Particles | medium.com | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 51 | Mt Pelerin | medium.com | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 52 | Synthetix | medium.com | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 53 | Redacted Cartel | medium.com | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 54 | APWine | medium.com | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 55 | Enzyme | medium.com | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 56 | Notional | medium.com | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 57 | Bitswift | medium.com | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 58 | Polygon | medium.com | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 59 | dHEDGE | mirror.xyz | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 60 | Multichain (Previously Anyswap) | medium.com | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 61 | Mushrooms | medium.com | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 62 | Zapper | medium.com | 403 | rejected: original page unreachable (403) |
| 63 | Tidal | medium.com | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 64 | xDai | medium.com | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 65 | IPOR | twitter.com | not fetched | rejected: write-up is a tweet (login wall); no code page |
| 66 | Enzyme | medium.com | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 67 | Polygon zkEVM | twitter.com | not fetched | rejected: write-up is a tweet (login wall); no code page |
| 68 | Lybra Finance | medium.com | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 69 | Spartan | github.com | 404 | rejected: original page 404 |
| 70 | DFX Finance | medium.com | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 71 | Perennial | mirror.xyz | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 72 | Silo | twitter.com | not fetched | rejected: write-up is a tweet (login wall); no code page |
| 73 | Q Blockchain | medium.com | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 74 | Astroport | defihacklabs.substack.com | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 75 | BendDAO | medium.com | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 76 | OpenZeppelin | twitter.com | not fetched | rejected: write-up is a tweet (login wall); no code page |
| 77 | Eco | mirror.xyz | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 78 | Bifrost Finance | medium.com | 200 | rejected: test-marker text only in prose/snippets; no complete test passes the selector |
| 79 | - | medium.com | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 80 | - | medium.com | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 81 | Yield Protocol | medium.com | 200 | rejected: Immunefi bug-fix review; its PoC is the Yield unit already in v1 X (bugfix-reviews-pocs) |
| 82 | O3 Swap | medium.com | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 83 | - | mirror.xyz | 404 | rejected: original page unreachable (404) |
| 84 | DFX Finance | www.beirao.xyz | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 85 | Beluga Protocol | github.com | 200 | rejected: gist PoC exists but the page states no payout/acceptance (program removed from Immunefi) |
| 86 | GYSR | github.com | 200 | rejected: gist PoC exists but no program outcome is stated (report header severity only) |
| 87 | Optimism | www.iosiro.com | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 88 | Threshold Network | blog.threshold.network | 404 | rejected: original page unreachable (404) |
| 89 | Balancer | medium.com | 200 | rejected: Immunefi bug-fix review; its PoC is the Balancer unit already in v1 X (bugfix-reviews-pocs) |
| 90 | RAI | mirror.xyz | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 91 | Lybra Finance | twitter.com | not fetched | rejected: write-up is a tweet (login wall); no code page |
| 92 | Perpetual Protocol | securitybandit.com | 000 | rejected: original page unreachable (000) |
| 93 | Ocean Protocol | mirror.xyz | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 94 | Ocean Protocol | mirror.xyz | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 95 | Talent Protocol | mirror.xyz | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 96 | zkSync Era | medium.com | 200 | rejected: linked PoC (chainlight zksync-era) is Rust circuit code, not a Foundry/Hardhat test |
| 97 | Tranchess | github.com | 200 | TAKEN J-bounty-tranchess-floranguyen0: Foundry test @ a270357; README states $200,000 bounty paid |
| 98 | Retro+Thena+`unknown protocol` | github.com | 503 | rejected: raw markdown has no Foundry block passing the selector |
| 99 | Astar | www.zellic.io | 200 | rejected: Zellic Astar write-up; Astar incident already in v1 X; page blocks are not a Foundry test |
| 100 | Cronos Gravity Bridge | faith2dxy.xyz | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 101 | Nomad | nikitastupin.com | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 102 | Beanstalk | medium.com | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 103 | 100+ projects | www.trust-security.xyz | 404 | rejected: original page unreachable (404) |
| 104 | Oasys | mirror.xyz | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 105 | 15+ projects | mirror.xyz | 200 | rejected: one generic signature-replay PoC across 15+ programs; no per-finding judged outcome |
| 106 | zkSync Lite | medium.com | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 107 | Polygon PoS | www.asymmetric.re | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 108 | Stacks | medium.com | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 109 | Stargate | www.trust-security.xyz | 404 | rejected: original page unreachable (404) |
| 110 | LayerZero | www.trust-security.xyz | 404 | rejected: original page unreachable (404) |
| 111 | Deri | mirror.xyz | 200 | rejected: PoC is a Hardhat JS script; the Solidity selector (v1 select_sol_block) admits Solidity only |
| 112 | Polygon zkEVM | blog.verichains.io | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 113 | Cronos | gist.github.com | 200 | rejected: gist has a Foundry test but states no outcome (bounty only in the finder list) |
| 114 | The Graph | medium.com | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 115 | Sei Network | usmannkhan.com | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 116 | Sovryn | x.com | not fetched | rejected: write-up is a tweet (login wall); no code page |
| 117 | Raydium | medium.com | 200 | rejected: Raydium (Solana) review already screened in v3 (no runnable test) |
| 118 | Sei Network | exvul.com | 404 | rejected: original page unreachable (404) |
| 119 | - | github.com | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 120 | Evmos | medium.com | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 121 | Wormhole | x.com | not fetched | rejected: write-up is a tweet (login wall); no code page |
| 122 | Balancer V2 | mirror.xyz | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 123 | Axelar Network | marcotnunes.com | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 124 | Vesu | x.com | not fetched | rejected: write-up is a tweet (login wall); no code page |
| 125 | Acala | immunefi.com | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 126 | Scroll | x.com | not fetched | rejected: write-up is a tweet (login wall); no code page |
| 127 | Scroll | forum.scroll.io | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 128 | Vesu | docs.vesu.xyz | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 129 | Across V3 | mirror.xyz | 200 | rejected: the Solidity test block also matches the selector's TS markers, so the content selector rejects it |
| 130 | VeChainThor | immunefi.com | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 131 | Fraxlend | mirror.xyz | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 132 | Story | www.story.foundation | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 133 | Story | www.story.foundation | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 134 | Movement Labs | medium.com | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 135 | Lido | research.lido.fi | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 136 | Sui | immunefi.com | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 137 | marginfi | blog.asymmetric.re | 200 | rejected: no Foundry/Hardhat test on the page and no linked PoC file |
| 138 | RAI | www.trust-security.xyz | 404 | rejected: original page unreachable (404) |
| 139 | dHEDGE | x.com | not fetched | rejected: write-up is a tweet (login wall); no code page |
| 140 | zkSync Lite | x.com | not fetched | rejected: write-up is a tweet (login wall); no code page |
| 141 | Gnosis | x.com | not fetched | rejected: write-up is a tweet (login wall); no code page |
| 142 | Gnosis | x.com | not fetched | rejected: write-up is a tweet (login wall); no code page |
| 143 | Uniswap | x.com | not fetched | rejected: write-up is a tweet (login wall); no code page |
| 144 | Yearn | x.com | not fetched | rejected: write-up is a tweet (login wall); no code page |
| 145 | Reserve | x.com | not fetched | rejected: write-up is a tweet (login wall); no code page |
| 146 | Uniswap | x.com | not fetched | rejected: write-up is a tweet (login wall); no code page |
| 147 | Balancer | x.com | not fetched | rejected: write-up is a tweet (login wall); no code page |
| 148 | Injective | x.com | not fetched | rejected: write-up is a tweet (login wall); no code page |

## Units taken

| Tier | Source | Units | Split DEV/V1/V2/FINAL |
|---|---|---|---|
| J | Sherlock final-report sections, 97 contests | 322 | 138 / 54 / 48 / 82 |
| J | Bounty write-ups (dacian, GothicShanon89238, floranguyen0, deliriusz) | 5 | 1 / 1 / 3 / 0 |
| X | coinspect/learn-evm-attacks | 42 | 20 / 7 / 5 / 10 |
| **Total** | | **369** | 159 / 62 / 56 / 92 |

Dedupe: 0 content-hash or URL matches against `eval/corpus.json`, `eval/solana-poc*/`,
`eval/solidity-poc-v1/`, `eval/near-poc-v1/` (builder `opened_index`).

## Combined v1+v2 J platform shares

Code4rena 477 (56.32%), Sherlock 322 (38.02%), CodeHawks EVM 41 (4.84%), Immunefi 3 (0.35%),
Hats 2 (0.24%), Sherlock bounty 1 (0.12%), platform unstated 1 (0.12%); total 847.

## Dead ends and what remains

- Trust Security blog posts (15 rows) return 404; the Internet Archive was offline for every
  CDX attempt after the first, so no Wayback copy was tried. Retry when the archive is up.
- Medium write-ups embed code as images or gists in iframes; only `<pre>` blocks and linked
  files are readable without a browser.
- CodeHawks EVM results beyond v1's four: not re-attempted (archive offline).
- Cantina competition findings for Solidity: located via the mirror index but not collected.
- `2023-06-unstoppable-judging` clone failed on Windows path length.

## Verification

`python -I eval/solidity-poc-v2/build_inventory.py --check` → `inventory.json ok`;
`python -I eval/solidity-poc-v2/fetch.py --check` → `all 369 unit pins verified (offline)`.

## Correction 2026-11-26 (orchestrator verification of `26b076e`)

Defect: `report_sha256` hashed the Windows working-tree README, which `core.autocrlf=true`
had converted to CRLF, so 309 of 322 Sherlock pins were host-dependent (the 13 CRLF-upstream
reports matched by accident). Fix: the builder and `fetch.py` now read the git blob at the
pinned commit (`git cat-file blob <commit>:README.md`, no eol filters); the hash is of the blob
bytes, and the text is newline-normalised before parsing (as the former universal-newline
read did). Effect: 309 `report_sha256` values changed; no unit id, membership, selected block
index, `block_sha256` or split changed (`norm_hash` already strips `\r`). Proof: all 97
distinct report pins equal the sha256 of the raw upstream bytes
(`raw.githubusercontent.com/<repo>/<commit>/README.md`, 97/97), and fresh
`git -c core.autocrlf=false` clones of smilee-finance and gmx reproduce their pins.
learn-evm-attacks `sha256_norm` is now computed from the blob too (values unchanged).
Dacian page: page bytes are dynamic and are no longer a pin; `block_sha256` is the pin, and a
fresh fetch on 2026-11-26 reproduces it (block 3, `10ae36ec…`). Online `fetch.py` re-checks it.
