# Solana PoC corpus v2 (P1-C fresh corpus)

Collected 2026-11-25, **before any change to the tool's `src/`**, for the P1-C
re-measurement. This is the out-of-sample headline corpus: it must not be run
through realism-gate, opened or labelled until the code freeze (the freeze
commit and the preregistration are recorded in `PREREGISTRATION.md` at that
point). Kind is recorded from the file alone at collection time.

The P1 corpus (`eval/solana-poc/`) stays unchanged and is re-measured
separately as an **in-sample** set.

## What is here

| File | What it is |
|---|---|
| `corpus.json` | Every git source pinned by commit and by `sha256(git ls-tree -r <commit>)`; every Arena issue pinned by `sha256` of its markdown body; every rendered report page pinned by file `sha256`. |
| `inventory.py` / `inventory.json` | The unit decisions, in `ARENA_J`, `ARENA_X`, `JITO` and `PATH_UNITS`, with tier, kind, outcome and file hashes; exclusions with reasons; the code-search screen; dead ends. |
| `fetch.sh` | Fetches and verifies every pin. `--check` verifies what is present offline. |
| `extract.py` | Writes the named PoC blocks from issues and report pages into a gitignored work tree. |
| `SEARCH_LOG.md` | Every search query, its date, hits screened, units taken and dead ends. |
| `PREREGISTRATION.md` | Written at the code freeze, before the first run. |
| `RESULTS.md` | The frozen run: RQ1 coverage and the in-sample 1.0.4 vs 1.2.0 comparison. |

Checkouts live under the gitignored `.corpus/solana-poc-v2/`, extracted blocks
under `.corpus/solana-poc-v2-work/`. No third-party source or issue text is
committed: the repository holds pointers, hashes, labels and one-line reasons.

## Tiers

- **J, judged (28 units).** An outcome is published:
  - Solana Audit Arena: 25 submissions the maintainer labelled (`Valid` /
    `invalid` / `duplicate` plus severity), with the week's final report
    committed in the same repository under `reports/`.
  - Immunefi Audit Competition | Jito Restaking: 3 findings whose published
    report contains a test block.
- **X, exploit-by-construction (53 units).** No published outcome: Arena
  submissions whose week is not judged yet, CTF solve binaries, teaching
  exploit tests, and public reproductions of known bugs.
- **NC, negative control.** None: the fresh sources contain no new
  exploit/secure pair. This is stated rather than manufactured.

## Kinds (recorded at collection, before the new readers existed)

`test-file`, `test-fragment`, `rust-unit-test`, `script`, `binary-main`,
`ctf-solve-program` — the same vocabulary as P1. Examples:

- Arena issues: the extracted block is a Rust `#[test]`/`#[tokio::test]`
  (FrankSol), an Anchor TypeScript `describe()` file (x402-Meridian), a bare
  `it()` fragment, or a standalone Rust model (issue 243).
- Merovingio/Jet/Halborn/Loopscale: Rust `fn main` binaries.
- Francis: Mollusk `#[test]` files; 1inch/demo/AMM: `solana-program-test` or
  LiteSVM `#[test]` files.

## Selection (reproducible)

Seed `rg-solana-poc-v2-2026-11-25`.

- **Arena J:** the two issues the maintainer marked `invalid` that carry a test
  block are always kept (RQ3 needs both classes); the remaining labelled issues
  with a test block are taken in ascending `sha256(seed|issue)` order to 25.
- **Arena X:** the 20 lowest `sha256(seed|issue)` among unlabelled issues with
  a test block; issue 278 dropped after its block turned out to be a
  compute-unit benchmark, not an exploit (recorded in the exclusions).
- **Path units:** every exploit PoC file in the included repositories, minus
  the screened-out files listed in `inventory.json`.

Dedupe is by the normalised `sha256` of the first file (CRLF→LF, trailing
spaces stripped, surrounding blank lines dropped). A unit whose first file
matches an earlier unit is kept with `duplicate_of` and is not counted. The
inventory has **81 counted units (28 J, 53 X)**, 0 duplicates.

## Readiness checks

```bash
bash eval/solana-poc-v2/fetch.sh --check
python eval/solana-poc-v2/extract.py
python eval/solana-poc-v2/inventory.py --check
```
