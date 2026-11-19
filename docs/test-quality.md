# Test quality

How well the test suite exercises the rules, measured on `main` after the
adversarial, fuzz and property tests (2026-11-19). These numbers are a snapshot
for reviewers. CI does not gate on them.

## Commands

```bash
# Line and region coverage, once (needs the llvm-tools-preview component)
cargo install cargo-llvm-cov --locked
cargo llvm-cov --locked --workspace --summary-only --no-cfg-coverage

# Mutation testing on the rule, claim and binding code
cargo install cargo-mutants --locked
cargo mutants --list --file 'src/rules/*.rs' --file 'src/claims/*.rs' --file 'src/bind/*.rs'
cargo mutants --file 'src/rules/*.rs' --file 'src/claims/*.rs' --file 'src/bind/*.rs'
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
89.47%. The MCP server binary, `src/bin/realism-gate-mcp.rs`, has 0% line coverage:
no test drives its stdio protocol.

## Mutation testing

`cargo mutants --list` on `src/rules/`, `src/claims/` and `src/bind/` lists
**875 mutants**: `evidence.rs` 233, `realism.rs` 137, `bind/mod.rs` 100,
`bind/facts.rs` 99, `claims/mod.rs` 81, `privilege.rs` 78, `injection.rs` 58,
`claims/extract.rs` 32, `rules/mod.rs` 28, `rules/catalog.rs` 21,
`claims/init.rs` 8.

**Not measured.** A full run builds and tests each mutant. On this machine that
is hours per run, so no caught, missed, timeout or unviable counts are
recorded here, and no survival rate is claimed. The next step is a run with a
time limit, one file at a time, and this page updated with its counts.
