# Changelog

All notable changes to this project are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[semantic versioning](https://semver.org/). Until 1.0, the JSON and SARIF
output (schema 1) is the stable interface; the library API and the text output
may change in any minor release.

## [Unreleased]

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

[Unreleased]: https://github.com/KhomDev/realism-gate/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/KhomDev/realism-gate/releases/tag/v0.1.0
