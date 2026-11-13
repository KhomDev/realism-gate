# Running realism-gate in CI

## GitHub Action

```yaml
- uses: KhomDev/realism-gate@v1
  with:
    path: test/
```

For a Solana program's tests, Rust or Anchor TypeScript, point it at the
test directory: `path: tests/`.

That is the whole setup. The action needs no Rust or Node toolchain: it downloads the
release binary for the runner (Linux x86-64 and arm64, macOS x86-64 and arm64,
Windows x86-64), checks it against the release's `SHA256SUMS`, and runs it.
`@v1` follows the newest 1.x release; pin `@v1.0.0` or a commit SHA if you
want nothing to move. (The `v0` tag keeps pointing at the last 0.x release;
it is not moved backwards.)

A complete workflow:

```yaml
name: poc-realism
on: [pull_request]

permissions:
  contents: read
  pull-requests: write     # only for the PR comment

jobs:
  realism:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: KhomDev/realism-gate@v1
        with:
          path: test/
```

To also bind the report's claims to the PoC, add the finding. The claim table
is printed in the job log; the JSON report and the evidence card carry it in
machine-readable form.

```yaml
      - uses: KhomDev/realism-gate@v1
        with:
          path: test/
          report: findings/rain-usdr.md
          card: "true"
```

What it does on every run:

- writes the results to the job summary (`$GITHUB_STEP_SUMMARY`);
- on a pull request, keeps **one** comment with the same results, found by a
  hidden marker and edited in place on every push;
- by default reports only findings in files the pull request changes
  (everything is still read, so inherited `setUp()` code is understood);
- fails the job on a finding at or above `fail-on`, on a `CONTRADICTED` claim, or when the claims were inferred.

### Inputs

| Input | Default | Meaning |
|---|---|---|
| `path` | `test/` | Files, directories or globs, space-separated. |
| `version` | this action's release | realism-gate release to download, e.g. `v1.0.0`. |
| `changed-only` | `true` | Report only findings in files changed by the PR (or the push). |
| `fail-on` | `error` | `none`, `note`, `warn` or `error`. |
| `comment` | `true` | Keep one PR comment with the results. |
| `sarif` | `false` | Also upload SARIF to code scanning (needs `security-events: write`). |
| `declaration` | `""` | A `realism.yaml`; empty means `./realism.yaml` if present. |
| `report` | `""` | Path to the finding. Enables the claim table in the log and the evidence card. |
| `claims` | `""` | A `claims.yaml`. Empty means `claims.yaml` next to the report, then the report's front matter, then heuristics (labelled inferred). |
| `card` | `false` | Write the evidence card (Markdown and JSON) into the run's output directory; needs `report` or `claims`. |
| `args` | `""` | Extra arguments for `realism-gate check`, e.g. `--config ci/realism-gate.toml` or `--no-fail-on-claims`. |
| `github-token` | `github.token` | Used for the comment and to fetch the base commit. |
| `binary` | `""` | For testing the action: run this binary instead of downloading one. |

`paths` and `upload-sarif`, the 0.1.0 names, still work.

### Outputs

`exit-code` (0 clean, 1 a finding at or above `fail-on` or a claims failure,
2 could not check), `errors`, `warnings`, `notes`, `version`, `report` (JSON
path), `markdown`, `sarif`, `comment` (`posted`, `updated`, `skipped`,
`failed`), and — when `report` or `claims` is set — `claim-binding`
(`structured`/`inferred`) and `claims-outcome` (`shown`/`incomplete`/
`contradicted`). With `card: true`, `card` and `card-json` are the evidence
card's two paths. They are set when the action fails the job too; read them
from a step that runs anyway (`if: always()`, or `continue-on-error: true` on
the action).

The PR comment and the job summary use the Markdown format, which is the rule
findings. The claim table is printed in the job log, is in the JSON report
(`claims`), and is the evidence card when `card` is on. A claim failure does
set the exit code even when `fail-on: none`; add `--no-fail-on-claims` to
`args` only if the workflow scores the table itself.

### When there is nothing to check

A `path` with no Foundry test in it fails the job with exit code 2, whatever
`fail-on` says. Nothing checked is not the same as nothing found, and a
misspelt path should not pass silently. A pull request that changes no test
file is different: everything is read, nothing is reported, and the job
passes.

### changed-only and shallow clones

`actions/checkout` fetches one commit by default. For a pull request the
action fetches the base commit itself (`git fetch --depth=1 origin <base>`),
using the credentials checkout leaves in place. If that is not possible
(`persist-credentials: false`, or a push whose previous commit is gone), it
says so in the log and checks everything instead.

### Pull requests from forks

GitHub gives a workflow triggered by a fork's pull request a read-only token.
The action detects a fork, skips the comment, writes the results to the job
summary, and does not fail because of it. SARIF upload is skipped for the
same reason.

To comment on fork pull requests anyway, do it in a second workflow that runs
in the base repository's context and **never checks out or runs the pull
request's code**. The first workflow saves the Markdown; the second posts it:

```yaml
# .github/workflows/poc-realism.yml: runs the fork's code, read-only token
name: poc-realism
on: [pull_request]
permissions:
  contents: read
jobs:
  realism:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - id: rg
        uses: KhomDev/realism-gate@v1
        with:
          path: test/
          comment: "false"
      - if: always()
        run: |
          mkdir -p out
          cp "${{ steps.rg.outputs.markdown }}" out/summary.md
          echo "${{ github.event.pull_request.number }}" > out/pr.txt
      - if: always()
        uses: actions/upload-artifact@v4
        with:
          name: realism-gate
          path: out/
```

```yaml
# .github/workflows/poc-realism-comment.yml: never touches PR code
name: poc-realism-comment
on:
  workflow_run:
    workflows: [poc-realism]
    types: [completed]
permissions:
  pull-requests: write
  actions: read
jobs:
  comment:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/download-artifact@v4
        with:
          name: realism-gate
          run-id: ${{ github.event.workflow_run.id }}
          github-token: ${{ github.token }}
      - env:
          GH_TOKEN: ${{ github.token }}
          REPO: ${{ github.repository }}
          HEAD_SHA: ${{ github.event.workflow_run.head_sha }}
        run: |
          set -eu
          pr=$(tr -dc '0-9' < pr.txt)
          # The number came from the fork's run: only trust it if that pull
          # request's head is the commit the run checked.
          test "$(gh api "repos/$REPO/pulls/$pr" --jq .head.sha)" = "$HEAD_SHA"
          marker='<!-- realism-gate job=fork -->'
          { echo "$marker"; cat summary.md; } > body.md
          id=$(gh api --paginate "repos/$REPO/issues/$pr/comments" \
                --jq ".[] | select(.body | startswith(\"$marker\")) | .id" | head -n1)
          if [ -n "$id" ]; then
            gh api --method PATCH "repos/$REPO/issues/comments/$id" -F body=@body.md
          else
            gh api --method POST "repos/$REPO/issues/$pr/comments" -F body=@body.md
          fi
```

The second workflow treats the artifact as data: it reads a number, checks
that the pull request with that number is the one the run tested, and posts
the Markdown. It does not check out, build or run anything from the fork. The
Markdown itself comes from the fork's run, so a hostile fork controls what
that comment says; it cannot do anything else with the token. Do not use `pull_request_target` with a checkout of the pull
request's head to get a write token: that runs untrusted code with write
access to your repository.

## Any other CI

```bash
cargo binstall realism-gate --no-confirm
realism-gate check test --format sarif --output realism.sarif
```

or download and check an archive:

```bash
version=v1.0.0
target=x86_64-unknown-linux-musl
curl -fsSLO "https://github.com/KhomDev/realism-gate/releases/download/$version/realism-gate-${version#v}-$target.tar.gz"
curl -fsSLO "https://github.com/KhomDev/realism-gate/releases/download/$version/SHA256SUMS"
sha256sum -c SHA256SUMS --ignore-missing
tar xzf "realism-gate-${version#v}-$target.tar.gz"
"./realism-gate-${version#v}-$target/realism-gate" check test --changed-since origin/main
```

Exit status: 0 when no active finding reaches `--fail-on`, no claim is
`CONTRADICTED` and the binding is structured; 1 when a finding reaches
`--fail-on`, **or** a claim is `CONTRADICTED`, **or** the claims were inferred;
2 on a usage, input, configuration or declaration error. `--fail-on` does not
suppress a claim failure, and `--no-fail-on-claims` is the escape hatch. A run
that finds no test suite at all (nothing parsed, nothing matched
`--match-contract`, or everything excluded) exits 2 rather than reporting a
clean result.

## Formats

- `--format json` (schema 1): described field by field in
  [output-schema.json](output-schema.json), which the test suite checks real
  output against. Fields are only added within a schema version. With
  `--report`, two optional fields are added: `claims` (schema
  `realism-gate.claims/v1`, binding, source, outcome, four counts, and one
  row per claim field with `status` in `shown`/`not_shown`/`contradicted`/
  `not_checkable`) and `claim_binding` (`structured` or `inferred`). See
  [claims.md](claims.md#what-the-json-says).
- `--format sarif`: SARIF 2.1.0 with the full rule table, `inSource`
  suppressions for allowed findings, and a stable `partialFingerprints` entry
  so code scanning tracks a finding across commits. Claims are not SARIF
  results: a SARIF alert points at a file and line and says the code is wrong,
  while a claim row says the report's evidence is or is not in the PoC. The
  JSON report and the [evidence card](evidence-card.md) are where claims go.
- `--card FILE` / `--card-json FILE`: the evidence card, Markdown and schema
  `realism-gate.card/v1`. It needs `--report` or `--claims`. See
  [evidence-card.md](evidence-card.md).
- `--format markdown`: what the action posts. For people; may change.
- `--format text`: for people. It may change between releases; do not parse it.
