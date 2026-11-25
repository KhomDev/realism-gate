# Preregistration: Rust and Anchor TypeScript rule precision (P1-A)

Frozen 2026-11-24, before any label was written.

## What is frozen

- Rule logic: `realism-gate` 1.0.4 at commit
  `6019773976706e42e11ceefc2ee1b03496f2df54` (the `main` merge this branch starts
  from). No rule implementation, catalog severity, or predicate may change
  during the measurement. Measurement metadata, harness code, labels, docs and
  output-format additions are allowed; a rule change invalidates this freeze
  and needs a new dated preregistration.
- Corpus pins: `eval/solana/corpus.json`. Every repository is pinned by commit
  and by the sha256 of `git ls-tree -r <commit> -- <paths>`, so the pin covers
  every file's blob, not the checkout's line endings.
- Seed: `rg-solana-2026-11-24`.
- Sampling: at most 30 flags per rule per corpus, ordered by
  `sha256(seed + ':' + flag_id)`; every flag is taken when the rule raised 30 or
  fewer on that corpus. The sample files under `eval/solana/samples/` are
  committed.
- Split: a sampled flag is `dev` when
  `int(sha256(seed + '|' + corpus + '|' + flag_id)[:8], 16)` is even, and
  `heldout` otherwise. The split is computed from the committed flag id, so it
  is reproducible without the labels. The headline table is the **held-out**
  split. The all-sample and dev numbers are published beside it for
  transparency, not as the headline.
- Label rubric: below. It is frozen with this file.

## Corpus: what population this measures

The four licensed corpora are public Rust and Anchor TypeScript **test suites**:
`coral-xyz/anchor`, `solana-developers/program-examples`, `LiteSVM/litesvm`, and
five `solana-labs/solana-program-library` test directories. They are not a
corpus of judged PoCs or public incident reproductions. The intended PoC corpus
does not exist publicly in a form this tool reads: the Neodyme CTF and workshop
repositories ship exploit **binaries** (`pocs/src/bin/*.rs`), not test functions,
and `coral-xyz/sealevel-attacks` has no test suite the tool recognizes. Those
three are recorded as excluded in `corpus.json` and contribute no flags.

**Consequence, stated before measuring:** every number from this corpus is a
precision measurement on public test suites, not on submission PoCs. Feature
tests often sign as an admin because that is what the test is demonstrating;
the rules are written for a PoC that claims an exploit. A low number here is
evidence about false positives when the rules run over public code, and it is
not evidence about the PoC population. The report says this next to every
table, and no release note or post may quote the number as PoC precision.

## Label rubric

A label is one of `true-positive`, `false-positive`, or `unclear`, with a
one-line reason. The unit is one flag, not the test file. The reader has the
flagged source, the surrounding lines, the rule page under `docs/rules/`, and
the tool's evidence string.

- **true-positive**: the pattern the rule describes is at the flagged line, and
  the rule's stated consequence holds in context. The test is claiming, or
  demonstrating as an exploit, the behaviour the rule warns about; a reviewer
  of that claim would need the privilege/injection/assertion defect justified.
- **false-positive**: the stated consequence does not hold, or the context is a
  feature/configuration test rather than an exploit claim: the "privileged"
  key is the attacker's own, the deployer call is ordinary local deployment, the
  written account is the test's own fixture, the assertion exists through a
  helper the rule cannot see, the starting balance was set by the test, or the
  cluster is a local validator rather than a live fork.
- **unclear**: the source alone does not settle it (for example a helper in
  another crate that the reader cannot see). Reported beside precision, never
  counted as TP or FP.

Rule-specific guidance, applied in this order:

1. **RG02 privileged-step-in-exploit.** TP when the flagged call is in the
   exploit path (not setup), the signer is a protocol role by name or by a
   grant the test records, and the claimed impact depends on that signature.
   FP when the signer is the attacker's own key, a test wallet, or the call is
   setup/configuration in a feature test. Unclear when the role grant is hidden
   in another file the reader cannot see.
2. **RG03 deployer-is-attacker.** TP when the deployer calls a function that
   the test shows is access-controlled, in the path the test presents as the
   attack. FP for ordinary local `deposit`/`initialize` calls and feature tests.
3. **RG04 storage-write.** TP when the test writes a protocol account's state
   directly (`vm.store`, `stdstore`, `set_account`, `setAccount`) in the path
   that is presented as the exploit, bypassing the program. FP when the account
   is the test's own fixture or the write is an explicit setup step whose effect
   is later reached through the program.
4. **RG08 no-assertion.** TP when the test makes no assertion anywhere in its
   body or in a helper the tool inlines. FP when an assertion exists through a
   recognized helper or the test is a setup/helper body that is not a test.
5. **RG09 tautological-assertion.** TP when the assertion cannot fail for the
   values in scope. FP when the values are dynamic and the assertion can fail.
6. **RG10 no-baseline.** TP when a gain is asserted after the attack with no
   before-value and the starting balance was not set by the test. FP when the
   test set the starting balance (`deal`, `set_account`, a mint) or asserts a
   difference.
7. **RG11 unpinned-fork.** TP when the test connects to a live cluster/fork
   without a pinned slot. FP when the connection is a local validator, a
   test-only client, or a pinned slot is set where the tool cannot see it.
8. Rules that raise nothing on the sampled corpora are reported as
   **unmeasured**, never 100%.

## Measurement

- Precision = TP / (TP + FP), Wilson 95% interval (z = 1.959964); `unclear` is
  reported beside the ratio and left out of it.
- Per rule and per language (Rust, TypeScript) on the held-out split, pooled
  over the four corpora. Per-corpus rows are published too.
- A rule with no labelled flag on the held-out split is `unmeasured` with the
  reason ("no flags raised" or "flags only in dev"). A rule that raised nothing
  anywhere is not 100%; it is unmeasured.
- The all-sample and dev tables are published beside the held-out table.
- The label set is one primary reader's judgement (AI-assisted, disclosed),
  with a blinded second pass on a seeded 20-flag sample; agreement is published
  as a count and a rate with its interval. This is not independent human
  validation.

## Deviation, 2026-11-24 (after the held-out labels, before any dev label)

Only the held-out split is labelled. The 70 dev-sampled flags are left
unlabelled, so the all-sample and dev tables promised above are **not**
published. The reason is not convenience: labelling the dev flags after the
held-out result is known would add no power to the prespecified headline and
would expose the labels to the result. The held-out table is the measurement;
`eval/solana/RESULTS.md` states the dev count and the gap. A later measurement
that wants the dev table should label the dev split before opening a new
held-out corpus, under a new dated preregistration.

## What this cannot claim

- It is not PoC precision, because the corpus is test suites.
- It is not recall: flags that never fired are unmeasured, not absent.
- It is not a property of future versions; it is one tree at one commit.
- It does not establish that a rule is safe to ship at error severity. The
  existing R3 floor applies only to the Solidity labels it was computed on.
