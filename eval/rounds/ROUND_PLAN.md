# Round plan and handoff: from returned labels to FINAL (written 2026-11-27 22:25 Budapest)

Audience: the next agent, once any labeller or researcher returns labels. The same file is committed in realism-gate and priorart
(`eval/rounds/ROUND_PLAN.md`); realism-gate's copy is canonical. `<ORCH>` is the operator's orchestration folder on Yura's PC,
`_orchestration` (its full path is in the progress file, not in the repo). Nothing below has been done unless the
state section says so. Every step that is marked **GATE** needs Yura's explicit go-ahead in the current conversation; an earlier yes does not carry over.

## 0. Rules that never bend

1. **FINAL and V2 are sealed.** No agent reads, samples, counts, extracts, runs `--check` on or tunes against any FINAL or V2 unit of any
   corpus (solana-poc-v3/v4/v5, solidity-poc-v1/v2/v3, Dup-Bench FINAL, priorart's sealed pairs are V1 only). Never run a runner
   `--check` with placeholder `--round/--version/--binary-sha256` on a sealed split: that extracts sealed code.
2. **Never tune on opened data.** V1 and DEV are in-sample from now on. A rule or threshold change after V1 is a new freeze, and its V1 numbers stay labelled in-sample.
3. **No claim without its label status.** Flag counts and AI-reader numbers are not precision. Judge-labelled (priorart pairs) is not human-labelled. Every number is k/n with a Wilson 95% interval and its slice; n < 5 is a count; more than 20% unclear makes a cell inconclusive; kappa < 0.70 makes a cell inconclusive. Solana and Solidity are never pooled.
4. **Nothing outward without a separate line from Yura:** no sending of mail, DMs or posts, no publishing, no tags, no releases, no registry pushes. Outreach is done by Yura; do not redraft it.
5. **Normal workflow:** PR as KhomDev, CI green, merge normally, never `--admin`, never force-push main. Never print or commit tokens, keys or labeller contact data. Labeller identities are recorded only as Yura allows (consent_to_be_named).
6. Host: one heavy job at a time (`<ORCH>\tools\serial.sh`), `-j 2`, `CARGO_PROFILE_RELEASE_LTO=false`; realism-gate `cargo +1.95.0-x86_64-pc-windows-gnu`; priorart `cargo +stable-x86_64-pc-windows-gnu` was broken upstream-side on this host (dlltool), so check `rustup toolchain list` and use 1.95.0-gnu if it still fails. Hash git blobs, not working trees (`core.autocrlf=true` on the host).

## 1. State at handoff (verified 2026-11-27 22:20 Budapest)

| Item | State |
|---|---|
| realism-gate main | `8d76b93` (PR #50), CI green; no open PRs |
| priorart main | `f3799cf` (PR #48: the rubric, sheet builder, unit tests and CI job; see its PR); PR #42 merged as `2d99dd3` |
| Round 1 freeze | `eval/rounds/round-1.json` (realism-gate): v1.5.1 `77c89d9`, binary sha256 `9e2af1795df815b8f23594f265760367d06680eacf08c8084fc9c2ca8258a5d6`, priorart 0.2.2 binary `54d23059b3ffc2d7a6f9602f3e03cd1d58b71cc333e487304301dad14df06478`; amendments A1, A2 |
| V1 opened once | solana-poc-v3, solana-poc-v4, solidity-poc-v1, solana-poc-v5, solidity-poc-v2; priorart sealed pairs V1 (judge-labelled) |
| V1 counts | `eval/rounds/round-1-v1-counts.md` and `.json` (counts only, no precision) |
| priorart pairs V1 | pooled recall@5 350/445 = 78.7% [74.6, 82.2]; FAR 2/104 = 1.9% [0.5, 6.7]; warning precision 43/48 = 89.6% [77.8, 95.5]; abstention 81/549. Judge-labelled, not human. `eval/rounds/round-1-pairs.json` |
| Human labels | none received as far as this file knows. Outreach was done by Yura; whether sheets have gone out is his to say. Ask before assuming. |
| Labeller plan | `eval/rounds/round-1-labellers.template.json`: 30 unpaid English labellers; Solidity 18 (9 batches x 2), Solana/Rust 4 (all 15 flags), priorart duplicates 8 (4 batches x 2); 3 anchors per area. Identities null. |
| Not on main | `round-1-labellers.json` (does not exist yet), `round-1-RESULTS.md` (does not exist), any Round 2 or FINAL file |

## 2. Where incoming labels go (exact paths)

Private material (keys, sheets, issue text) stays outside both repos. Create `<ORCH>\labels-incoming\` (does not exist yet) and drop each returned file there unmodified, one file per labeller slot, never edited:

| Area | Returned file goes to | Sheet it answers (already built) | Key (never shown to labellers) |
|---|---|---|---|
| Solidity v1 | `<ORCH>\labels-incoming\solidity-poc-v1\<slot>.csv` | `<ORCH>\logs\p2r2\v1\solidity-poc-v1\sheet\LABEL_SHEET.csv` (and `.md`) | `<ORCH>\private\p2r2-v1-keys\solidity-v1.json` |
| Solidity v2 | `<ORCH>\labels-incoming\solidity-poc-v2\<slot>.csv` | `<ORCH>\logs\p2r2\v1\solidity-poc-v2\sheet\` | `<ORCH>\private\p2r2-v1-keys\solidity-v2.json` |
| Solana v3, v4 | `<ORCH>\labels-incoming\solana-poc-v3\<slot>.csv`, `...\solana-poc-v4\<slot>.csv` | `<ORCH>\logs\p2r2\v1\solana-poc-v3\sheet\`, `...\solana-poc-v4\sheet\` | `<ORCH>\private\p2r2-v1-keys\solana-v3.json`, `solana-v4.json` |
| Solana v5 | `<ORCH>\labels-incoming\solana-poc-v5\<slot>.csv` | `<ORCH>\logs\p2r2\v1\solana-poc-v5\` (2 units; 0 J/X flags to label per the template, so expect nothing) | none |
| priorart duplicates | `<ORCH>\labels-incoming\priorart-pairs\<slot>.csv` where slot is `P1-A` ... `P4-B` | `<ORCH>\private\p2r2-priorart-judging\batches\<slot>\PAIR_SHEET.csv` (`.md`, `rowkey.json` per slot) | `<ORCH>\private\p2r2-priorart-judging\items.json` and `plan.json` |

The `<slot>` naming above is a convention this file introduces; the sheets' own slot ids are authoritative. Check that the folder listings match before relying on them. For each returned file: record the sha256, the received time (Budapest) and the slot; confirm the row count equals the sheet's (anchors included); labels are exactly `true-positive`/`false-positive`/`unclear` for realism-gate and `yes`/`no`/`unclear` for priorart. Reject and report any other vocabulary; do not repair silently.

## 3. Fill the round-1 labeller amendment (before any label is used)

Pre-registration 4.2: identities and date are recorded **before any label is collected**. If labels already came back before this is merged, say so in the amendment (`"recorded_after_labels": true`) and in every result. Do not backdate.

1. Copy `eval/rounds/round-1-labellers.template.json` to `eval/rounds/round-1-labellers.json`. Keep `schema`, `plan` and `paid: false`.
2. For each of L01..L30 fill only what Yura provides: `identity`, `handle_or_alias`, `contact_recorded_where` (a pointer to a private place, never the contact itself), `consent_to_be_named` (true/false; if false, publish the alias only), `conflicts_declared`, `date_recorded` (ISO 8601 with the Europe/Budapest offset). Unfilled slots stay null and are reported as unfilled; do not invent or infer identities.
3. Add an amendment A3 to `eval/rounds/round-1.json` (`id`, `recorded`, `authority` quoting Yura's line, `changes`, `unchanged`) that points to the new file, records the sha256 of the sheet builders used (`eval/p2r_sheet.py`, and priorart `eval/dupbench/pairs-swap/judging_sheet.py` plus the rubric `JUDGING_RUBRIC.md`) and states that binary, rubric, pre-registration, golden manifest and the sealed slices are unchanged.
4. PR, CI green, merge. Then, and only then, unblind and score.

## 4. Scoring commands

Run from the repo root, one heavy job at a time. Run each command with `--help` first: flags below are taken from the scripts' usage text on main at 22:20 and not re-run by me.

**realism-gate (per corpus)**
```
python -I eval/p2r_sheet.py --unblind <ORCH>\labels-incoming\<corpus>\<slot>.csv --key <key json> --out <ORCH>\labels-incoming\<corpus>\<slot>.human.csv
python eval/p2r_score_human.py --run-dir <ORCH>\logs\p2r2\v1\<corpus> --human <slot>.human.csv --labels <adjudicated ai labels jsonl> --version v1
python eval/solana-poc-v3/measure.py --flags <ORCH>\logs\p2r2\v1\<corpus>\flags.jsonl --labels <resolved labels jsonl> --measured <scratch measured.json> --results <scratch results.json> --corpus <solana-v3|solidity-v1>   # add --tiers J,X for verdict cells; --tiers J and --tiers A are descriptive
python -I eval/p2r_counts.py --run NAME=DIR --json out.json
```
Notes: `measure.py` takes a `--corpus` vocabulary; Solana and Solidity runs are separate invocations. Write `--measured` and `--results` to scratch paths, never over the committed `crates/rg-rules/data/measured.json` (FINAL cells only go there, much later). Two labellers per batch: report Cohen's kappa per batch pooled, and Krippendorff alpha on the shared anchors; kappa < 0.70 means the cell is inconclusive and that is what gets reported. Disagreements get a written adjudication (reason per id) that replaces the label for the cell, never for kappa. If there are no AI-reader labels yet, `p2r_score_human.py` has nothing to compare to: say that instead of faking it.

**priorart pairs (human second opinion on the judge result)**
No scorer exists yet. Write `eval/dupbench/pairs-swap/score_judging.py` with unit tests (stdlib only, the CI job `eval python (unit tests)` already runs `test_*.py` in that folder). Inputs: returned `PAIR_SHEET.csv` per slot, `rowkey.json` per slot, `items.json`. Report, each as k/n with Wilson 95%: (a) per batch Cohen's kappa between A and B, pooled over the 4 batches, and Krippendorff alpha on the 3 anchor rows across all 8 sheets; (b) among the 48 alarm firings, the share labelled `yes` by humans next to the judge-labelled 43/48; (c) among the 48 quiet controls, the share labelled `yes`. Never pool human and judge labels. The V1 numbers in `round-1-pairs.json` are not recomputed or changed.

## 5. Error analysis (class level, DEV and V1 only)

Never by reading a FINAL or V2 unit. Per rule and per class, list false positives and misses from the human-labelled V1 flags and the DEV labels, name the class, then decide fix or document. Known queue (from the P2R-2 plan):
1. `drives_harness` drops tests whose only calls are helpers absent from the fragment.
2. `src/cli.rs` `has_top_level_test` matches `it(` inside identifiers (`deposit(`).
3. Comment-only stubs carry no unreadable reason.
4. TS `tx.feePayer` plus `new Transaction().add(ix)` models the entry as `add`.
5. Dialect difference in the assertion actor.
6. Three pre-existing `src/engine.rs` mutation survivors (lines 63, 288, 298); fuzz seeds for the RG-A6 parse paths.
7. RG-A4 `check --execute` is Round 2 per Amendment 2.
Every fix is a new commit with a deliberate-break test and a golden diff, then a new freeze (section 6). Report the in-sample gap honestly: a gain on DEV or V1 after tuning on them is not evidence.

## 6. Round 2, V2 and FINAL gates

| Step | What | Gate |
|---|---|---|
| Round 2 freeze | New `eval/rounds/round-2.json` (commit, binary sha256, rubric, scripts, labellers, expected units) merged to main before V2 is read | **GATE:** Yura's explicit yes, plus a green main |
| Open V2 once | realism-gate only; priorart has no V2 slice (say so). Same procedure as V1: counts first, labels later, no precision without labels | **GATE:** Yura's explicit yes |
| FINAL pre-registration | `FINAL_PREREGISTRATION.md` per repo: cells per the analysis plans, the "above note" rule, the dataset cut-off, what a pass and a fail mean. Committed and merged before the freeze | **GATE:** Yura's explicit yes on the text |
| FINAL freeze | `eval/rounds/final.json`: commit, binary sha256, readers, labellers | **GATE:** Yura's explicit yes |
| Unseal FINAL once per tool | Open once, record the opening in the progress file with time and command; no re-run after seeing results except the `--check` reproduction | **GATE:** Yura's explicit yes, given just before the run |
Known before FINAL, from the analysis plans: the amended Solana FINAL minimum (at least 60 labelled J+X flags from at least 4 clusters) is unreachable on public data (about 16 units, 2 clusters, about 11 flags); priorart's abstention row misses its 25% target (43.9% on train). Expect PARTIAL as the best overall verdict. Say it before the run, not after.

## 7. Verification checks, every session

1. `git rev-parse --short origin/main` in both repos; `gh run list --branch main -L 3` green; `gh pr list` for anything open.
2. Sealed-id grep: the 436 ids of v3 and v1 V1/V2/FINAL plus the v4/v2 V1/V2/FINAL ids, whole-word, over `<ORCH>\_tmp`, `*\.corpus`, `worktrees\*\tmp`, `worktrees\*\.corpus`: 0 hits required. Stop and report on any hit.
3. Pinned binaries: sha256 of the realism-gate 1.5.1 release binary and the priorart 0.2.2 `--no-default-features` build equal the values in `round-1.json`. Rebuild only when a freeze says so; a deliberate-break build leaves a broken binary, so rebuild before any measurement.
4. Re-run `python -I -m unittest discover -s eval/dupbench/pairs-swap -p "test_*.py"` (32 tests at handoff) and the repos' own suites on the commit you measure with.
5. Every report states: slice, corpus, commit, binary sha256, label status, exact k/n, interval, anything unlabelled.
6. Update `<ORCH>\P2R2_PROGRESS_2026-11-27.md` as you go; times in Europe/Budapest.

## 8. Stale items from `PROMPT_P2R2_CONTINUE_2026-11-27.md` to re-check (not verified at handoff)

The continue prompt predates most merges. These were not re-verified when this file was written; check them instead of trusting the prompt.
1. **RG-A6 changed-lines mutants** on the frontend parse paths: `gh workflow run mutants.yml --ref main -f files="crates/rg-frontend-ts/src/parse_ts.rs crates/rg-frontend-rust/src/parse_rust.rs" -f base=ae654cf`. Local logs exist at `<ORCH>\logs\p2r2\mutants-rga6-45938826459` and `mutants-rga6-45946711166`; read them, then confirm the missed count on the current head or re-dispatch. Say which.
2. **Coverage run ids** (`coverage.yml`) for both repos on the current main heads: find the latest run, record its id and the percentage, or dispatch one and record it.
3. **Weekly fuzz** after RG-A6's new parse paths: logs `<ORCH>\logs\p2r2\fuzz-45972434655`, `fuzz-45977542883`, `fuzz-45978548735` exist; confirm a green run on the current main head or dispatch once and record the run id.
4. Open PR list from the prompt (demo GIFs, lockfile bumps, cargo-deny-action) is obsolete: both repos have no open PRs at handoff.
5. Prompt's `p2/*` branch heads are old; do not use that table.
6. Host toolchain note: confirm `stable-x86_64-pc-windows-gnu` state before choosing a priorart toolchain; the default `fetch` build needs `gcc.exe` (ring), which is not on the msys-free PATH, hence `--no-default-features`.

## 9. After results exist

`eval/rounds/round-1-RESULTS.md` in each repo (every number k/n + Wilson, label status in the first line); update `docs/precision.md` (realism-gate) and `docs/benchmark.md` (priorart) only with what the labels support; article draft at `<ORCH>\posts\` (new file, never edit the published article), labelled Round-1/V1 if FINAL was not reached; `P2R_STATUS.md`. Nothing is posted without Yura.