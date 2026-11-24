# eval/solana: Rust and Anchor TypeScript rule precision

**Population: public test suites, not judged PoCs.** This harness pins four
public Rust/Anchor TypeScript test suites, runs realism-gate over them, labels
a sample of the flags, and measures per-rule precision on a held-out split.

The intended corpus for P1 was public PoCs and incident reproductions. It does
not exist publicly in a form this tool reads: the Neodyme CTF and workshop
repositories ship exploit **binaries** (`pocs/src/bin/*.rs`), not test
functions, and `coral-xyz/sealevel-attacks` has no test suite the tool
recognizes. Those repositories are recorded as excluded in `corpus.json`. The
numbers here are therefore precision on public test suites; they are not PoC
precision and must not be quoted as such.

## Files

| Path | What it is |
|---|---|
| `corpus.json` | the four corpora, each pinned by repo, commit, paths and the sha256 of the `git ls-tree -r` listing |
| `fetch.sh` | fetches the pins into `.corpus/solana/` (gitignored) and verifies the listing hashes |
| `flags.py` | runs a binary and writes every active flag (no evidence text) plus the per-rule sample |
| `PREREGISTRATION.md` | the freeze: rule commit, seed, split, rubric, sampling |
| `samples/<corpus>.jsonl` | the sampled flag ids (at most 30 per rule per corpus) |
| `flags/v1.0.4-<corpus>.jsonl` | every active flag, with the evidence sha256; the evidence itself is not committed |
| `labels/v1.0.4-heldout.jsonl` | the held-out labels: TP/FP/unclear and a one-line reason |
| `context.py` | prints a flag with source around it for a label reader; `--agreement-sample` makes a blinded sample |
| `measure.py` | Wilson 95% intervals, `results.json`, `RESULTS.md`, `measured.json` |
| `RESULTS.md` | the readable table |
| `measured.json` | the compact file embedded in the binary for `rules` and `explain` |
| `agreement.json` | the blinded second-pass agreement, when run |

## Reproduce

```bash
bash eval/solana/fetch.sh
cargo build --release
python eval/solana/flags.py target/release/realism-gate
python eval/solana/measure.py
```

The labels are committed, so `measure.py` needs no network. `fetch.sh` is the
only network step. A reader who wants to re-label can use
`python eval/solana/context.py --split heldout > heldout.txt` and compare.

## What the measurement covers

- 174 flags raised by realism-gate 1.0.4 on the four corpora.
- 133 sampled (at most 30 per rule per corpus, by `sha256(seed:flag_id)`).
- 63 held-out labels; 70 dev-sampled flags are unlabelled.
- Rules with no held-out label are `unmeasured`, never 100%.
- One reader wrote the labels (AI-assisted, disclosed). The blinded second pass
  is a seeded 20-flag sample; its agreement is in `agreement.json`.

## What it cannot claim

- It is not PoC precision; the population is public test suites.
- It is not recall; a rule that raised nothing is unmeasured.
- It is not a property of future versions; it is one tree at one commit.
- It does not establish that a rule is safe to ship at error severity.
