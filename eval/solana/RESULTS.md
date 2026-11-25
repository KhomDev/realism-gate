# Rust and Anchor TypeScript rule precision

**Population: public test suites, not judged PoCs.** The four corpora are pinned public
Rust and Anchor TypeScript test suites. This page originally said the intended PoC corpus
does not exist publicly in a form this tool reads; that was wrong (the search stopped after
three repositories). The PoC measurement is `eval/solana-poc/RESULTS.md` (2026-11-25).
Every number below is precision on public test suites. It is not PoC precision, and it
must not be quoted as such.

Frozen rule logic: realism-gate 1.0.4 at `6019773976706e42e11ceefc2ee1b03496f2df54`. Seed: `rg-solana-2026-11-24`. The headline is the held-out split; each flag was assigned by `sha256(seed|corpus|flag_id)` before any label was written.

Counts: 174 flags, 133 sampled (at most 30 per rule per corpus), 63 held-out labels. 70 dev-sampled flags are unlabelled; the held-out table is the measurement.

## Held-out precision

| Rule | Language | Precision | n | Unclear |
|---|---|---|---:|---:|
| RG02 | javascript | 0/5 = 0.0% [0.0-43.5] | 5 | 0 |
| RG02 | rust | 0/21 = 0.0% [0.0-15.5] | 21 | 0 |
| RG02 | typescript | 0/8 = 0.0% [0.0-32.4] | 8 | 0 |
| RG03 | rust | 0/6 = 0.0% [0.0-39.0] | 6 | 0 |
| RG03 | typescript | 0/2 = 0.0% [0.0-65.8] | 2 | 0 |
| RG04 | rust | 0/6 = 0.0% [0.0-39.0] | 6 | 0 |
| RG08 | javascript | 1/1 = 100.0% [20.6-100.0] | 1 | 0 |
| RG08 | rust | 5/5 = 100.0% [56.5-100.0] | 5 | 0 |
| RG08 | typescript | 5/5 = 100.0% [56.5-100.0] | 5 | 0 |
| RG09 | rust | 1/1 = 100.0% [20.6-100.0] | 1 | 0 |
| RG10 | rust | 0/2 = 0.0% [0.0-65.8] | 2 | 0 |
| RG10 | typescript | 0/1 = 0.0% [0.0-79.3] | 1 | 0 |

## Unmeasured

A rule with no held-out label is not 100%. It is unmeasured, with the reason.

| Rule | Language | Reason |
|---|---|---|
| RG03 | javascript | no sampled flag on the held-out split |
| RG04 | javascript | no sampled flag on the held-out split |
| RG04 | typescript | no sampled flag on the held-out split |
| RG09 | javascript | no sampled flag on the held-out split |
| RG09 | typescript | no sampled flag on the held-out split |
| RG10 | javascript | no sampled flag on the held-out split |
| RG11 | javascript | no sampled flag on the held-out split |
| RG11 | rust | no sampled flag on the held-out split |
| RG11 | typescript | no sampled flag on the held-out split |

## Agreement

A blinded second pass on a seeded 20-flag sample: raw 19/20 = 95.0% [76.4-99.1]; after the one adjudication 20/20 = 100.0% [83.9-100.0]. The disagreement is recorded in `agreement.json`; it changed the RG04 cell.

## What this cannot claim

- It is not PoC precision; the population is public test suites.
- It is not recall; a rule that raised nothing is unmeasured.
- It is not a property of future versions; it is one tree at one commit.
- It does not establish that a rule is safe to ship at error severity.
