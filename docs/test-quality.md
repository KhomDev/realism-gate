# Test quality

How well the test suite exercises the rules, measured on `main` after the
adversarial, fuzz and property tests (2026-11-19). These numbers are a snapshot
for reviewers. CI does not gate on them.

The coverage numbers below are the 1.0.3 snapshot: a 1.0.4 run is blocked on
the Windows host this page was last updated from (see below), and no number is
claimed without a run that produced it. The mutation measurement is scoped and
partial; its exact scope and the reason are recorded with it.

## Commands

```bash
# Line and region coverage, on Linux (needs the llvm-tools-preview component)
cargo install cargo-llvm-cov --locked
cargo +1.95.0 llvm-cov --locked --workspace --summary-only --no-cfg-coverage

# Mutation testing, the scoped measurement below. RUSTUP_TOOLCHAIN is explicit
# because cargo-mutants starts its own cargo processes.
cargo install cargo-mutants --locked
RUSTUP_TOOLCHAIN=1.95.0-x86_64-pc-windows-gnu \
  cargo +1.95.0-x86_64-pc-windows-gnu mutants \
  --file src/claims/init.rs --file src/claims/extract.rs \
  --timeout 180 -j 2
```

`.github/workflows/coverage.yml` runs the coverage command on ubuntu-24.04 with
rustc 1.95.0 and writes the summary to the run's job summary; that run is the
source for a 1.0.4 number. The weekly `.github/workflows/mutants.yml` covers
the rules, claims, bind and verify-fix files.

## Line coverage

1.0.3 snapshot, 2026-11-19 (`cargo llvm-cov --summary-only`):

| Scope | Lines | Missed | Line coverage |
|---|---|---|---|
| Whole crate (`TOTAL`) | 13 250 | 1 621 | 87.77% |

The least-covered files of the rule code (line coverage):

| File | Line coverage |
|---|---|
| `src/move_lang/rules.rs` | 74.17% |
| `src/rules/injection.rs` | 83.09% |
| `src/rules/privilege.rs` | 89.62% |
| `src/rules/evidence.rs` | 93.27% |
| `src/rules/realism.rs` | 94.66% |
| `src/solana/rules.rs` | 93.66% |

The bind and claims code is also thin: `src/bind/mod.rs` at 77.58% is the
least covered file in those two directories, and `src/bind/facts.rs` is at
89.47%. These are 1.0.3 numbers; 1.0.4 adds tests for the MCP stdio server
(`tests/mcp_stdio.rs`), the ffi guard (`tests/verify_fix_ffi.rs`), the render
sanitizer and the input caps (`tests/adversarial.rs`).

**1.0.4 is not measured.** On 2026-11-24 the pinned
`1.95.0-x86_64-pc-windows-gnu` toolchain failed before running a test:

```text
error[E0463]: can't find crate for `profiler_builtins`
  = note: the compiler may have been built without the profiler runtime
```

`rust-std-x86_64-pc-windows-gnu` ships no profiler runtime. The MSVC toolchain
does ship one, but this host has no Visual C++ linker or CRT (`link.exe`
resolves to the MSYS coreutils `link`; there is no `msvcrt.lib` or
`vcruntime.lib`). Docker Desktop's Linux engine did not start (the
`dockerDesktopLinuxEngine` pipe was absent) and WSL has no distribution, so
there was no Linux container either. Run `.github/workflows/coverage.yml` and
quote its summary; until then this page claims no 1.0.4 coverage number.

## Mutation testing

`cargo mutants --list` on `src/rules/`, `src/claims/` and `src/bind/` listed
**875 mutants** on 1.0.3: `evidence.rs` 233, `realism.rs` 137, `bind/mod.rs`
100, `bind/facts.rs` 99, `claims/mod.rs` 81, `privilege.rs` 78,
`injection.rs` 58, `claims/extract.rs` 32, `rules/mod.rs` 28,
`rules/catalog.rs` 21, `claims/init.rs` 8. The 1.0.4 weekly scope adds
`src/verify_fix.rs`, so the listed count is higher; the job below prints the
current number and the outcomes.

**Weekly measurement, not a gate.** `.github/workflows/mutants.yml` runs its
own `workflow_dispatch`/Monday 04:17 UTC schedule (the regular `ci.yml` does
not run on a schedule) over the rule, claim, bind and verify-fix files with
`--timeout 120`. The job writes the counts (caught, missed, timeout, unviable)
and the survival rate, `missed / (caught + missed)`, to the run's job
summary. The job was added on 2026-11-22; it has not produced a rate yet, so
this page records none from it rather than inventing one. A surviving mutant is
a review lead, not a failed build.

### Scoped measurement, 2026-11-24 (PARTIAL)

cargo-mutants 27.1.0 and rustc 1.95.0 (`x86_64-pc-windows-gnu`), on a clean
clone of `p1/precision-2026-11-24` whose `src/` is the same as the commit
titled `build(release): pin the toolchain and make release archives
reproducible`, whole test suite per mutant, `--timeout 180`, `-j 2`. The row set is
**partial**: the run was capped after 18 of the 40 listed mutants had produced
an outcome, all of them in `src/claims/extract.rs`; the other 22 were not
tested (including every mutant in `src/claims/init.rs`).

| File | Mutants listed | Tested | caught | missed | timeout | unviable |
|---|---|---|---|---|---|---|
| `src/claims/init.rs` | 8 | 0 | 0 | 0 | 0 | 0 |
| `src/claims/extract.rs` | 32 | 18 | 13 | 0 | 4 | 1 |
| **Total** | **40** | **18** | **13** | **0** | **4** | **1** |

Survival rate over the mutants that produced an outcome:
**0 of 13 = 0%** (`missed / (caught + missed)`). The four timeouts and the 22
untested mutants are not in that denominator.

The four timeouts are all boolean-operator changes in
`src/claims/extract.rs:is_identifier` (lines 56, 62, 67 twice): each hit the
180 s limit instead of finishing the suite. They are recorded as timeouts, not
counted as caught and not counted as survived. The unviable mutant is
`src/claims/extract.rs:15` (`backtick` replaced with a leaked default regex).
The host was heavily loaded by a concurrent workstream throughout the run,
which is part of why a 180 s limit was reached; a rerun on an idle host with a
larger timeout is the way to classify those four.

A missed mutant means the suite did not distinguish that change from the
original. It is a review lead, not a failed build, and this measurement gates
nothing.

### RG-A6 frontends, changed lines since ae654cf (2026-11-27)

Run 45938826459, `cargo mutants --workspace --test-workspace=true --in-diff`
against `ae654cf`, files `crates/rg-frontend-rust/src/parse_rust.rs` and
`crates/rg-frontend-ts/src/parse_ts.rs`: 123 mutants, 105 caught, 11 missed,
3 unviable, 4 timeouts.

- Nine survivors were missing tests, now added: six in `looks_like_script`
  (each `&&`/`||` of the import, `require` and mocha checks), `i > 0` in
  `tokens_hold_test_attr` (a bracket group first in a macro's tokens),
  `added < 2` in `blank_diff_marks` (exactly two added lines) and the
  `+++ `/`--- ` header check (a `+++ ` header is blanked whole, not read as an
  added line).
- Two are equivalent: `<` to `<=` on the loop bounds of `blank_elisions`
  (line 229) and `blank_diff_marks` (line 276). The extra pass reads an empty
  tail slice and changes nothing.
- The four timeouts (`+=` to `*=`/`-=` in `blank_elisions`, `+` to `-`/`*`
  in `blank_diff_marks`) make the scan loop stop advancing; the test run
  detects them by timeout.

The re-run on the branch with the new tests is run 45946711166.
