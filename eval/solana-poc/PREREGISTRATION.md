# Preregistration: realism-gate on real Solana PoCs (P1-A redo)

Frozen 2026-11-25, after the inventory was built and **before the tool was run
on any PoC in this corpus and before any label was written**. The commit that
adds this file precedes the commit that adds the first flag, and the first
label comes later still; the commit order is the evidence.

Working title: *Does a static PoC-realism checker hold up on real Solana exploit
proofs? An empirical study of realism-gate on judged and exploit-by-construction
PoCs.*

## Why this study exists

The P1 measurement (`eval/solana/`) ran the Rust/Anchor TypeScript rules on four
public **test suites** and said a public PoC corpus "does not exist in a form
this tool reads". That claim was wrong: the P1 search checked three repositories
and stopped. This study measures the same frozen rules on public PoCs.

## What is frozen

- **Rule logic**: realism-gate 1.0.4 at commit
  `6019773976706e42e11ceefc2ee1b03496f2df54`. The binary used is
  `target/release/realism-gate.exe` (`realism-gate 1.0.4`, sha256
  `6fab967a7dd41cb3af3599e5728b6783ec799f799d521695b20e0ef8a767ba09`), built
  2026-11-24 from `p1/precision-2026-11-24`, whose `src/` differs from
  `6019773` only in the measured-status display (`cli.rs`, `catalog.rs`
  markdown, `rules/measured.rs`). Before trusting it I re-ran P1's `flags.py`
  on the litesvm corpus and got the committed 15 flag ids exactly. No rule,
  severity or predicate changes during this study.
- **Corpus**: `corpus.json` pins every source by commit and by the sha256 of
  `git ls-tree -r <commit> -- <paths>`; `fetch.sh --check` verifies all 45.
  One report page (Code4rena Pump Science) is a rendered page and is not
  hash-pinned; the two PoC blocks taken from it are pinned by normalised
  sha256 in `inventory.json`.
- **Inventory**: `inventory.json`, built by `inventory.py` from the decisions
  written in that script, is committed with this file. 103 units, 4 duplicates,
  99 counted: 12 judged (J), 82 exploit-by-construction (X), 5 negative
  controls (NC). 309 candidates excluded with a reason.
- **Seed**: `rg-solana-poc-2026-11-25`.
- **Sampling**: at most 30 flags per rule per source repository, ordered by
  `sha256(seed + ':' + flag_id)`; every flag when a rule raised 30 or fewer on
  a source. Only J and X flags are sampled; NC flags are counted, not labelled.
- **Split**: same formula as P1 with the new seed and the source in place of
  the corpus: a sampled flag is `dev` when
  `int(sha256(seed + '|' + source + '|' + flag_id)[:8], 16)` is even, and
  `heldout` otherwise. The headline is the held-out split. Both splits are
  labelled (P1's deviation of leaving dev unlabelled is not repeated); held-out
  first, then dev.
- **Minimum-n rule**: a rule/language cell with fewer than 5 held-out labels
  is reported as a count ("2/3"), never as a percentage headline.

## Tiers and inclusion

- **J, judged**: a coded PoC submitted to a contest or bounty whose outcome is
  published. Outcome source `platform-report` (the Sherlock report in the
  judging repository's README; the Code4rena public report) or
  `self-reported` (the author's repository says the item was approved).
  RQ3's primary analysis uses `platform-report` only.
  - Sherlock outcome rule, verified against the repositories (and stated
    because the prompt's assumption was wrong): a numbered folder is a
    **duplicate family**, valid or not. An issue is valid when its family's
    lead, or the issue itself at the root, is a `Source:` of the published
    report and the author's handle is in that finding's "Found by" list.
    Orderly has 19 families and 3 valid findings; WOOFi has 8 families and 5.
    The GitHub labels on these issues are sponsor labels only
    (Sponsor Confirmed/Disputed, Will/Won't Fix), not judge labels.
  - A Code4rena report lists valid findings only, so its PoCs are all valid.
- **X, exploit-by-construction**: a test or program whose purpose is to show an
  exploit succeeding against a Solana program (CTF solves, teaching
  exploit/secure pairs, audit-bootcamp PoCs, public PoCs of found bugs).
- **NC, negative control**: the patched counterpart of an X file
  (`secure.ts` beside `exploit.ts`).
- **P, the operator's private PoCs**: dev/sanity only; measured in a private
  note; never in a public file, table or the article.
- **Excluded**: templates (an empty "add your code" slot, an empty `hack()`,
  Anchor's default `it("Is initialized!")`), source quotes and fixes in issues,
  sponsor test suites, regression/security suites where the attack is expected
  to fail, tool/agent/prompt/fuzzer source, non-Solana PoCs. The code-search
  screen's rule and every excluded repository are in `inventory.json`.
- **Dedupe**: a unit whose first file has the same normalised sha256 (CRLF to
  LF, trailing spaces stripped, leading/trailing blank lines dropped) as an
  earlier unit's is a duplicate and is not run or counted.
- **Unlicensed repositories**: included as pointers only (operator decision
  2026-11-25). `inventory.json` marks each unit `unlicensed`.

## Extraction rule (PoCs in issues and reports)

`extract.py` writes the named fenced code block verbatim (LF line endings) to a
gitignored work tree. Nothing is wrapped, completed or given imports. When the
warden names a place (or says "Add" in a test suite), the code repository's
directory at the pinned commit is copied first so relative imports resolve,
and the PoC is written into it; "replace X" overwrites X in the copy. The
extracted text is never committed.

## What the tool is given, and readability (RQ1)

For each counted unit, `run.py` gives the binary the unit's files plus every
local file they import, transitively (relative TS/JS imports and requires;
Rust `mod x;` and `#[path]`), with `--no-declaration --no-config --format json
--fail-on never`. One unit at a time, sequentially.

A unit is **readable** when the tool checked at least one test in it
(`summary.tests > 0`, which counts only tests that drive a recognised harness).
An unreadable unit is reported under exactly one reason, by its kind:

| Kind | Reason reported |
|---|---|
| binary-main | Rust binary with `fn main` (PoC framework or CTF client) |
| ctf-solve-program | on-chain attacker program, not a test |
| rust-unit-test | Rust unit test that drives no Solana harness |
| script | TS/JS script, not a test (CTF client, replaced deploy script, simulation) |
| test-fragment | fragment pasted in an issue (no test wrapper, or no imports) |
| test-file | does not parse, when the tool reports a parse error; otherwise "test drives no harness the tool recognises" |

RQ1 is reported per tier, per kind and pooled, with Wilson 95% intervals, and
is a headline number. A unit is never dropped because the tool cannot read it.

## Label rubric

The P1 rubric (`eval/solana/PREREGISTRATION.md`) applies unchanged, including
the rule-specific guidance for RG02, RG03, RG04, RG08, RG09, RG10 and RG11.
Additions, written before any label, because the population is PoCs:

1. **The claim is the PoC's.** A flag is judged against what the PoC claims:
   the test title and comments, and for J the issue text. A flag on a test
   that is not an exploit claim (a setup test, or a control/secure-path test
   in a mixed exploit file that shows the attack failing) is judged by the
   P1 rubric's feature-test clause: TP only for RG08/RG09, whose defect
   (nothing is asserted / the assertion cannot fail) holds whatever the claim.
2. **Challenge setup is setup.** In a CTF or teaching PoC, a step signed by
   the challenge's admin to create the vulnerable state the challenge hands
   the attacker is setup, not the exploit path (RG02/RG03 FP), unless the
   claimed impact depends on that signature.
3. **RG01 attacker-holds-privilege** (not in P1's rubric; it never fired):
   TP when the attacker identity is given a protocol role, made the authority,
   or created as the privileged account, and the claimed impact depends on
   it. FP when the "role" is the attacker's own account by design (the
   attacker initialises its own position) or the grant is challenge setup the
   challenge statement gives the player.
4. **RG05 code-replacement / RG06 balance-injection** (not in P1's rubric):
   TP when the PoC replaces a program the protocol uses / writes lamports or
   token balance into a protocol account in the exploit path, outside the
   program. FP when it is the test's own fixture or a faucet-style funding of
   the attacker's own account.
5. `unclear` is allowed and reported beside precision, never counted.

The label set is one primary reader's judgement, AI-assisted (disclosed).

## Agreement

One blinded second pass by a single read-only subagent that sees the rubric,
the flag and its source, but no first-pass label: a sample of 30 labelled
flags drawn by `sha256("rg-solana-poc-agreement-2026-11-25:" + flag_id)`
across both splits (all of them if fewer than 30 are labelled). Raw agreement
with its Wilson interval and Cohen's kappa are reported with the count;
disagreements are adjudicated in writing in `agreement.json`, and both the raw
and the adjudicated figures are published.

## Analysis plan

- **RQ1 coverage**: readable units / units, per tier, per kind, pooled; the
  unreadable units by reason, as counts.
- **RQ2 precision on PoCs**: per rule and language on the held-out split,
  TP/(TP+FP) with Wilson 95% intervals (z = 1.959964), pooled over J and X;
  per-tier rows beside it; dev and all-labelled tables beside the held-out
  table; a pooled all-rules number per split. Rules that raise nothing are
  `unmeasured`, never 100%. The minimum-n rule applies.
- **RQ3 flag vs judged outcome (exploratory)**: on J units with a
  `platform-report` outcome, a unit is *flagged* when it has at least one
  active flag. A 2×2 table (flagged × valid/invalid), Fisher's exact two-sided
  p, and the risk difference P(flagged | valid) − P(flagged | invalid) with a
  Newcombe hybrid-score 95% interval. A second row restricted to readable
  units, and a sensitivity row adding the `self-reported` units. With about
  ten units this is exploratory: no predictive claim is made from it.
- **RQ4 negative control**: for the exploit/secure pairs, the share of units
  flagged and the flags per checked test, on the exploit side and the secure
  side, with Wilson intervals on the unit shares. The expectation stated in
  advance: the secure side's flag rate does not exceed the exploit side's.
  Per-rule counts are listed.
- **RQ5 comparison with P1**: P1's held-out test-suite cells (n and interval as
  published) beside this study's held-out PoC cells, rule by rule, with the
  population difference stated. The two populations are never pooled.

## Expected outcome, stated in advance

The corpus is about a hundred units, most of them teaching and CTF material,
and the judged tier is about ten. Per-rule intervals will be wide. RQ1
coverage and pooled numbers carry the result; the judged tier is too small for
anything but description. If the tool reads few judged PoCs, that is a
finding, not a reason to pad the corpus with test suites.

## Known deviations from the operator's prompt, decided before running

1. The prompt assumed Sherlock folder = valid and root = invalid. That is not
   how these repositories work (see the outcome rule above); I used the
   published report.
2. Several sources the prompt listed as exploit-by-construction are unsolved
   templates: Ackee bootcamp CTF levels 1-5, Ackee Breakpoint 2023
   exploit-0..3, the OtterSec `cut-and-run` solve test and `attachments/`
   solve programs, Neodyme workshop `pocs/src/bin/level0-4.rs` (empty
   `hack()`), and `sealevel-attacks/tests/raw.ts`. They are excluded as
   templates, not counted as unreadable PoCs, because they contain no exploit.
   This also corrects P1's stated reason for excluding the Neodyme workshop.
3. The prompt cited Pump Science H-01's PoC as a Rust unit test; in the public
   report the Rust unit test belongs to H-02. H-01 has no coded PoC.
4. `web3-master/audit-reports` states outcomes only in its repository
   description; its two Solana PoCs are tier J with `self-reported` outcomes
   and are kept out of RQ3's primary analysis.

## What this cannot claim

- Not recall: a defect the rules do not flag is not counted.
- Not a property of later versions; one tree at one commit.
- Not predictive of judging outcomes (RQ3 is exploratory).
- Not independent human validation: one AI-assisted reader, one AI second pass.
