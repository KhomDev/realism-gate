# Running realism-gate in CI

## GitHub Action

The repository is a composite action. It downloads the release binary for the
runner, verifies it against the release's `SHA256SUMS`, runs `check`, and can
upload SARIF to GitHub code scanning so findings appear inline on the pull
request.

```yaml
name: poc-realism
on: [pull_request]

permissions:
  contents: read
  security-events: write   # only needed for upload-sarif

jobs:
  realism:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: KhomDev/realism-gate@v0.1.0
        with:
          paths: test
          fail-on: error
          upload-sarif: true
```

Inputs:

| Input | Default | Meaning |
|---|---|---|
| `version` | the action's own tag | Release to download, e.g. `v0.1.0`. |
| `paths` | `test` | Space-separated files or directories. |
| `fail-on` | `error` | `error`, `warn`, `note` or `never`. |
| `declaration` | `""` | Path to a `realism.yaml`. Empty: `./realism.yaml` if present. |
| `args` | `""` | Extra arguments for `realism-gate check`. |
| `upload-sarif` | `false` | Upload results to code scanning. |

Supported runners: Linux x86-64 and arm64, macOS x86-64 and arm64, Windows
x86-64.

## Any other CI

Download a release archive, check it, and run it:

```bash
version=v0.1.0
target=x86_64-unknown-linux-gnu
curl -fsSLO "https://github.com/KhomDev/realism-gate/releases/download/$version/realism-gate-${version#v}-$target.tar.gz"
curl -fsSLO "https://github.com/KhomDev/realism-gate/releases/download/$version/SHA256SUMS"
sha256sum -c SHA256SUMS --ignore-missing
tar xzf "realism-gate-${version#v}-$target.tar.gz"
"./realism-gate-${version#v}-$target/realism-gate" check test --format sarif --output realism.sarif
```

Exit status: 0 when no active finding reaches `--fail-on`, 1 when one does, 2
on a usage, input or declaration error. A run that finds no parseable test
suite at all exits 2 rather than reporting a clean result.

## Formats

- `--format json` (schema 1): `tool`, `version`, `schema`, `summary`,
  `declaration`, `findings[]`, `parse_errors[]`. Fields are only added within a
  schema version.
- `--format sarif`: SARIF 2.1.0 with the full rule table, `inSource`
  suppressions for allowed findings, and a stable `partialFingerprints` entry
  so code scanning tracks a finding across commits.
- `--format text`: for people. It may change between releases; do not parse it.
