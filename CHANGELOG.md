# Changelog

All notable changes to this project are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[semantic versioning](https://semver.org/). Until 1.0, the JSON and SARIF
output (schema 1) is the stable interface; the library API and the text output
may change in any minor release.

## [Unreleased]

## [0.2.1] - 2026-11-05

A release-pipeline fix. The rules, the findings and the JSON/SARIF output are
unchanged from 0.2.0.

### Fixed

- The `release` workflow ran on the floating `v0` Action tag and failed its
  tag-matches-version check, leaving a red run every time `v0` moved. It now
  runs only on version tags (`vX.Y.Z` and `vX.Y.Z-*`).
- `verify release` run before the release was finished failed in every install
  job with a misleading `cargo-binstall` error. A new `preflight` job fails once,
  with a plain message, until the GitHub release has its `SHA256SUMS` and the
  version is on crates.io.

### Changed

- The Action's default release, the docs and the examples point at `v0.2.1`.

## [0.2.0] - 2026-11-04

Installable without Rust, usable in CI in one line, and measured: every
precision figure below is regenerated in CI by `eval/run.sh` from pinned
corpora and committed labels ([eval/README.md](eval/README.md)).

### Added

- Published on crates.io: `cargo install realism-gate --locked`.
- `cargo binstall realism-gate`: release archives follow the layout declared
  in `[package.metadata.binstall]`, and the release workflow installs through
  that metadata on Linux, macOS and Windows before publishing.
- Static musl Linux builds (x86-64 and arm64) replace the glibc builds.
- `uses: KhomDev/realism-gate@v0`: the action now writes the results to the
  job summary, keeps one pull-request comment edited in place, reports only
  files the pull request changes by default, takes `path`, `fail-on:
  none|note|warn|error`, `comment` and `sarif`, and degrades to the job
  summary on a fork's read-only token. See [docs/ci.md](docs/ci.md).
- `--format markdown`, for job summaries and PR comments.
- `realism-gate.toml`: per-rule level (`error|warn|note|off`) and path
  `exclude` globs; `--config`, `--no-config`, `--severity RULE=LEVEL`,
  `--exclude GLOB`. Flags win over the file.
- `--changed-since REF`: read everything, report only findings in files that
  differ from `REF` (committed, staged, unstaged or untracked).
- `docs/output-schema.json` describes JSON schema 1, and a test validates real
  output against it. New fields, all additive: `summary.suppressions_ignored`,
  `config`, `changed_since`.
- `// realism-gate: allow RG04 <reason>`, without parentheses, alongside the
  existing `allow(RG04) <reason>`.
- A labelled precision evaluation over three pinned corpora
  ([docs/precision.md](docs/precision.md)), checked in CI against the 0.1.0
  release binary.

### Changed

- **RG05 (code-replacement) is a warning, never an error.** It was right on 2
  of 5 labelled flags across the corpora, below the 80% floor for an error.
  Use `--severity RG05=error` or `realism-gate.toml` to restore it.
- The action's inputs are renamed `paths` -> `path` and `upload-sarif` ->
  `sarif`; the old names still work.
- An action run with no `version` and a non-release ref (such as `@v0`) now
  downloads the release that copy of the action belongs to instead of failing.

### Fixed

- RG10 (no-baseline) no longer flags an absolute balance read when the test
  fixed that balance itself: `deal`/`vm.deal`/`hoax` before the attack
  (including through a shared modifier and an internal getter), a contract
  the test deployed and funded with a different token, and the test
  contract's token balances. It still flags `> 0` over a funded balance and the
  test contract's ether.
- `scripts/bench-corpus.sh --check` passes on Windows (it failed on line
  endings alone).

### Measured, before -> after

From [eval/results/precision.md](eval/results/precision.md); precision is
TP / (TP + FP) on labelled flags, with Wilson 95% intervals there.

| | 0.1.0 | 0.2.0 |
|---|---|---|
| RG10, in-sample corpus | 2/5 correct | 2/2 correct |
| RG10, DeFiHackLabs files added later (seen during tuning) | 2/8 correct | 1/1 correct (one true positive lost) |
| RG10, corpus never used for tuning | did not fire | did not fire |
| RG05, all corpora | 2/5 correct, error | 2/5 correct, warn |
| RG01, RG02 (in-sample), RG04, RG07, RG08, RG11 | every labelled flag correct | unchanged |
| DeFiHackLabs tests with a finding other than RG08 | 23 | 20 |

Same Windows x86-64 laptop, both binaries built locally
([eval/perf](eval/perf)): binary 4,568,064 -> 4,940,288 bytes (+8.1%, mostly
the TOML parser); full in-sample corpus, median of 21 runs, 786 -> 722 ms,
within run-to-run noise. Recall is not measured: there is no labelled set of
defective PoCs to measure it against.

## [0.1.0] - 2026-10-30

First release.

### Added

- `realism-gate check`: parses Foundry tests with solar, rebuilds each test's
  execution order (initializers, constructors, `setUp`, the test wrapped in its
  modifiers, inlined helpers, callbacks), replays sender switches, and
  classifies addresses as attacker, privileged, user, mock, helper or protocol.
- Fourteen rules: RG01-RG11 on the code, RG12-RG14 on the declaration.
- `realism.yaml` declarations (schema version 1), with `realism-gate init`.
- Inline `realism-gate: allow(RULE) reason` suppressions; a suppression without
  a reason is ignored and reported.
- Text, JSON (schema 1) and SARIF 2.1.0 output; `--fail-on`, `--rules`,
  `--skip`, `--match-test`, `--match-contract`.
- `realism-gate rules` and `realism-gate explain`, generated from the same
  catalog as `docs/rules/`.
- A composite GitHub Action that downloads and checksum-verifies the release
  binary and can upload SARIF to code scanning.
- A pinned public-corpus benchmark (`scripts/bench-corpus.sh`,
  `bench/defihacklabs.json`, `docs/benchmark.md`), re-measured in CI.

[Unreleased]: https://github.com/KhomDev/realism-gate/compare/v0.2.1...HEAD
[0.2.1]: https://github.com/KhomDev/realism-gate/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/KhomDev/realism-gate/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/KhomDev/realism-gate/releases/tag/v0.1.0
