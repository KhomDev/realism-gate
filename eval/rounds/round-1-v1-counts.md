# Round 1: V1 opened once per corpus. Flag counts only.

Recorded 2026-11-27 (Europe/Budapest). Scope: `eval/rounds/round-1.json` and its amendment A1.

**These are flag counts from unlabelled runs. They are not precision, not recall, and not a held-out measurement
of anything.** No human and no AI reader has labelled a V1 flag. `eval/solana-poc-v3/measure.py` (Wilson cells and
the 10,000-draw source-clustered bootstrap, seed `p2r-bootstrap-2026-11-25`) needs labels and was **not run**;
there is nothing for it to compute. Every flag is at `note`. V1 is in-sample from now on. V2 and FINAL were not opened.

## What was run
- Binary: realism-gate 1.5.1 (tag `v1.5.1`, commit `77c89d9`), Windows `realism-gate.exe`, sha256
  `9e2af1795df815b8f23594f265760367d06680eacf08c8084fc9c2ca8258a5d6`. Each runner checked it against `--binary-sha256`.
- Runner tree: `main` at `d246817` for solana-poc-v3, solana-poc-v4 and solidity-poc-v1; `19e875d` (after
  amendment A1, PR #44) for solana-poc-v5 and solidity-poc-v2. The runners are byte-identical in both trees.
- One run per corpus: `python -I eval/<corpus>/run.py <exe> --version 1.5.1 --split V1 --round round-1
  --binary-sha256 9e2af179... --cache <pinned cache> --out-dir <dir>`. Counts: `eval/rounds/round-1-v1-counts.json`.

## Results per corpus (Solana and Solidity are never pooled)
| Corpus V1 | units (by tier) | readable [Wilson 95%] | flags | flags on J/X units (to label) | flags on N units |
|---|---|---|---:|---:|---:|
| solana-poc-v3 | 27 (J13 X11 N3) | 26/27 = 96.3% [81.7, 99.3] | 12 | 10 (J6 X4) | 2 |
| solana-poc-v4 | 5 (J4 A1) | 4/5 (n < 5 flags: read as a count) | 5 | 5 (J5) | 0 |
| solana-poc-v5 | 2 (J1 A1) | 1/2 (a count) | 0 | 0 | 0 |
| solidity-poc-v1 | 111 (J96 X8 N7) | 99/111 = 89.2% [82.1, 93.7] | 238 | 111 (J104 X7) | 127 |
| solidity-poc-v2 | 62 (J55 X7) | 62/62 = 100% [94.2, 100] | 68 | 68 (J62 X6) | n/a (no N units) |

Unit counts equal the pre-registration's expected V1 sizes. Solana total to label: 15 flags. Solidity total: 179.

Flags by rule (tier J/X unless marked):
- solana-poc-v3: RG01 2, RG03 1, RG08 5 (+2 on N), RG10 2.
- solana-poc-v4: RG08 1, RG09 2, RG10 2.
- solidity-poc-v1: RG02 39 (+12 N), RG03 5 (+61 N), RG04 1, RG06 5, RG07 7, RG08 20 (+23 N), RG09 6 (+1 N), RG10 2,
  RG11 2, RG16 2, RG17 1, RG18 21 (+30 N).
- solidity-poc-v2: RG02 25, RG03 5, RG05 3, RG06 2, RG07 1, RG08 18, RG10 2, RG11 2, RG17 3, RG18 7.
Units with at least one flag: solana-v3 J 6/13, X 2/11, N 1/3; solana-v4 J 4/4; solana-v5 0/2; solidity-v1 J 58/96,
X 5/8, N 5/7; solidity-v2 J 37/55, X 2/7.

## Caveats
1. Unlabelled. A flag on a J/X unit may be a false positive; a flag on an N unit is not a measured false positive
   either. Nothing here supports a claim of precision, recall or "false alarm rate".
2. The Solidity-v1 N units carry 127 flags (5 of 7 units flagged; RG03 61, RG18 30, RG08 23). That is a reason to
   read the controls, not a rate: 7 units is too few.
3. The Solana corpora are tiny: 15 J/X flags from four sources (arena 4, jupiter-lend 1, rustfund 5, lavarage 5).
   Most per-rule Solana cells will stay below the floor's n even after labelling.
4. Solidity J/X flags cluster by source (v1: stratax 13, ethereumcreditguild 8, salty 7; v2: coinspect 6, then
   several at 4), so labelled cells need the source-clustered bootstrap.
5. `CAP = 30` in the runners shapes only `sample.jsonl`; in V1 it did not bind (sampled = J/X flags in all five
   runs). Labelling reads every J/X flag through `p2r_sheet.py` / `p2r_packet.py --all-flags`.
6. No sheet has calibration rows (`--no-calibration`): there are no opened, labelled flags for these corpora.
7. `p2r_sheet.py` was changed after the freeze to build the solidity-poc-v2 sheet (it did not know the
   `sherlock-report-section-block` pin and the `page-pre-block` pin without `pre_index`); see amendment A2. The
   solidity-poc-v1 sheet rebuilt byte-identically with the changed script.
8. V1 is opened. It is in-sample for any later tuning; Round 2 must not treat it as fresh.

## Sheets
Label sheets (neutral row ids, the rule's claim only, no raised or shipped severity) and blind packets were generated
for solana-poc-v3 (10 flags), solana-poc-v4 (5) and solidity-poc-v1 (111), solidity-poc-v2 (68). The answer keys are
outside the repository. None has been sent. solana-poc-v5 has no flags, so no sheet.

## Labellers
None found. The plan and the slots are in `eval/rounds/round-1-labellers.template.json` (identities and dates
null). Identities and the date go into `round-1-labellers.json` via a PR before any label is collected.