# realism-gate in OSS-Fuzz

Local copy of the OSS-Fuzz project configuration for realism-gate's six
cargo-fuzz targets. It follows the layout of
`google/oss-fuzz/projects/<name>/` so it can be reviewed here, but **no
upstream pull request was opened and nothing was submitted**. The placeholder
`primary_contact` in `project.yaml` must be replaced with a deliverable
maintainer address before any upstream submission.

## What is built

All six targets share one property: **the parser never panics on any input**.
They read arbitrary bytes (lossily decoded as UTF-8) through the same functions
`cargo test` exercises.

| Target | Source | Parses |
|---|---|---|
| `solidity` | `fuzz/fuzz_targets/solidity.rs` | Foundry `.t.sol` (solar) |
| `rust` | `fuzz/fuzz_targets/rust.rs` | Rust/Anchor `.rs` (syn) |
| `typescript` | `fuzz/fuzz_targets/typescript.rs` | Anchor `.ts` (oxc) |
| `move` | `fuzz/fuzz_targets/move.rs` | Move `#[test]` sources |
| `claims` | `fuzz/fuzz_targets/claims.rs` | `claims.yaml`, front matter, prose heuristics |
| `declaration` | `fuzz/fuzz_targets/declaration.rs` | `realism.yaml` |

The build uses nightly, AddressSanitizer, and libFuzzer. `sanitizers` lists only
`address`: rustc has no `undefined` sanitizer, so there is no Rust UBSan build
to configure. The same seeds live under `fuzz/corpus/<target>/` and are also
replayed by `tests/fuzz_corpus.rs` on every ordinary `cargo test`.

## Layout

- `Dockerfile` — `base-builder-rust`, installs nightly-2026-11-01 (the base
  image's 2025-09-05 nightly is older than the crate's `rust-version = "1.95"`),
  clones `KhomDev/realism-gate`, and checks out the pinned `RG_COMMIT` (a full
  40-character SHA).
- `build.sh` — resolves the fuzz workspace from its lockfile, runs
  `cargo fuzz build -O -s address`, copies each binary to `$OUT/<target>`, and
  zips the seed corpus to `$OUT/<target>_seed_corpus.zip`.
- `project.yaml` — language `rust`, sanitizer `address`, engine `libfuzzer`.

## Reproduce locally

The same run as the weekly `fuzz.yml` job, without OSS-Fuzz:

```bash
cd fuzz
cargo +nightly fuzz run solidity -- -max_total_time=300
```

Rebuild exactly what `build.sh` produces, inside the OSS-Fuzz image:

```bash
# From the oss-fuzz checkout, with this directory as projects/realism-gate.
python3 infra/helper.py build_image realism-gate
python3 infra/helper.py build_fuzzers --sanitizer address realism-gate
python3 infra/helper.py check_build realism-gate
```

Run a built target, where `<corpus>` is a directory of inputs:

```bash
python3 infra/helper.py run_fuzzer realism-gate solidity -- -max_total_time=60 <corpus>
```

## What was not run here

Docker Engine was not running on the Windows host used to write this
configuration, so the image was not built and no OSS-Fuzz `check_build` or
`run_fuzzer` was observed. `build.sh` was syntax-checked with `bash -n`, and
`project.yaml` was parsed as YAML. A build and a fuzz run remain to be done on
a host with a working Docker daemon and network access to
`gcr.io/oss-fuzz-base/`; the commands above are the ones to run.

## Maintenance

- Bump `RG_COMMIT` in a commit that says why. The commit must be on
  `main`/`release/**` and publicly fetchable, because the Dockerfile clones
  from GitHub. At pinned 1.0.4 (`6019773`), `fuzz/Cargo.lock` still names the
  root package `1.0.1`, so `build.sh` cannot pass `--locked`; the lockfile is
  fixed in the trust-infrastructure commit. After the pin moves to a commit
  that contains that fix, change the `cargo metadata` line to
  `cargo fetch --locked`.
- A new fuzz target needs a `[[bin]]` entry in `fuzz/Cargo.toml`, a file in
  `fuzz/fuzz_targets/`, an entry in `TARGETS` and the seed corpus in
  `fuzz/src/lib.rs` / `fuzz/corpus/`, and its name added to `targets` in
  `build.sh`.
