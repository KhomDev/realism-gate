# eval/: how often each rule is right

Everything here exists so that the precision figures in `docs/precision.md`,
the changelog and the release notes can be re-derived by anyone. The summary
of what they say is in `docs/precision.md`; this file is how they are made.

## Inputs

| File | What it is | Written by |
|---|---|---|
| `corpus.json` | three corpora, each pinned by commit and by the sha256 of its `git ls-tree` listing | hand, hashes by `corpus.py --write` |
| `seed.txt` | the sampling seed | hand, once |
| `samples/<corpus>.jsonl` | the flags drawn for labelling | `sample.py` |
| `labels/v0.2.0.jsonl` | one label per sampled flag: `true-positive`, `false-positive` or `unclear`, with a one-line reason | a person reading the flagged source |

The corpora:

- **in_sample**: DeFiHackLabs `src/test` at the commit `docs/benchmark.md`
  uses. The 0.1.0 rules were written against it.
- **later**: the `.sol` files added to DeFiHackLabs after that commit, up to
  upstream main on 2026-11-04 (70 files). They were labelled before the
  v0.2.0 fix was written, but two of the patterns it fixes were first seen
  here, so this is a second tuning set, not a held-out one.
- **fresh**: DeFiVulnLabs `src/test` (57 files), a different style of PoC
  (local deployments, not fork reproductions). It was first opened after the
  rule logic was frozen at commit `e9bea03` and was not used to change it.
  The repository states no licence, so it is fetched and never vendored, and
  the committed flag files for it leave out the source text.

## Outputs

| File | Written by |
|---|---|
| `flags/<version>-<corpus>.jsonl` | `flags.py`: every active finding, with a stable id |
| `results/precision.json`, `results/precision.md` | `precision.py` |
| `results/rules-v0.2.0.json` | `realism-gate rules --format json` |
| `perf/*` | `scripts/measure-perf.py`, `scripts/measure-install.sh`, `scripts/release-assets.sh` |

## Definitions

- A **flag** is an active (not allow-silenced) finding.
- Its **id** is the first 16 hex digits of sha256(rule | file | line |
  column | evidence). It ignores the message and severity, so one label
  covers the same flag in both versions.
- **Sample**: per corpus and rule, flags ordered by sha256(seed:id); the
  first 30, or all of them if the rule raised 30 or fewer. Only RG08 (no
  assertion) raised more than 30 anywhere.
- **true-positive**: the pattern the rule describes is at the flagged line
  and the rule's stated consequence holds in context; a reviewer of this PoC
  would need it justified.
- **false-positive**: the stated consequence does not hold, e.g. the
  "replaced" code is the attacker's own contract, or the "unknown" starting
  balance was set by the test.
- **unclear**: the source alone does not settle it.
- **precision** = TP / (TP + FP), with a Wilson 95% interval. `unclear` is
  reported beside it, not counted.

## Reproduce

```bash
cargo build --release
gh release download v0.1.0 --repo KhomDev/realism-gate \
  --pattern 'realism-gate-0.1.0-x86_64-unknown-linux-gnu.tar.gz' --pattern SHA256SUMS
sha256sum -c SHA256SUMS --ignore-missing
tar xzf realism-gate-0.1.0-x86_64-unknown-linux-gnu.tar.gz
bash eval/run.sh realism-gate-0.1.0-x86_64-unknown-linux-gnu/realism-gate target/release/realism-gate --check
```

CI runs the same check on every pull request (`.github/workflows/ci.yml`,
job `eval`).

To look at what a label is about: `python eval/show.py in_sample
eval/samples/in_sample.jsonl --id <id>`.

## Who labelled

Every label in `labels/v0.2.0.jsonl` was written by an AI agent (Claude Opus
5.5) working on this release, from the source around each flag, and is marked
`pending operator review`. They are one reader's judgement, not ground truth;
a second reader may disagree on some, especially RG05 and RG10. The numbers
are only as good as the labels, which is why the labels are published.
