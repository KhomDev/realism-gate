# eval/solana-poc: realism-gate on real Solana PoCs

**Population: public Solana PoCs.** Judged contest PoCs (Sherlock, Code4rena),
two self-reported bounty PoCs, CTF solves, teaching exploit/secure pairs and a
GitHub code-search screen; 45 pinned sources. This corrects the P1 statement
that no public PoC corpus exists (see `../solana/README.md`). The measurement is
preregistered in `PREREGISTRATION.md`; the result is `RESULTS.md`.

## Files

| Path | What it is |
|---|---|
| `corpus.json` | every source pinned by repo, commit, sparse paths and the sha256 of its `git ls-tree -r` listing; license marked (most have none: pointer-only inclusion, operator decision 2026-11-25) |
| `fetch.sh` | fetches the pins into `.corpus/solana-poc/` (gitignored) one at a time and verifies them; `--check` verifies only |
| `extract.py` | writes PoCs that live in an issue or a report page, verbatim, into `.corpus/solana-poc-work/` (gitignored) |
| `inventory.py` / `inventory.json` | every candidate with tier, kind, include/exclude and reason, judged outcome and its source, dedupe group, normalised hashes |
| `run.py` | runs a binary over each counted unit, one at a time: `runs/`, `flags/`, `sample.jsonl` |
| `context.py` | prints a flag with source for a label reader (split hidden); `--agreement-sample` builds the blinded sample |
| `labels/heldout.jsonl`, `labels/dev.jsonl` | every sampled flag labelled TP/FP/unclear with a one-line reason |
| `agreement.json`, `agreement/second-pass.jsonl` | the blinded second pass, the disagreement and its adjudication |
| `measure.py` | RQ1-RQ5 with Wilson intervals, Fisher's exact test and a Newcombe interval: `results.json`, `RESULTS.md`; `--check` re-derives both and the docs block offline |

No third-party text is committed: pointers, hashes, flag ids and labels only.

## Reproduce

```bash
bash eval/solana-poc/fetch.sh          # network: the pinned sources and one report page
python eval/solana-poc/extract.py
python eval/solana-poc/inventory.py --check
python eval/solana-poc/run.py target/release/realism-gate   # realism-gate 1.0.4
python eval/solana-poc/measure.py --check                   # offline
```

`measure.py --check` needs only the committed files. The run needs the 1.0.4
binary (commit `6019773`); a later version produces different flags.

## What it cannot claim

- Not recall, not a property of later versions, not predictive of judging outcomes.
- One AI-assisted primary reader and one blinded AI second pass; not independent human validation.
- Flags cluster within units and sources; the intervals assume independence.
