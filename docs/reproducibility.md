# Reproducing realism-gate's numbers

Every command runs from the repository root. Toolchain: Rust 1.95 or later (`rust-version` in `Cargo.toml`; CI pins its toolchains in `.github/workflows/`). Python 3.12 runs the `eval/` scripts with the standard library only (`python -I`).

## Build and tests
```bash
cargo build --release --locked
cargo test --workspace --locked                      # unit, property, adversarial, integration, golden
REALISM_GATE_BLESS=1 cargo test --test docs          # only when a doc block is meant to change
```
The test map is in [`testing.md`](testing.md).

## Benchmark (DeFiHackLabs, in-sample)
```bash
bash scripts/bench-corpus.sh          # print the summary
bash scripts/bench-corpus.sh --check  # compare with bench/defihacklabs.json (658 findings, raised vs shipped)
```

## Corpora (DEV only until the held-out rounds)
```bash
python -I eval/solidity-poc-v1/fetch.py --cache <cache>               # rebuild the pinned cache
python -I eval/solidity-poc-v1/run.py --check --split DEV --cache <cache>   # verify every pin offline
python -I eval/solidity-poc-v1/run.py target/release/realism-gate --version <v> --split DEV \
    --cache <cache> --out-dir tmp/dev-solidity-poc-v1
python -I eval/p2r_packet.py --corpus solidity-v1 --run-dir tmp/dev-solidity-poc-v1   # blind packet
python -I eval/solana-poc-v3/measure.py --corpus solidity-v1 --flags <run>/flags/<v>.jsonl \
    --labels <labels.jsonl> --measured measured.json --results results.json
```
The same commands apply to `solana-poc-v3/v4/v5` and `solidity-poc-v2`. Sealed splits need
`--round`, `--version` and a matching `--binary-sha256`. Do not run them before the round freeze.

## Pinned inputs
The inventory sha256 values, the golden manifest hash and the source commit are recorded in
`eval/P2R2_PREREGISTRATION_POST_MAIN.md`. A run is reproduced when its `meta.json` binary sha256
and its `flags/<v>.jsonl` match.