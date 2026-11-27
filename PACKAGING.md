# Packaging realism-gate from the workspace

`realism-gate` is a Cargo workspace. The published binary crate is the root
package (`realism-gate`); the library crates under `crates/` are the parts it
is built from.

The library crates are `publish = false` for now. Flipping them to publishable
and publishing them is an approval-gated release step, not something a build
does on its own.

## What is measured on this workspace

`cargo package -p realism-gate --list --locked` succeeds and prints the
package contents (22 files: `src/**/*.rs`, `Cargo.toml`, `Cargo.lock`,
`README.md`, `LICENSE`, `CHANGELOG.md`).

`cargo build --release -p realism-gate --locked` succeeds: the packaged set of
crates builds from the workspace.

Plain `cargo package -p realism-gate --no-verify --locked` **fails**, and this
is expected on cargo 1.95.0:

```text
error: failed to prepare local package for uploading
Caused by:
  no matching package named `rg-exec` found
  location searched: crates.io index
```

Cargo rewrites a path dependency to its crates.io version before writing the
archive and resolves that version first. `--no-verify` skips the build, not the
resolution. The same failure appears one crate down (`rg-frontend-sol` needs
`rg-ir`); only `rg-ir`, which has no workspace dependencies, packages plainly:

```sh
cargo package -p rg-ir --no-verify --locked   # writes rg-ir-1.5.0.crate
```

So a verified `cargo package` (and `cargo publish --dry-run`) becomes possible
only after the sub-crates are published in dependency order. That is an
approval-gated outward action and this workspace does not attempt it.

## Rehearsing the archives locally

`scripts/package-order.sh` produces the `.crate` archive for every crate, in
dependency order, without publishing anything. It passes a temporary
`[patch.crates-io]` section that maps each workspace crate to its path, which
lets Cargo resolve the rewritten dependencies while it writes the archives.
The archives are otherwise the ones the registry would receive. This proves the
manifest, the file list and the packaging of every crate; it does not make the
crates publishable.

## Dependency order for a future verified publish

Publish the crates in this order; each row depends only on rows above it:

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

## Data embedded in `rg-rules`

`crates/rg-rules/data/measured.json` is the measured-precision table the `rules`
command prints. It is a byte-for-byte copy of `eval/solana/measured.json`,
which the evaluation scripts generate. A unit test in `rg-rules` fails when the
two differ, so regenerating the evaluation table means copying it into
`crates/rg-rules/data/measured.json` in the same change.
