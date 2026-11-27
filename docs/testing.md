# Testing: the test map and how to run it

The suite has four layers, plus mutation testing and fuzzing. Every command below runs from the repository root.

```bash
cargo test --workspace --locked                      # all four layers (about 600 tests)
cargo test --test floor_properties                   # one file
REALISM_GATE_BLESS=1 cargo test --test docs          # re-bless generated doc blocks (only on purpose)
cargo +nightly fuzz run solidity -- -max_total_time=30   # one fuzz target, briefly
```

## Layers

| Layer | Where | What it pins |
|---|---|---|
| **Unit** | `#[test]` in `src/` and `crates/*/src/` | Every rule, the IR, the renderers (text, JSON, SARIF), `measured` (floor, cells, records), parsers, the program model |
| **Property** (proptest) | `tests/properties.rs`, `tests/floor_properties.rs`, `tests/f1_binding.rs`, `tests/exploit_model.rs`, `tests/f3_oxc_guard.rs` | The report does not depend on file order. An unrelated test never changes other findings. Only a live literal branch reaches the trace. Every report validates as SARIF. **Severity floor:** Wilson bounds stay in [0, 1] and bracket p; the level is monotone in TP (never down), and turning a TP into an FP never raises it (an added FP alone can, by lifting a cell to the minimum n of 10; pinned by a test); a cell that is not fresh never lifts a rule; the minimum n holds; the cap never raises and is idempotent; an unknown rule ships `note`. Binding and parser idempotence. |
| **Adversarial** | `tests/adversarial.rs`, `tests/security.rs`, `tests/verify_fix_paths.rs`, `tests/verify_fix_limits.rs`, `tests/verify_fix_copy_limits.rs`, `tests/verify_fix_ffi.rs`, `tests/fuzz_corpus.rs`; `fuzz/` (6 targets: solidity, rust, typescript, move, claims, declaration) | Malformed, huge and unicode sources; path traversal and symlinks in `verify-fix`; copy and size limits; FFI and cheatcode abuse; replay of the committed fuzz seeds on every CI run. realism-gate opens no network connection, so SSRF, redirect and timeout cases do not apply here; they are in priorart's `fetch` tests. |
| **Integration** | `tests/cli.rs`, `tests/golden.rs` (117 hash-pinned cases, `tests/golden/manifest.json`), `tests/plugin.rs`, `tests/mcp_stdio.rs`, `tests/schema.rs`, `tests/docs.rs`, `tests/conformance*.rs`, `tests/rules.rs`, `tests/options.rs`, `tests/precision_floor.rs`, `tests/claims.rs`, `tests/cite.rs`, `tests/review.rs`, language suites (`rust.rs`, `typescript.rs`, `move_lang.rs`, `f2_solidity_model.rs`) | End-to-end CLI and plugin runs against goldens, the output schema, generated docs, and the shipped severity table against an independent re-implementation of the floor |

## Gates in CI
- `ci.yml` runs fmt, clippy `-D warnings` and the full suite on every push and PR, on Linux, macOS and Windows.
- `fuzz.yml` runs each target for **30 s on every PR and integration push**, and for 5 min weekly.
- `mutants.yml` runs weekly and on dispatch. `files` limits it to some files and `base` to changed
  lines (`--in-diff`). The survivors and the documented equivalents are in
  [`test-quality.md`](test-quality.md).
- `coverage.yml` runs on dispatch and reports the numbers. It does not fail on a coverage figure, on
  purpose (see `test-quality.md`).

## Counts, before and after P2R-2
`#[test]` functions declared in the tree. The proptest functions are a subset of them.

| | `main` (1.4.0) | `p2/integration` + this branch |
|---|---:|---:|
| Unit (`src/`, `crates/`) | 183 | 321 |
| Integration-level files (`tests/`) | 248 | 360 |
| Property functions | 4 | 16 |
| Fuzz targets | 6 | 6 (now with a PR smoke) |
| Executed by `cargo test --workspace` | n/a | 600 passed, 0 failed, 2 ignored |

Coverage (cargo llvm-cov, regions / functions / lines): 88.21 / 89.50 / 89.09% at `d747bed`. The
figure for this branch is in the PR body.