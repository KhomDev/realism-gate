## What this changes

<!-- One or two sentences. -->

## Checklist

- [ ] `cargo fmt --all`, `cargo clippy --all-targets --all-features -- -D warnings` and `cargo test` pass.
- [ ] A rule change comes with a fixture that fires and a fixture that must not.
- [ ] If rule text changed: `REALISM_GATE_BLESS=1 cargo test --test docs` was run and the regenerated pages are in this PR.
- [ ] If corpus numbers moved: `scripts/bench-corpus.sh --write` was run and `docs/benchmark.md` says why.
- [ ] No PoC for a live finding, no target or platform names, no host paths (`python scripts/check_hygiene.py`).
