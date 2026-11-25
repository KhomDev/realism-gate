# Preregistration: realism-gate 1.2.0 on the sealed fresh Solana PoC corpus (v2)

Frozen 2026-11-25, after the corpus was collected, pinned and committed
(commit `006f5e4`, before the first `src/` change) and **before the tool was
run on any v2 unit or any v2 label was written**. The code freeze is commit
`7779ce2`; this file is committed before the first v2 run.

Working title: *Does the 1.2.0 PoC reader and the corrected rules hold up on a
fresh Solana PoC corpus?*

## Why this study exists

The P1 PoC study (`eval/solana-poc/`) read 1 of 12 judged PoCs, and its
identity/state rules were mostly wrong (RG01 16.7%, RG02 26.7%, RG04 0%,
RG06 0% on the held-out split). P1-C fixes the readers and the rules against
that error analysis. Any number on the P1 PoCs after the fixes is an
**in-sample re-measurement**: it shows the fixes work on the cases they were
designed from, not that they generalise. The headline must come from this
sealed corpus.

## What is frozen

- **Rule logic and version**: realism-gate **1.2.0**, commit
  `7779ce2` (`p1c/fixes-2026-11-25`). Binary: `target/release/realism-gate.exe`
  built with `cargo +1.95.0-x86_64-pc-windows-gnu build --release --locked`
  from that commit; sha256 `3c145dc3aec5c7ba85058e3bb8e73d2179b9aa04334349a3e029e7c8b02e3baf`.
  No rule, severity or reader change after this file is committed.
- **Corpus**: `eval/solana-poc-v2/`, committed at `006f5e4`. 81 counted units
  (28 judged, 53 exploit-by-construction), 0 duplicates. Every git source is
  pinned by commit and listing hash, every Arena issue by body hash, every
  rendered report page by file hash; `fetch.sh --check` verifies all of them.
- **Inventory**: `inventory.json`, built by `inventory.py` from decisions
  written in that script. Kinds are recorded from the file alone at collection
  time, before the 1.2.0 readers existed.
- **Seed**: `rg-solana-poc-v2-2026-11-25`.
- **Sampling**: at most 30 flags per rule per source, ordered by
  `sha256(seed + ':' + flag_id)`; every flag when a rule raised 30 or fewer on
  a source. Only J and X flags are sampled.
- **Split**: a sampled flag is `dev` when
  `int(sha256(seed + '|' + source + '|' + flag_id)[:8], 16)` is even, and
  `heldout` otherwise. Held-out first, then dev.
- **Minimum-n rule**: a rule/language cell with fewer than 5 labels is a count,
  never a percentage headline.

## Rubric decisions (written before any label)

The P1 rubric applies, with these decisions:

1. **RG08's scope** is the rule's definition: "no assertion on the exploit's
   outcome" (the exploit part of the trace). An assertion in setup does not
   silence it; a test that asserts nothing at all is a true positive. The P1
   rubric's "anywhere" wording is withdrawn.
2. **A precondition fabricated in setup** (RG04/RG06 on a victim's funded
   account, with the drain going through the program) is a **true positive for
   the note-level finding** whose message begins `Precondition fabricated:`.
   The finding claims exactly that the precondition never passed through the
   program's accounting; it does not claim the exploit is fabricated.
3. **A fixture the program initialises** (the test writes an account, then an
   init/create instruction of the program under test sets it up) is not a
   finding: FP if the rule fires.
4. **A control test** (expects failure, or a title saying secure/control/
   prevents/rejects/fails when) is judged by the P1 rubric's feature-test
   clause: RG01/RG02/RG03/RG04/RG06 are FP if they fire; RG08/RG09 are judged
   on their own defect.
5. **The attacker's own key** (a fresh keypair with no grant, role field,
   initialise call or declaration) is the attacker; RG01/RG02/RG03 are FP if
   they call it privileged.
6. **The attacker taking the role through the bug** (it signs the
   grant/re-initialise itself) is the exploit, not a harness grant; RG01 is FP
   if it fires on that grant.
7. **RG10**: a measurement of the same account read before the attack is a
   baseline; RG10 is FP if it fires when such a read exists.
8. **Unreadable units** are never dropped. A unit is readable when
   `summary.tests > 0`. A fragment read with `--fragment` counts as read and
   the run records `fragment: true`; the wrapper is the tool's test boundary,
   and the label reason says so.

## Tiers

- **J, judged (28)**: 25 Solana Audit Arena issues the maintainer labelled
  (`Valid` / `invalid` / `duplicate` + severity; the week's final report is in
  the arena checkout under `reports/`) and 3 Immunefi Audit Competition | Jito
  Restaking reports whose published report contains a test block.
- **X, exploit-by-construction (53)**: Arena submissions whose week is not
  judged yet, Rust PoC-framework binaries, teaching exploit tests, and public
  reproductions of known bugs.
- **NC**: none. The fresh sources contain no new exploit/secure pair; this is
  stated rather than manufactured.

## Analysis plan

- **RQ1 coverage**: readable units / units, per tier and kind, pooled; the
  unreadable units by reason, as counts. Headline target on the P1 set:
  judged readability >= 8/12 on the 10 fixable judged PoCs; exploit-by-
  construction >= 80/82; any regression in a previously readable unit is a
  failure.
- **RQ2 precision**: per rule and language on the held-out split, TP/(TP+FP)
  with Wilson 95% intervals (z = 1.959964); a source-clustered bootstrap
  interval beside Wilson for every pooled cell (resample sources, 10,000
  draws, seed `rg-solana-poc-v2-bootstrap-2026-11-25`).
- **RQ3 flag vs judged outcome**: exploratory 2x2 (flagged x valid/invalid)
  on J units with a platform outcome; Fisher exact and the risk difference
  with a Newcombe interval. Valid includes `Valid` + `duplicate`; `invalid` is
  the other class.
- **RQ4 negative control**: none in v2; not reported as a number.
- **RQ5 comparison with P1**: P1's published populations and numbers beside
  the v2 numbers, never pooled. In-sample v2 re-measurement is reported
  separately and every table is marked **in-sample**.
- **Severity floor**: a rule ships at `error` on Solana only when a fresh
  held-out cell's Wilson lower bound is at least 0.80; `measured.json` is
  updated from fresh held-out cells only.

## Expected outcome, stated in advance

The new readers should make most judged PoCs readable (the four Rust unit
tests, the fragments and the client-driving scripts; a pure simulation script
that drives no client stays unreadable). The identity/state rules should fall
to note-level and stop firing on the attacker's own keys and control tests.
The fresh judged tier is 28 units, larger than P1's 12, so the RQ3 table can
say a little more than P1's, but it is still not a population estimate.
