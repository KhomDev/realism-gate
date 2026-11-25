# NEAR PoC corpus v1 (collected 2026-11-25)

A pinned starter corpus for a future NEAR reader. **realism-gate has no NEAR
frontend yet** (1.2/1.3 parse Solana Rust, Solidity and Move); nothing here is
run or measured, and no number is claimed from it.

## What is here

| Source | Pin | What it contributes |
|---|---|---|
| `jatinsahijwani/exorcist` | `c7a7b0d` | 9 `near-workspaces` sandbox tests that replay Lean-model attack traces against real vault contracts (kind `near-workspaces-test`) |
| `danielAsaboro/exploit-escrow` | `3858e82` | screened: self-audit prose, Kani harnesses and testnet demos, no near-workspaces PoC test |
| `skyhighdesignsinc-sudo/baseline-near` | `74bdc45` | screened: web e2e only |

The units are pinned by `corpus.json` (commit + listing hash) and recorded in
`inventory.json` with the reason for every exclusion. No third-party text is
committed.

## Why it is not measured

The judged tier is empty: the deeper search found no public NEAR contest or
bounty that publishes judge-grouped coded PoCs (the NEAR audit material found
is firm reports and self-audits). The X tier has one source. A NEAR reader
would also need its own harness vocabulary (`near_workspaces::sandbox`,
`dev_deploy`, `call`, `view`, `patch_state`), identity model (predecessor vs
signer), and state model (`patch_state` is the NEAR counterpart of RG04), none
of which exists yet. Collecting the corpus now pins what a later reader will
be measured on; it does not pretend to a measurement.
