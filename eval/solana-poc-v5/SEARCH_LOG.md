# Search log — solana-poc-v5

P2R-2 extension, 2026-11-27. Scaffold and salt committed first (`1941b91`),
before any unit was collected. No realism-gate run and no label was made on
any unit. Third-party text is pointer plus sha256 only. Hashes are of
`git cat-file blob` bytes, or of the downloaded report bytes.

Move was not collected. The extension rules name Solana and Solidity only, and
there is no Move corpus to extend without opening a new measurement.

## Result

Counted units: **J 2, X 1, A 3**. J+X clusters: **2** (Meteora, Cashio).
Port Sundial is tier A and is not pooled. The target of ≥ 4 new J+X clusters
was not met. FINAL received 0 of these units under the committed salt.
FINAL was not enlarged.

| Tier | Cluster | Counted | Split DEV / V1 / V2 / FINAL |
|---|---|---:|---|
| J | code4rena-meteora-dbc | 2 | 1 / 1 / 0 / 0 |
| X | cashio-2022-infinite-mint | 1 | 1 / 0 / 0 / 0 |
| A | ottersec-port-sundial | 3 | 2 / 1 / 0 / 0 |

## Taken

| Date | Source | Verdict |
|---|---|---|
| 2026-11-27 | `https://code4rena.com/reports/2025-08-meteora-dynamic-bonding-curve` | **Taken, J, 2.** The contest repository is 404. The final report lists both findings under "Medium Risk Findings (2)". `select_block` (v3, content) accepts one block in each. Page bytes are pinned. Licence: none. |
| 2026-11-27 | `PwnedNoMore/cashio-exploit-workshop` @ `9392b12` branch `poc`, `poc/src/main.rs` | **Taken, X, 1.** Real Cashio incident (README: "which allowed the attacker to print infinite amount of $CASH"). The file's entry point is `async fn main`. v3 `select_block` does not treat that as a driver, so the pin is the whole git blob and the kind is `binary-main`. Licence: Apache-2.0; text not copied. |
| 2026-11-27 | `otter-sec/port-sundial-pocs` @ `289272c` and sponsor PDF `port-finance/sundial` @ `9395556` `audits/port-finance-sundial-audit-public.pdf` | **Taken, A, 3.** Three `*.spec.ts` files. `select_block` accepts each. The PDF lines quoted in the inventory say Critical/High and Resolved. Not a contest judgement, so not J. Licence: none. |

## Dead ends

| Date | Query or source | Verdict |
|---|---|---|
| 2026-11-27 | `code-423n4/2025-11-garden` and `code4rena.com/reports/2025-11-garden` | The one judged medium is an EVM `UDA.sol` finding. No `program.methods`, `it(`, or `#[test]` in the report. Not added (and it would have been Code4rena Solidity, which this round does not add). |
| 2026-11-27 | `code-423n4/2022-02-hubble-findings` `report.md` | 3 high and 17 medium headings. `select_block` accepted 0. The report text is EVM (Ownable, Vyper), not a Solana driver. |
| 2026-11-27 | Org listing `sherlock-audit` (460 repos) | Solana judging repos are only Orderly and WOOFi, already in `eval/solana-poc`. |
| 2026-11-27 | `reports.immunefi.com` index | The only Solana program competition linked is `/jito-restaking`, already in `eval/solana-poc-v2`. Firedancer is not a program-test corpus. |
| 2026-11-27 | `code-423n4` report-slug scrape of `code4rena.com/reports` | Solana-shaped slugs besides ones already sealed: Meteora (taken), Garden (rejected above), Jupiter Lend and Pump Science and Lavarage (already v1/v3/v4), Solana Foundation (v3 log: no coded PoC; not re-opened). `2025-09-layerzero-endpoint-v2-sui` is Move; skipped. |
| 2026-11-27 | `Quincy-Labs/solana-sec-bench` README | Synthetic cases (treasury, fee vault, loyalty). Public release withholds `tests/exploit/`. Not an incident and not judged. |
| 2026-11-27 | `kagen-0/solana-security-lab`, `vaultvult/web3-security-research` | Intentionally vulnerable lab, and a portfolio v2 already screened as print-only. |
| 2026-11-27 | `YadlaMani/solana-hacks` | A Next.js casino, not an exploit corpus. |
| 2026-11-27 | `otter-sec` repos `mango-v4`, `phoenix-v1`, `klend`, `cega-vault-report`, `battle-of-titans`, `synonym-wormhole-test` | Protocol trees or an empty repo or a report with no test file. `port-sundial-pocs` is the one PoC repo (taken, tier A). |
| 2026-11-27 | `neodyme-labs` org listing | Workshop, CTF, and the poc-framework. Already P1/v2 sources or not incident PoCs. |
| 2026-11-27 | Repository search `cashio exploit`, `crema finance exploit`, `solend exploit poc`, `raydium exploit poc`, `mango markets exploit poc` | Only the Cashio workshop (taken) and unrelated hits. |
| 2026-11-27 | `Soles-IA/web3-security-writeups` tree and the headers of `gmx-solana-store.md`, `gmx-solana-treasury.md`, `stakeflow.md`, `missionx-practice.md`, `bug-catalog.md`, `arbitrary-cpi` README | Orderly, WOOFi, and Pump Science are already sealed. GMX pages are an in-progress live bounty with no published judgement (treasury: no reportable finding). StakeFlow and MissionX are Solana Audit Arena, which this round does not add. The CPI PoC is the Pump Science finding. The bug catalog is a checklist, not a finding. |
| 2026-11-27 | `NaryaAI/cashio-exploit-workshop` branches `main`, `predicate`, `sunburn` | `main` is the unsolved workshop. `poc` is the same commit as `PwnedNoMore` (taken once). The other branches were not separate incidents. |
| 2026-11-27 | Code search `org:code-423n4 program.methods extension:md` and the Sherlock equivalent | GitHub returned 429. Not retried after the report-page and org listings above; those listings already name the public Solana contests. |

Dedupe against every opened and sealed corpus under `eval/` (including v4 and solidity v2): 0 content-hash or file-URL matches among the 6 counted units.

## Verification

`python -I eval/solana-poc-v5/build_inventory.py --check` → inventory matches the cache.

`python -I eval/solana-poc-v5/fetch.py --check` → all 6 unit pins verified offline.
