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
2. **Rule ids are stable.** RG01-RG20 keep their ids, slugs, severities and
   meanings across the release. A 1.x release may add a rule, but never
   renumber or re-grade an existing one without a major version and a
   changelog entry that says which ids moved.
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
library API, rule severities and defaults, and the MSRV.

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
   (`RG_DEFAULT_VERSION`), and in any doc that pins a version. `tests/docs.rs`
   fails if `action.yml` disagrees with the crate version.
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
13. `cargo publish` is a separate, manual step; it is not part of the release
    workflow. A release is not done while crates.io is behind: after
    publishing, `verify-release.yml` installs the published release from
    crates.io, from the archives and through the action.
14. Move the floating Action tag to the release, and only then:
    ```bash
    git tag -f -a v1 -m "v1: floating major tag, now vX.Y.Z" <commit>
    git push --force origin v1
    ```

## What the release workflow does

| Job | What it proves |
|---|---|
| `verify` | the tag matches `Cargo.toml`; fmt, clippy and tests pass |
| `build` | five targets build; each archive carries every binary declared in `Cargo.toml` |
| `checksums` | exactly five archives, one `SHA256SUMS` |
| `install-test` | the archive and the `cargo binstall` metadata install on Linux, macOS and Windows, and every declared binary is present |
| `release` | on a tag only: notes from the changelog, assets, build provenance |

The binary list is read from `Cargo.toml` by `scripts/declared-bins.sh` in the
workflow and by `scripts/install-check.sh` in the install test, so a new
`[[bin]]` cannot be left out of an archive again.
