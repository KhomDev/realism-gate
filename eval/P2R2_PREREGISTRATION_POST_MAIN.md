# P2R-2 pre-registration for the held-out rounds after main (committed before any V1, V2 or FINAL slice is opened)

Committed 2026-11-27 (Europe/Budapest). It supplements `eval/P2R_PREREGISTRATION.md`,
`eval/P2R_AMENDMENT_2.md`, `eval/P2R2_ANALYSIS_PLAN.md` and `eval/P2R2_ANALYSIS_PLAN_ADDENDUM_1.md`.
Where this file differs from them, it states the change and the reason, and the change applies from
here on. No V1, V2 or FINAL unit of any corpus has been run, labelled or read.

## 0. What ships to main, and what is not claimed
- The release that goes to `main` carries **DEV-only (in-sample) numbers**. Each one is labelled
  DEV-ONLY in `docs/precision.md`, `docs/benchmark.md` and `docs/methodology.md`. **No held-out
  precision is claimed for the P2 binaries.**
- RG-A5 ships every rule at `note`, because no committed cell is fresh. Reports show the raised level
  next to the shipped level, and `--severity RULE=LEVEL` overrides it.
- The 1.4.0 held-out rows in `docs/precision.md` (Solana test suites) were measured on the 1.4.0
  binary. They are kept as history and are not numbers for this binary.

## 1. Code under test
| Item | Value |
|---|---|
| Source tree at pre-registration | `p2/integration` `d0d5e646f9ac57ad6f78a693151db8524a09e415` |
| Toolchain | `1.95.0-x86_64-pc-windows-gnu` (host); CI pins in `.github/workflows/ci.yml` |
| Round-1 binary | sha256 to be recorded in `eval/rounds/round-1.json` **before** V1 is opened (frozen commit, rubric version, labeller IDs) |
| Golden manifest | `tests/golden/manifest.json` sha256 `3b1dbe7d09d8bed2963bdd602c40515b880e4441b53ec40ac401168d16b43496` (117 cases) |

## 2. Dataset manifest (sha256 of the committed LF files)
Split rule: `int(sha256(salt|id)[:8],16) % 100`, DEV 0-39, V1 40-59, V2 60-74, FINAL 75-99
(each runner's salt). Tiers are J (judged), X (exploit/PoC with an external verdict), A (author
assertion), N (negative control).

| Corpus | Inventory sha256 | Units | DEV | V1 | V2 | FINAL |
|---|---|---:|---|---|---|---|
| `solana-poc-v3` | `d8b9fe78ab0d3948b37051422fe861cb203b7457301907f817a65fac6e0ab7f0` | 134 | J18 X27 N5 | J13 X11 N3 | J16 X9 N4 | J11 X13 N4 |
| `solana-poc-v4` | `c5529eba3828af72e4b384f4c88e04d8c9a56c51361c67bfb9e795cdaa7fa609` | 13 | J2 A3 | J4 A1 | A1 | A2 |
| `solana-poc-v5` | `35494651f35538759abf0d3272423c1664cc3f0b1f7c085a327888283548418f` | 6 | J1 X1 A2 | J1 A1 | - | - |
| `solidity-poc-v1` | `7a2ce2ace8ecbf6a52af5f87bcf1d1717459aa7ec178b87c7b12efb1ce5de5b4` | 586 | J213 X12 N9 | J96 X8 N7 | J84 X7 N4 | J127 X9 N10 |
| `solidity-poc-v2` | `fbdfe62a5c45baa521c4aa5343fd0d1129fe4161c26ded5a2027a76344714348` | 369 | J139 X20 | J55 X7 | J51 X5 | J82 X10 |
| `solidity-poc-v3` | `36cc8cfaaadd513340c52bcc5c7411d984feec4534a0be51fc60e8f229c5f0ef` | - | **excluded from every measurement** (Yura, 2026-11-27: 35/35 units are near-copies of opened DeFiHackLabs `d4bd931`; see `eval/solidity-poc-v3/README.md`) | | | |
| DeFiHackLabs `d4bd931` | `bench/defihacklabs.json` | 658 findings | opened tuning population: in-sample only, never held-out | | | |

Counts are units marked counted in each inventory. They were read from the split and tier fields
only, with no unit opened.

## 3. Metrics (unchanged from the analysis plan)
- Per rule × language precision on J+X, the pooled J+X precision, and readability (units with at least
  one test read, over all units). Each comes with a Wilson 95% interval; n < 5 is reported as a count;
  more than 20% unclear makes a cell inconclusive. Pooled cells carry the 10,000-draw source-clustered
  bootstrap (seed `p2r-bootstrap-2026-11-25`).
- Floor: `warn` needs a fresh cell with Wilson LB ≥ 0.50 and n ≥ 10; `error` needs LB ≥ 0.80 and n ≥ 16. Only
  FINAL sets a shipped severity (analysis plan §3, including the stated selection bias).
- Solana and Solidity are never pooled, nor are different binaries, nor DEV/opened data with fresh data.
  DEV and opened data are reported in a **separate table**.

## 4. Changes to the labelling plan (decided by Yura, 2026-11-27)
1. **Every held-out flag is labelled; there is no cap.** This replaces the per-(source, rule) cap of 30 in
   analysis plan §4 for V1, V2 and FINAL. Reason: the cap left the per-rule cells too small to reach
   the floor's n. DEV keeps the seeded cap, because it is descriptive.
2. **Labellers: two blinded external labellers.** This replaces the two AI readers in analysis
   plan §4. Each labels independently with rubric v3 (`docs/labeling-rubric.md`). Labeller B is blind to A and
   to the severity. Each labeller sees the flag, the file and the rule text, never the tool's
   raised or shipped level. If Cohen's kappa is below 0.70, the cell is inconclusive. Every
   disagreement gets a written adjudication. Labeller identities and the date are recorded in the
   round file before labelling starts.
3. `solidity-poc-v3` is excluded (Addendum 1 §2 is resolved: exclusion).

## 5. Held-out plan (after main)
1. Round 1: freeze (binary sha256 and rubric version in `eval/rounds/round-1.json`), open V1 once per
   corpus, label every flag (two labellers), report the V1 cells. V1 is in-sample after that.
2. Round 2: fixes only at class level, from DEV and V1; freeze; open V2 once.
3. FINAL: commit `FINAL_PREREGISTRATION.md`, freeze, open FINAL once per tool; FINAL sets the shipped severity
   by the floor.
4. Never: tuning on a sealed slice, a runner `--check` with placeholder round/version/sha on a sealed
   split, or pooling across the boundaries in §3.
5. Known limit, stated in advance: the Solana FINAL slice has J11 + X13 (v3) + A2 (v4) units, so some Solana
   rule cells cannot reach n ≥ 16 at FINAL, whatever the precision. Those rules stay at `note`.