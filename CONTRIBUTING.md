# Contributing

The most valuable contribution is **a false positive or false negative reduced
to a synthetic test**. The second most valuable is a rule with a test that
fires and a sound test that must not. The third is a report that a number in
the docs is wrong.

## Ground rules

1. **No PoCs for live findings.** Not in an issue, a fixture, a commit message
   or a discussion. Reduce it: rename the contracts, drop the addresses, keep
   only the lines the rule looks at.
2. **No targets.** Fixtures, examples and rule pages are synthetic. They name
   no protocol, chain deployment or bug-bounty platform.
   `python scripts/check_hygiene.py` checks this in CI.
3. **No invented numbers.** Every figure in the docs comes from a command, and
   the command is next to it. The corpus numbers are re-measured in CI.
4. **Never weaken a test to make it pass.** If a rule change breaks a fixture,
   either the change is wrong or the fixture is. Say which in the commit
   message.
5. **Every rule documents how it can be wrong.** A rule page without known
   false-positive modes is a page that has not been tested against real PoCs.

## Setup

```bash
git clone https://github.com/KhomDev/realism-gate
cd realism-gate
cargo test
```

## Adding or changing a rule

1. Metadata and documentation go in `src/rules/catalog.rs`. It is the only
   place rule text lives.
2. The check goes in `src/rules/privilege.rs`, `injection.rs` or
   `evidence.rs`, depending on what it is about; declaration rules go in
   `src/declaration.rs`.
3. Add `tests/fixtures/bad/RGxx_<slug>.t.sol` that fires exactly the new rule,
   and extend a `tests/fixtures/good/` file with the sound version. Add the
   expectation to `tests/rules.rs`.
4. If the rule checks Rust tests, add `tests/fixtures/bad/RGxx_<slug>.rs`
   that fires exactly it, keep the `tests/fixtures/good/` Rust files clean,
   and fill in the rule's `rust` notes in the catalog. `tests/rust.rs` runs
   the notes' "fires on" and "does not fire on" examples and fails if either
   does the opposite.
5. Regenerate the docs: `REALISM_GATE_BLESS=1 cargo test --test docs`.
6. Re-measure the corpus: `cargo build --release && bash scripts/bench-corpus.sh`.
   If the numbers move, run it with `--write` and explain the change in
   `docs/benchmark.md`.

## Before you open a pull request

```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test
python scripts/check_hygiene.py
```
