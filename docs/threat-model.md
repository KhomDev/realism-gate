# Threat model

Scope: realism-gate 1.0.1 on `main`. Every code reference is `file:line` in
this repository. Items marked **unverified** have no test and no reading yet.
The method is STRIDE-light: what is worth protecting, who might attack, where
data crosses a boundary, and what proves each guarantee.

## Assets

| ID | Asset | Why it matters |
|---|---|---|
| A1 | Unreleased findings: PoC and report text for bugs that are not fixed yet | The checker must never send them anywhere, or write them where another process reads them. |
| A2 | The user's machine | No command execution from the project under review, except the one opt-in, documented command (`verify-fix`). No writes outside the paths the user named. |
| A3 | The honesty of the verdict | A `check` exit code, a claim table or a `verify-fix` outcome must not say more than the inputs support. |

## Attackers

| ID | Who | What they control |
|---|---|---|
| T1 | Author of a PoC or target repository | File contents, file names, a `.git` directory, `foundry.toml`, patches. |
| T2 | A prompt-injected agent driving the MCP server or the Action | Command arguments, environment variables, the working directory; for an agent with a shell, any command. |
| T3 | A compromised dependency or release | Code in the binary. Covered by `cargo deny`, `cargo audit` and the release attestation (`.github/workflows/release.yml`), not by the checks below. |

## Trust boundaries

| ID | Boundary | Entry point | Checks at the boundary |
|---|---|---|---|
| B1 | PoC, report and claims files read by `check` | `read_to_string` at `src/cli.rs:354`, `:497`, `:538`, `:613` | Read as data. **No size or depth cap**: a grep of `src/` finds no `metadata` or size check on these reads. |
| B2 | `realism.yaml` and `--config` | `src/cli.rs:475-480`, `:517-522` | A policy file inside the repo under review is not applied (`src/cli.rs:754` `ensure_outside`; `tests/options.rs:71`). |
| B3 | `git` in the working directory | `src/cli.rs:731` (`rev-parse --show-toplevel`, run on every `check`); `src/changed.rs` (`rev-parse`, `ls-tree`, `ls-files`, `hash-object --no-filters`, for `--changed-since`) | `--changed-since` refuses a ref that starts with `-`. Git runs with `core.fsmonitor=false` (`src/git.rs`), and git is never asked to compare the worktree: blob ids are compared in Rust, and files are hashed with no filters (PR #21). See H2. |
| B4 | `git` in `--target` at `--target-commit` | `src/cite.rs:54` (`grep`), `:89` (`rev-parse`, `ls-tree`) | A commit that starts with `-` is refused (`src/cite.rs:40-42`). The commit is resolved to a hash before the other calls. Git runs with `core.fsmonitor=false` (`src/git.rs`, PR #21). See H2 and H3. |
| B5 | `forge` on a copy of the project | launch at `src/verify_fix.rs:129-141`, run at `:169-226`; copy made by `copy_of` (`:564`) and `copy_tree` (`:601`) | `ffi = true` in the root `foundry.toml` is refused (`src/verify_fix.rs:86-95`, `:298-316`). `FOUNDRY_FFI=false` is set on the forge process (`:179`). Symlinks are not copied: `copy_tree` copies only directories and regular files (`:601-636`). |
| B6 | A unified diff given to `verify-fix --patch` | `src/verify_fix.rs:328` | Paths that are absolute, contain `..`, or carry a drive prefix are refused (`:350-392`, `:402-410`). `git apply` runs without `--unsafe-paths`. The built-in applier checks each destination again (`:412-425`). See H4. |
| B7 | MCP arguments turned into a command line | `src/mcp.rs:101-121`, `:123`, `:145`, `:176` | Paths follow `--`, and a path that starts with `-` does not run (`src/mcp.rs:379`). |
| B8 | The host decision for `verify-fix` | `src/cli.rs:296-315`; `sandbox_attested()` at `src/cli.rs:422-426` | A host run needs `--sandbox`, `--allow-host`, or `sandbox_attested()` returning true. See H1. |
| B9 | Output files | `src/cli.rs:656`, `:679`, `:697` (report, card); `:288` (`init`); `:375` (`init-claims`) | The path the user passed. **Unverified** whether a symlink at that path is followed. |

The MCP server exposes two tools, `check_poc` and `check_claims`
(`src/mcp.rs:49-79`). It does not expose `verify-fix`.

## Guarantees and the proof for each

"Test" means an automated test that fails if the guarantee breaks. "Reading"
means the code was read and no test pins it.

| ID | Guarantee | Proof | Status |
|---|---|---|---|
| G1 | `check` never starts a compiler or a test runner | Reading: only `src/verify_fix.rs` starts `forge`. | Reading. Not pinned by a test. |
| G2 | The binary has no network code | `.github/workflows/ci.yml`, job `no-network`: dependency graph and a grep of `src/`. | CI. |
| G3 | `check` writes only the files the user named | Reading: writes at `src/cli.rs:656`, `:679`, `:697`, `:288`, `:375`. | Reading. Not pinned by a test. |
| G4 | `verify-fix` does not run forge on the host without an attestation or `--allow-host` | Reading: `src/cli.rs:296-315`. | Reading. Not pinned by a test. |
| G5 | `ffi = true` in the root `foundry.toml` stops `verify-fix` | `src/verify_fix.rs` test `ffi_in_foundry_toml_refuses_to_run`. | Test. |
| G6 | A patch path that is absolute or contains `..` is not applied | `src/verify_fix.rs` test `a_patch_that_escapes_the_tree_is_refused`. | Test. |
| G7 | The `verify-fix` verdict comes from forge's JSON and exit code, not log text | `src/verify_fix.rs` tests `a_log_line_that_says_success_false_does_not_prove_a_fix` and `status_success_is_parsed_without_reading_log_strings`. | Test. |
| G8 | A commit that cannot be read is an error, not a pass | `tests/cite.rs` `an_unreadable_commit_is_an_error` (`:117`). | Test. |
| G9 | An MCP path that looks like a flag does not run | `src/mcp.rs` test `a_path_that_is_a_flag_does_not_run_and_is_not_a_pass` (`:379`). | Test. |
| G10 | A config file inside the repo under review is not policy | `tests/options.rs:71` (comment and test). | Test. |

## Hypotheses

A hypothesis is a candidate, not a finding. The status column records what
has been observed.

| ID | Hypothesis | Status |
|---|---|---|
| H1 | **Sandbox spoofing.** `sandbox_attested()` returns true when `REALISM_GATE_SANDBOX=1`, or when `/.dockerenv` or `/run/.containerenv` exists (`src/cli.rs:422-426`). An agent with a shell can set the variable or create the file, then run `verify-fix` on the host. | Confirmed by reading and by test. Not reachable through MCP (`src/mcp.rs:49`). Reachable for an agent with a shell (T2). The policy is unchanged. `tests/verify_fix_limits.rs` (PR #22) pins it, and `docs/verify-fix.md` says what it trusts. |
| H2 | **git in an untrusted tree.** A `.git/config` in the working directory or in `--target` sets `core.fsmonitor`, a pager, an alias, `diff.external`, or a filter driver, and `check` runs it. | **Tested** (`tests/security.rs`, PR #21). Not exploitable: fsmonitor, pager, alias and `diff.external` in `--target`. **Exploitable, fixed**: `core.fsmonitor` and a clean filter in the working tree under `--changed-since`, with the filter named in `.gitattributes` or in `.git/info/attributes`. A first fix that passed an empty `--attr-source` still ran a filter named in `.git/info/attributes`. Every git call now goes through `src/git.rs`, which sets `core.fsmonitor=false`, and `--changed-since` compares blob ids from `ls-tree` with `hash-object --no-filters` instead of running `git diff` on the worktree. |
| H3 | **Argument injection through `--target-commit` or `--changed-since`.** A value that starts with `-` is read as a git option. | **Tested** (`tests/security.rs`, PR #21). Refused with exit 2, no file written. Guarded by `src/cite.rs:40` and `src/changed.rs:33`. |
| H4 | **Patch escape.** A patch writes or creates a path outside the temp copy. | **Tested** (`src/verify_fix.rs` tests, PR #22). Path spellings (absolute, drive, verbatim, UNC, backslash `..`) were already refused. **Exploitable as stated, now refused**: a patch with a mode-120000 entry (a symlink) was accepted. A project symlink is not copied. |
| H5 | **Config outside the copy.** `forge` reads configuration or a binary from outside the copied tree, for example a `solc` path in `foundry.toml`, or a global Foundry profile. | **Unverified.** The `ffi` check reads only the root `foundry.toml` (`src/verify_fix.rs:298`). Not tested. |
| H6 | **Resource exhaustion.** A very large or deeply nested input exhausts time or memory. | No caps on PoC, report and claims input (B1). **Patch size is capped at 8 MiB** (PR #22). Depth and the other inputs remain **unverified** and are a follow-up. |

## Out of this model

- Bugs in Foundry, solc or git themselves.
- A compromised `forge` or `git` on `PATH`. `verify-fix --forge PATH` names the binary explicitly.
- Whether a verdict is correct about the bug. The tool is a heuristic; see the limitations in `README.md`.
