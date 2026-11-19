# Architecture

This page explains how realism-gate is put together: why each part exists, what crosses each
boundary, and what the user sees when a part fails. It describes `main` after 1.0.1. The
module walk-through in [CODE_GUIDE.md](CODE_GUIDE.md) describes v0.4.0 and is kept as history.
Where they disagree, this page and the source win.

## One question, one pipeline

realism-gate answers one question: *does the proof prove the claim?* `check` reads a PoC test
and optionally the report that cites it. It never runs the test. `verify-fix` is the one command
that executes anything: it runs the PoC through `forge` on an unpatched tree and a patched one.

```text
                 ┌──────────── inputs (all untrusted) ────────────┐
                 │ PoC sources   report.md   claims.yaml          │
                 │ realism.yaml  realism-gate.toml  --target tree │
                 └────────────────────────────────────────────────┘
                        │                 │                 │
             project.rs │     claims/     │      cite.rs    │ (git, read-only)
   parse.rs (solar)     │  resolve +      │   RG20 index    │
   solana/parse_rust    │  extract        │                 │
   solana/parse_ts      ▼                 │                 │
   move_lang/parse   ir.rs / solana::ir   │                 │
                        │                 │                 │
                trace.rs, solana/trace.rs │                 │
                identity.rs, solana/env.rs│                 │
                        │                 │                 │
                        ▼                 │                 │
          rules/ (RG01–RG19), solana/rules, move_lang/rules │
          declaration.rs (RG12–RG14)  ◄───┼─────────────────┘ (RG20)
                        │                 │
                        ▼                 ▼
                    engine.rs ──► bind/ (claim table)
                        │
                        ▼
      render/: text · json (report/v1) · sarif · markdown · card (card/v1)
                        │
                        ▼
        cli.rs: exit 0 / 1 / 2     (Action and MCP wrap the CLI)
```

## Components

| Component | Files | Why it exists | Must never |
|---|---|---|---|
| Discovery and loading | `project.rs` | Finds test files and the base contracts they inherit, and resolves inheritance (C3 approximated depth first) | read outside the given paths |
| Solidity frontend | `parse.rs` → `ir.rs` | Lowers the `solar` AST (Foundry's parser) into an owned model, so rules can look at several files at once after the parser's arena is gone | evaluate code |
| Solana frontends | `solana/parse_rust.rs` (`syn`), `solana/parse_ts.rs` (`oxc`) → `solana/ir.rs` | Same shapes as the Solidity model, so the trace, identity and rule code is shared | resolve types (they don't; harness calls are recognised by name) |
| Move frontend | `move_lang/` | Reads Move tests far enough to find `#[test]` functions, signers and calls. There's no Move compiler | — |
| Trace | `trace.rs`, `solana/trace.rs` | Rebuilds execution order: initializers, constructors, `setUp()`, the test, with helpers inlined at the call site. Each call is tagged setup or exploit | follow external contracts' code |
| Identity | `identity.rs`, `solana/env.rs` | Decides who each address or keypair is (attacker, role, user, protocol) from names, labels, grants, deployments and data flow | claim to know on-chain access control |
| Declaration | `declaration.rs` | `realism.yaml`: facts the author states that code can't show (who depends on whom, which transaction). RG12–RG14 read only this | be applied from inside the repo under review (see *Trust*) |
| Rules | `rules/{privilege,injection,evidence,realism}.rs`, `solana/rules.rs`, `move_lang/rules.rs` | One rule per way a passing PoC proves less than it claims. Same ids across languages | change an id or meaning within 1.x |
| Rule catalog | `rules/catalog.rs` | Single source of rule metadata. `rules`, `explain`, the SARIF rule table and `docs/rules/` are generated from it | drift from the docs (a docs test regenerates them) |
| Suppressions | `suppress.rs` | `// realism-gate: allow(RGxx) reason` | count without a reason, or count at all unless `--trust-inline-allows` is set |
| Citations | `cite.rs` | RG20: names the PoC cites must exist in `--target` at `--target-commit`. Fails closed if the tree can't be read | decide the bug is real |
| Engine | `engine.rs` | Runs the rules per test, merges duplicates by (rule, file, line, column), applies scope, exclusions and suppressions, builds the `Report` | — |
| Claims | `claims/` | Resolves what the report says it shows: `--claims` → `claims.yaml` next to the report → front matter → heuristics marked *inferred* | guess a field; an empty field is NOT CHECKABLE |
| Binding | `bind/` | One predicate per claim field (docs/claims.md), evaluated against the parsed PoC and the rules that fired | hide a contradiction |
| Render | `render/` | text and markdown (for people; may change), JSON `report/v1` and SARIF 2.1.0 (stable), evidence card `card/v1` with its own SHA-256 (`render/sha256.rs`, so there's no hashing dependency) | remove or redefine a JSON field within schema 1 |
| CLI | `cli.rs`, `bin/realism-gate.rs`, `bin/rgate.rs` | Arguments, policy loading, exit codes | — |
| Verify-fix | `verify_fix.rs` | Applies a patch to a copy of the tree, runs `forge` on both sides and reads its JSON | apply a patch outside the copy, or run with `ffi = true` |
| MCP | `mcp.rs`, `bin/realism-gate-mcp.rs` | `check_poc` and `check_claims` over stdio. It shells out to the `realism-gate` binary rather than embedding it | run anything but `check` (verify-fix isn't exposed over MCP) |
| Action | `action.yml` | Downloads a pinned release (`RG_DEFAULT_VERSION`) and runs `check` | build from source |

## A run, end to end

`realism-gate check test --report finding.md --claims claims.yaml` on `examples/rain-usdr`:

1. `cli::check` finds the policy root (`git rev-parse --show-toplevel` in the working
   directory). A `realism.yaml` or `realism-gate.toml` found inside that root is ignored with
   a note, because the repo under review doesn't get to set its own policy.
2. `Project::load` reads `test/ShadowVault.t.sol` and its imports. `parse.rs` lowers each file.
3. For each test, `trace::build` produces the ordered calls and `identity` classifies the
   senders. `rules::check_test` emits raw findings: here, four RG03 *notes* (the deployer
   calls functions that aren't access-controlled).
4. `engine::analyze` merges, filters and counts them into the `Report`.
5. `claims::resolve` loads `claims.yaml` (*structured*). `bind::bind` evaluates each field:
   the attacker, impact, asset, amount and preconditions come back SHOWN, the citations
   NOT CHECKABLE (no `--target`) and the fix NOT SHOWN (`check` never applies a patch).
6. `render::text` prints the findings and the claim table. The exit code is 0: no finding
   reaches `--fail-on error`, and no claim is CONTRADICTED.

## States and contracts

**Exit codes, `check`** (`cli.rs`, lines 714–720):

| Code | Meaning |
|---|---|
| 0 | no active finding at or above `--fail-on`, and the claim table doesn't fail (or `--no-fail-on-claims` is set) |
| 1 | a finding at or above `--fail-on`, or a claim table that fails: a CONTRADICTED claim, or claims that were only inferred from the report |
| 2 | usage, input or declaration error: the inputs could not be read |

**Claim status** (`claims::Status`): `SHOWN` (the predicate holds in the PoC), `NOT SHOWN`
(the PoC lacks the evidence), `CONTRADICTED` (the PoC shows the opposite, for example a
revert under a "drain" claim), `NOT CHECKABLE` (the field is unstated, or needs an input that
wasn't given, such as `--target`). The table outcome (`claims/mod.rs`, around line 375) is
CONTRADICTED if any row is. It is INCOMPLETE if the binding was *inferred*, if a row other
than the fix is NOT SHOWN, or if any row is NOT CHECKABLE. Otherwise it is COMPLETE. A NOT
SHOWN fix row alone doesn't make it INCOMPLETE, because `check` never applies a patch.
`ClaimTable::fails()` is true for CONTRADICTED *and* for an inferred binding: an
inferred binding is not a pass.

**Verify-fix outcome** (`verify_fix::Outcome`):

| Unpatched run | Patched run | Outcome |
|---|---|---|
| passes | fails | `FixHolds` |
| passes | passes | `FixDoesNotHold` |
| fails | any | `BugDoesNotReproduce` |
| compile or usage error on either side | — | `VerifyError` (exit 2), never "failed PoC" |

## Trust boundaries

Every input is untrusted: PoCs and reports often come from someone else, or from an agent.

| Boundary | What crosses it | How it is contained |
|---|---|---|
| Source files → parsers | arbitrary Solidity, Rust, TS, Move | real parsers with error recovery; a malformed file is reported, not a panic (tested). No size or depth limits yet |
| Repo under review → policy | `realism.yaml`, `realism-gate.toml`, inline `allow` comments | ignored when inside the policy root, unless passed by flag from outside; allows count only with `--trust-inline-allows` |
| realism-gate → git | `rev-parse` in the cwd on every `check`; `rev-parse`, `ls-tree` and `grep` in `--target` (`cite.rs`); `--changed-since` (`changed.rs`) | every call goes through `src/git.rs` (`core.fsmonitor` off, no stdin); `--changed-since` never asks git to compare the worktree, so no clean filter runs; option-looking revisions are refused. Each case is planted in `tests/security.rs` |
| realism-gate → forge | `verify-fix` only | runs on a copy that holds no symlinks; a patch can't leave the copy however its path is spelled, can't create a symlink, and is capped at 8 MiB; `ffi = true` is refused; the host vs sandbox policy (`sandbox_attested`) trusts `REALISM_GATE_SANDBOX=1` or a container marker, which [docs/verify-fix.md](docs/verify-fix.md) states |
| MCP client → server | JSON-RPC over stdio | two read-only tools; the server runs the CLI binary, so the same checks apply |
| Network | — | none: no HTTP client in the dependency graph, and a CI job checks the manifest, lockfile and source |

The full threat model, with the test behind each guarantee, is [docs/threat-model.md](docs/threat-model.md).

## Failure modes

| Failure | What the user sees |
|---|---|
| A file doesn't parse | error in the report for that file; other files are still checked |
| A role is named innocently (`deployer`, not `owner`) | a false negative: no RG02. The known cases are in `docs/rules/RG02.md` and `docs/limitations.md` |
| A prank as a hard-coded address literal | a false negative: RG02 can't tell what the address is. A prank as a contract the PoC deploys from its own file has been flagged since #17 |
| `--target` tree or commit unreadable | exit 2; RG20 fails closed with the git error |
| A claims file that's empty or names no field | exit 2, "no claims" |
| forge missing, or compiles nothing | `VerifyError`, exit 2, with forge's stderr quoted |
| A policy file inside the reviewed repo | a note on stderr, not applied, and listed in the JSON report |

## Where to change things

- **A new rule:** add it to `rules/catalog.rs` (id, slug, severity, text), implement it in the
  matching `rules/*.rs` (and the Solana and Move twins if it applies there), add fixtures and
  tests, then regenerate the docs with `REALISM_GATE_BLESS=1 cargo test --test docs`.
- **A new claim field:** add it to the schema in `claims/`, add a predicate in `bind/`, then
  update the predicate table in docs/claims.md.
- **A new output field:** add it only (never remove or rename one) in `render/json.rs`, and
  update docs/output-schema.json.
