# Releasing realism-gate

This is the published release policy: how a version number is chosen, what a
release promises, and the steps that produce one. It exists because the
project shipped six versions in ten days and then tagged v0.6.0 whose release
workflow failed, so no v0.6.0 release was ever published. The cadence below is
the fix for the first problem; the mandatory dry run is the fix for the second.

## Cadence

- **Patch** (`1.x.y` to `1.x.y+1`): fixes only. A bug fix, a documentation
  correction, a packaging or CI fix. No new rules, no new flags, no changed
  defaults, no output changes beyond the fix. A patch may ship as soon as CI is
  green; it does not need a milestone.
- **Minor** (`1.x` to `1.x+1.0`): new rules, new flags, changed defaults, new
  output fields, a raised MSRV. **At most one minor per week.** A minor needs a
  named milestone (a GitHub milestone, or a named section in the changelog) and
  a changelog entry that says what changed and what did not.
- **No release without a reason.** A version number is not an announcement. If
  nothing user-visible changed, do not tag; batch the change into the next
  release.
- **1.0** is earned, not scheduled. 1.0.0 shipped when its five criteria
  were met (see below). The same rule applies to 2.0: it is earned by the
  compatibility record, not by a date.

## The 1.0 criteria

All five have to hold before 1.0.0 is tagged. They are the plan's section 1.4
list, in substance:

1. **The JSON schema is v1 and stable.** `realism-gate.report/v1` keeps
   `schema: 1`; fields are only added, never removed or redefined. SARIF for
   rule findings is unchanged.
2. **Rule ids are stable.** RG01-RG20 keep their ids, slugs and meanings
   across the release. A 1.x release may add a rule but never renumber one.
   Severity is calibrated data, not identity: a rule's severity may change in
   a minor only with a measured reason in the changelog (the fresh cell, its
   n and its Wilson interval), and never in a patch. This is the single
   resolution of the old contradiction between this criterion and the
   compatibility policy below; a measured re-grade is not a "re-grade
   without a major".
3. **Claim-binding precision is published.** `docs/precision.md` carries the
   measured number with its n and Wilson interval, generated from
   `eval/claims/results.json`. An unmeasured field is named as unmeasured, not
   left out.
4. **crates.io equals the GitHub release.** The version on crates.io is the
   one the GitHub release and the archives carry; a release is not done while
   `cargo install realism-gate` gets an older version.
5. **The Action tag is `v1`.** After the release exists, the floating `v1`
   tag is moved to it, so `uses: KhomDev/realism-gate@v1` runs 1.x. The `v0`
   tag stays where it is: the last 0.x release.

## Recorded exceptions

- **1.5.0 (2026-11-27).** 1.1.0, 1.2.0 and 1.4.0 were all released on
  2026-11-25, and 1.5.0 two days later, so the "at most one minor per week"
  rule was broken again. The maintainer decided to ship P2R-2 (binding model,
  precision floor, Claude Code plugin, measured release) as 1.5.0 anyway. It is
  the same kind of exception as 1.1.0 to 1.4.0 and is not a precedent.
- **1.5.0 first publishes the ten library crates** (`rg-ir` through
  `rg-render`, previously `publish = false`). Crate names on crates.io are
  permanent; the maintainer approved claiming all ten on 2026-11-27. They have
  no API stability guarantee outside `realism-gate`. From 1.5.1 a library is
  re-published only when it changed: each `crates/*/Cargo.toml` carries its own
  `version`, and `realism-gate` (the first `version =` line in the root
  `Cargo.toml`, which `release.yml` reads) can be ahead of them.
- **1.5.1 (2026-11-27) is a patch that adds MCP tools and MCP settings.** The
  cadence says a patch is fixes only. The maintainer chose a patch because
  the change is confined to `realism-gate-mcp`, whose 1.5.0 transport did not
  work with current clients: no rule, severity, CLI flag, default, report
  field or archive layout changed. Not a precedent for CLI changes.
- **1.5.2 (2026-11-27), same day as 1.5.1, is metadata only:** the MCP
  Registry name became `io.github.KhomDev/rgate` because the registry
  namespace is case-sensitive and 1.5.1's crates.io README carried the
  lowercase name, which cannot be changed after publishing.
- **1.5.3 (2026-11-27), same day as 1.5.2, is metadata only:** the GitHub
  Action description was shortened to 113 characters so the Actions
  Marketplace form accepts it (the limit is fewer than 125). 1.5.2 still
  carries the 136-character description.

## Compatibility policy

Stable within 1.x, and only changed in a minor with a changelog entry:

- the JSON and SARIF output schema 1: fields may be added, not removed or
  renamed, and `schema` stays `1`;
- the exit codes: `0` nothing at or above `--fail-on`, no claim contradicted,
  and a structured binding; `1` a finding at or above `--fail-on`, a
  `CONTRADICTED` claim, or an inferred binding; `2` the inputs could not be
  read;
- existing CLI flags and their meanings, and the action inputs;
- the archive names and layout, and the `[package.metadata.binstall]` metadata
  that `cargo binstall` reads;
- the set of binaries in every archive: `realism-gate`, `rgate` and
  `realism-gate-mcp`.

May change in a minor, and is called out in the changelog: the text output, the
library API, rule severities and defaults, and the MSRV. A severity change
carries its measured reason (the fresh cell, n and Wilson interval) in the
changelog entry; see criterion 2.

Never in a patch: a new rule, a changed rule severity, a changed default
`--fail-on`, a removed flag, or a changed archive layout.

A flag or output field is deprecated in a minor and removed no earlier than the
next minor; both the deprecation and the removal are in the changelog.

**A published version tag is immutable.** `vX.Y.Z` is never moved, deleted or
re-pointed. A bad release is superseded by a new patch, and the changelog says
which release it supersedes and why.

The one exception is `v1`, the floating major tag for the GitHub Action.
`uses: KhomDev/realism-gate@v1` has to run the newest 1.x release, so after a
1.x release is published and verified, `v1` is force-updated to that release's
commit:

```bash
git tag -f -a v1 -m "v1: floating major tag, now vX.Y.Z" <commit>
git push --force origin v1
```

`v1` is not a version tag: the release workflow only triggers on `vX.Y.Z`, so
moving it publishes nothing. It is updated last, after the release exists, and
never moved backwards. The old `v0` tag stays on the last 0.x release and is
not moved again.

## The dry run is mandatory before a tag

```bash
gh workflow run release.yml --ref main
gh run watch
```

The dry run builds every target, writes `SHA256SUMS`, and installs the result
on Linux, macOS and Windows twice: from the plain archive, and through
`cargo binstall`'s metadata against a local mirror of the release layout. It
creates no tag, no release and no attestation. **Do not create a version tag
until that run is green.**

This rule is not theoretical. v0.6.0 (run 42700992290) passed verify, all five
builds and checksums, then failed all three install tests: the archives carried
`realism-gate` and `rgate` but not `realism-gate-mcp`, and `cargo binstall`
aborts with `bin realism-gate-mcp is not found` because it installs every
binary the crate declares. The tag existed, the release did not. v0.6.1 fixes
the packaging and supersedes v0.6.0; the v0.6.0 tag is left where it is.

## Claude Code plugin

The plugin version is the crate version. `plugin/.claude-plugin/plugin.json`
and the `realism-gate` entry in `.claude-plugin/marketplace.json` name the
crates.io release `bin/realism-gate-ensure` installs (`cargo binstall` when it
is present, otherwise `cargo install --locked`). A version bump, patch or
minor, changes those two files in the same commit as `Cargo.toml`.
`tests/plugin.rs` fails if either disagrees with the crate, or if
`plugin/skills/realism-gate/SKILL.md` is not a byte-for-byte copy of
`skills/realism-gate/SKILL.md`. The plugin does not register
`realism-gate-mcp`; that server stays a user MCP config after the CLI is
installed (`plugin/README.md`).

## Release checklist

Run from a clean checkout of `main`.

1. `cargo fmt --all -- --check`
2. `cargo clippy --all-targets --all-features -- -D warnings`
3. `cargo test --locked`
4. `python scripts/check_hygiene.py`
5. **Check the precision page against its results.** If the claims code, the
   claims corpus or the eval harness changed, re-run and confirm the page:
   ```bash
   python eval/claims/run.py --check
   ```
   `docs/precision.md` must quote `eval/claims/results.json` (the number, its
   n and its Wilson interval) and must not publish any other claim-binding
   number. A 1.0 release with an unmeasured or stale claims page is not
   tagged.
6. Bump the version in `Cargo.toml` (`Cargo.lock` follows), in `action.yml`
   (`RG_DEFAULT_VERSION`), in `plugin/.claude-plugin/plugin.json` (`version`),
   and in `.claude-plugin/marketplace.json` (the `realism-gate` entry's
   `version`). The plugin version **is** the crates.io release
   `realism-gate-ensure` installs, so a bump (for example 1.4.0 to 1.5.0)
   changes `Cargo.toml`, `plugin.json` and the marketplace entry together.
   Also bump any doc that pins a version, and if `skills/realism-gate/SKILL.md`
   changed, copy it byte for byte to `plugin/skills/realism-gate/SKILL.md`.
   `tests/docs.rs` fails if `action.yml` disagrees with the crate version.
   `tests/plugin.rs` (`plugin_version_is_the_crate_version`) fails if
   `plugin.json` or the marketplace entry disagrees with the crate version,
   and `plugin_skill_is_the_repository_skill` fails if the plugin skill drifts.
7. Add the changelog entry under the new version and move the `[Unreleased]`
   comparison link.
8. If the README examples, the claims block or the golden card changed:
   `REALISM_GATE_BLESS=1 cargo test --test docs`, then `cargo test --test docs`
   without the variable to prove the blocks are current.
9. Commit, push `main`, and wait for CI to be green.
10. Run the dry run above and wait for every job, including the three install
    tests, to pass.
11. Create the annotated tag and push it:
    ```bash
    git tag -a vX.Y.Z -m "realism-gate X.Y.Z"
    git push origin vX.Y.Z
    ```
12. Wait for the tag-triggered run. Confirm the release has five archives and
    `SHA256SUMS`, and that the notes are the changelog section:
    ```bash
    gh run watch
    gh release view vX.Y.Z
    scripts/release-assets.sh vX.Y.Z
    ```
13. **Check the release is reproducible.** Run the rebuild workflow and confirm
    it reports `reproducible: YES`:
    ```bash
    gh workflow run reproduce-release.yml -f tag=vX.Y.Z
    gh run watch
    ```
    A mismatch is a packaging or toolchain defect: do not move `v1` until it
    is understood. (Releases before the pinned-toolchain change used a moving
    `stable` and plain tar/zip; they cannot pass, and `--binary-only` records
    the binary result for them.)
14. `cargo publish` is a separate, manual step; it is not part of the release
    workflow. A release is not done while crates.io is behind: after
    publishing, `verify-release.yml` installs the published release from
    crates.io, from the archives and through the action. Since 1.5.0 the ten
    library crates are published too, **before** `realism-gate`, in the order
    of PACKAGING.md (`rg-ir`, the four frontends, `rg-program`, `rg-model`,
    `rg-rules`, `rg-exec`, `rg-render`, then `realism-gate`). Publish a library
    only if its version was bumped because it changed (`git diff vPREV -- crates/NAME`);
    otherwise `realism-gate` depends on the version already on crates.io. `rg-render` is
    always bumped and published with `realism-gate`: its `VERSION` is the tool version the
    reports print. crates.io limits new crate names to a burst of five and
    then one per ten minutes, so a first publish of all ten takes about an
    hour; wait for each crate to appear in the index before the next that
    depends on it.
15. Move the floating Action tag to the release, and only then:
    ```bash
    git tag -f -a v1 -m "v1: floating major tag, now vX.Y.Z" <commit>
    git push --force origin v1
    ```

## Reproducible archives

A release is built to be rebuildable, and the published archives carry the
metadata that makes that check meaningful:

- **Toolchain pinned to 1.95.0**, in `release.yml` (`RELEASE_RUST`) and
  `reproduce-release.yml`. `stable` moved between releases and cannot recreate
  an old binary.
- **Linux builds remap absolute paths** with `scripts/reproducible-rustflags.sh`:
  the workspace becomes `/build/realism-gate`, the cargo registry and git
  checkouts become `/cargo/registry` and `/cargo/git`.
- **Archives are repacked deterministically** in the `checksums` job from the
  build jobs' transport archives: one mtime (`SOURCE_DATE_EPOCH`, default
  1980-01-01), uid/gid 0, sorted entries, `gzip -n`, `zip -X -@`. The job runs
  on `ubuntu-24.04` because the tar/gzip/zip versions are part of the bytes.
- **The attestation is created after the repack**, in the same job, over the
  five archives that are actually published.

`scripts/reproduce-release.sh TAG TARGET` downloads the tag's source archive,
the release archive and `SHA256SUMS`, verifies the archive against
`SHA256SUMS`, rebuilds with the pinned toolchain, and compares both every
binary's SHA256 and the repacked archive's SHA256 with the released entry. Exit
0 means both matched. `.github/workflows/reproduce-release.yml` runs it on
`ubuntu-24.04`.

This is a forward guarantee. Releases up to 1.0.4 were built with whatever
`stable` was on the runner and packaged with plain `tar czf`/`7z`, so their
archive bytes include mtimes and their compiler version was never recorded;
for those, run with `--binary-only` and treat any binary mismatch as the
answer.

**Local windows-gnu rebuilds.** The pinned toolchain on a Windows host can be
`x86_64-pc-windows-gnu`, whose GNU linker writes the build time into the PE
header. Two otherwise identical builds then differ by a handful of bytes:
`TimeDateStamp` at offset 0x88 and the PE `CheckSum` at 0xd8. When comparing
such builds by hand, add `-C link-arg=-Wl,--no-insert-timestamp`; the release
workflow's Windows artifact is x86_64-pc-windows-msvc, not this toolchain.

## What the release workflow does

| Job | What it proves |
|---|---|
| `verify` | the tag matches `Cargo.toml`; fmt, clippy and tests pass on the pinned toolchain |
| `build` | five targets build; each archive carries every binary declared in `Cargo.toml` |
| `checksums` | the five archives are repacked deterministically, exactly five remain, `SHA256SUMS` is written, and the provenance attestation covers those five |
| `install-test` | the archive and the `cargo binstall` metadata install on Linux, macOS and Windows, and every declared binary is present |
| `release` | on a tag only: notes from the changelog, assets, build provenance |

The binary list is read from `Cargo.toml` by `scripts/declared-bins.sh` in the
workflow and by `scripts/install-check.sh` in the install test, so a new
`[[bin]]` cannot be left out of an archive again.
