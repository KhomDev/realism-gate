# Search log — solidity-poc-v3

P2R-2 extension, 2026-11-27. Scaffold and salt committed first (`1941b91`),
before any unit was collected. This corpus is Solidity **X** only. Code4rena
Solidity J was not collected. No realism-gate run and no label was made.
Third-party text is pointer plus sha256 only. Hashes are sha256 of
`git cat-file blob` bytes.

Move was not collected. These rules do not define a Move corpus.

## Result

**35 counted X units**, all cluster `twabi-reentrancy-hacks`. Salted split:
DEV 11, V1 4, V2 6, FINAL 14.

v1 has 36 X and v2 has 42 X (78 combined, the published 78 of 150). These 35
are new normalised texts, so combined X is **113 of 150**. The remaining gap
is 37. No further public incident-reproduction corpus with a different text
was found in this pass.

## Taken

| Date | Source | Verdict |
|---|---|---|
| 2026-11-27 | `twabi/DeFi-Reentrancy-Hacks-Repository` @ `129b011` | **Taken, X, 35.** 47 `.sol` files. 11 have the same normalised text as a DeFiHackLabs blob and were screened out. 1 (`Parity_kill_exp.sol`) has no readme section and was screened out. The 35 that remain have a Foundry test marker and a quote from that year's `ReadMe.md` (the `Lost:` line, or the section heading when the loss line says "Not Specified"). Licence: none. |

The repository root `https://github.com/SunWeb3Sec/DeFiHackLabs` is already in
`eval/corpus.json`. Dedupe here is the file URL and the content hash, not that
root. A file whose path was absent from pin `d4bd931` can still be a duplicate
if its hash is already in a sealed inventory. That is what happened below.

## Screened, not counted

| Path or source | Reason |
|---|---|
| Nine `src/test/2026-10` and `src/test/2026-11` files added in DeFiHackLabs between `d4bd931` and `8a2b37f` | Same normalised hash as `eval/solidity-poc-v1` units `X-dhl-*`. Rows are kept with `opened_duplicate` and are not counted. |
| 11 twabi files (Cream, Grim, Rari, ValueDefi, Visor, Bacon, Omni, Rari, Revest, THB, uniswap-erc777) | Same normalised text as a DeFiHackLabs file at `d4bd931` or `8a2b37f`. |
| `src/2020-Older/test/Parity_kill_exp.sol` | No `ReadMe.md` section names the file, so there is no original quote. |
| `SidharthBhangalia/defi-attack-replay` `test/Reentrancy.t.sol` | README: a DAO-style lab with a contract the repo itself deploys. Not a named incident. |
| `pcaversaccio/reentrancy-attacks` @ HEAD | 0 `.sol` files. |

## Other queries

| Date | Query | Result |
|---|---|---|
| 2026-11-27 | `git diff` of DeFiHackLabs `src/test` at `d4bd931` versus `8a2b37f` | 9 added `.sol` files. All 9 already sealed in v1. |
| 2026-11-27 | Repository search `reentrancy-attacks foundry`, `defi hack exploit foundry` | The twabi repository (taken) and lab repositories (rejected). |
| 2026-11-27 | v1 and v2 X inventories | Already hold DeFiHackLabs-after-`d4bd931` (9), Immunefi bug-fix review tests (9), Damn Vulnerable DeFi Foundry (14), MixBytes CTF (4), and `coinspect/learn-evm-attacks` (42). Not reused. |

## Verification

`python -I eval/solidity-poc-v3/build_inventory.py --check` → inventory matches the cache.

`python -I eval/solidity-poc-v3/fetch.py --check` → all 44 rows' pins verified offline (35 counted, 9 marked already sealed).
