# Test quality

How well the test suite exercises the rules, measured on `main` after the
adversarial, fuzz and property tests (2026-11-19). These numbers are a snapshot
for reviewers. CI does not gate on them.

## Commands

```bash
# Line and region coverage, once (needs the llvm-tools-preview component)
cargo install cargo-llvm-cov --locked
cargo llvm-cov --locked --workspace --summary-only --no-cfg-coverage

# Mutation testing on the rule, claim, binding and verify-fix code
cargo install cargo-mutants --locked
cargo mutants --list --file 'src/rules/*.rs' --file 'src/claims/*.rs' --file 'src/bind/*.rs' --file 'src/verify_fix.rs'
cargo mutants --file 'src/rules/*.rs' --file 'src/claims/*.rs' --file 'src/bind/*.rs' --file 'src/verify_fix.rs'
```

## Line coverage

`cargo llvm-cov --summary-only`, run on 2026-11-19:

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
sanitizer and the input caps (`tests/adversarial.rs`). The MCP binary is
expected to have non-zero line coverage because `tests/mcp_stdio.rs` executes
it; no 1.0.4 coverage run was made, so no number is claimed.

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
summary. The job was added on 2026-11-22; it has not produced
a rate yet, so this page records none rather than inventing one. A surviving
mutant is a review lead, not a failed build.
