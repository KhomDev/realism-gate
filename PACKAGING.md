# Packaging realism-gate from the workspace

`realism-gate` is a Cargo workspace. The published binary crate is the root
package (`realism-gate`); the library crates under `crates/` are the parts it
is built from.

The library crates are `publish = false` for now. Flipping them to publishable
and publishing them is an approval-gated release step, not something a build
does on its own.

## What can be proved locally

The binary crate can be packaged without verification, and the packaged set
can be built:

```sh
cargo package -p realism-gate --no-verify --locked
cargo build --release -p realism-gate --locked
```

`cargo package -p realism-gate` with verification (and `cargo publish
--dry-run`) fails today, and this is expected: Cargo rewrites the path
dependencies to their crates.io versions, and `rg-ir`, `rg-model`, `rg-rules`
and the rest are not on crates.io yet. The error is `no matching package named
'rg-ir' found`.

## Dependency order for a future verified publish

Publish (or at least make available) the crates in this order; each row depends
only on rows above it:

| Order | Crate | Notes |
|---|---|---|
| 1 | `rg-ir` | shared IR, finding and severity types |
| 2 | `rg-frontend-sol` | depends on `rg-ir` |
| 2 | `rg-frontend-rust` | depends on `rg-ir` |
| 2 | `rg-frontend-ts` | depends on `rg-ir` |
| 2 | `rg-frontend-move` | depends on `rg-ir` |
| 3 | `rg-program` | skeleton; depends on `rg-ir` |
| 4 | `rg-model` | depends on `rg-ir` and the four frontends |
| 5 | `rg-rules` | depends on `rg-ir`, `rg-model` and the frontends |
| 6 | `rg-exec` | independent |
| 7 | `rg-render` | depends on `rg-ir`, `rg-model`, `rg-rules` |
| 8 | `realism-gate` | depends on all of the above |

`scripts/package-order.sh` runs the unverified local proof for every crate in
that order. It does not publish anything.

## Data embedded in `rg-rules`

`crates/rg-rules/data/measured.json` is the measured-precision table the `rules`
command prints. It is a byte-for-byte copy of `eval/solana/measured.json`,
which the evaluation scripts generate. A unit test in `rg-rules` fails when the
two differ, so regenerating the evaluation table means copying it into
`crates/rg-rules/data/measured.json` in the same change.
