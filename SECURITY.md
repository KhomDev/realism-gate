# Security policy

realism-gate reads a PoC and a report and writes a verdict. What it executes
depends on the command, so the guarantee is stated per command. The full model
is in [docs/threat-model.md](docs/threat-model.md).

| Command | Runs a Solidity compiler or test runner | Runs other programs | Network |
|---|---|---|---|
| `check` | no | `git` only, read-only: `rev-parse --show-toplevel` in the working directory on every run (`src/cli.rs:725-727`); `rev-parse`, `ls-tree`, `grep` in `--target` with `--target-commit` (`src/cite.rs:44`, `:47`, `:55`); `rev-parse`, `ls-tree`, `ls-files` and `hash-object --no-filters` in the working directory with `--changed-since` (`src/changed.rs:57`, `:63`, `:81`, `:122`) | none |
| `init-claims` | no | none | none |
| `explain`, `rules` | no | none | none |
| `verify-fix` | **yes**: `forge test --json` twice, once on an unpatched copy and once on a patched copy (`src/verify_fix.rs:184`, `:195`) | `git apply` inside the temp copy when `--patch` is used (`src/verify_fix.rs:780`, `:808`); `forge` | forge can reach the network if the PoC or the project asks for it; `FOUNDRY_FFI=false` is set but is not a control (Foundry documents it as an exception to env overrides); there is no network isolation in the tool |
| MCP `check_poc`, `check_claims` | no | runs the `realism-gate` binary with `check` arguments only (`src/mcp.rs:141`, `:171`; `src/bin/realism-gate-mcp.rs:34`) | none |

The MCP server does not expose `verify-fix` (`src/mcp.rs:49`, two tools only).

`check` never builds, compiles or runs the PoC. It does run `git` as above, so
a repository under review can influence `git` output. Whether a repository's
own `.git/config` can make `git` execute code was tested. One case was
exploitable under `--changed-since` and is fixed; the rest are not. See
[docs/threat-model.md](docs/threat-model.md) and `tests/security.rs` (PR #21).

`verify-fix` is the one command that runs code from the project under review.
It refuses to run on the host unless the operator passes `--sandbox` or
`--allow-host`, or an environment check passes (`src/cli.rs:296-315`). See
[docs/verify-fix.md](docs/verify-fix.md).

It makes no network calls of its own. CI checks the dependency graph and the
source for network code.

## What counts

1. **A way to make it reach the network.** The PoCs it reads may be for
   embargoed findings, so "never sends anything anywhere" is a guarantee, not a
   preference. CI checks the dependency graph and the source for network code.
   A way around that is a security issue.
2. **A way to make it write or execute something.** Crafted Solidity or a
   crafted `realism.yaml` that makes it write outside `--output` or run code.
3. **Something published that should not be.** A fixture, doc or example that
   identifies a real, unfixed vulnerability or a private target.

## What does not count

- A wrong verdict. It is a heuristic and will be wrong; file a
  [false positive report](https://github.com/KhomDev/realism-gate/issues/new/choose).
- A crash on malformed input. That is a bug; file it as one.
- A dependency advisory. `cargo audit` and `cargo deny` run in CI.

## Reporting

Open a [private security advisory](https://github.com/KhomDev/realism-gate/security/advisories/new)
on the repository. Do not open a public issue for any of the three categories
above.

Include the version, the input that triggers it (synthetic, please), and what
happens.

## Response

This is a one-person project with no service-level commitment. I will confirm
or dispute a report, fix a confirmed one in a release, and credit the reporter
in the changelog unless asked not to.
