# Threat model

Scope: realism-gate 1.0.4 on `main`. Every code reference is `file:line` in
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
| B1 | PoC, report and claims files read by `check` | `project::read_input` (report, claims, declaration, config) and `project::discover` / `Project::load` (sources) | Read as data. Size is checked from metadata before the read, and the error names the limit: 8 MiB per source file, 128 MiB and 20 000 files per run, 64 directories deep, 16 MiB per report/claims/declaration/config file. A 1 GiB sparse file is refused without being read (`tests/adversarial.rs` `an_oversized_source_file_is_refused_by_name_before_it_is_read`). The report/claims/declaration/config inputs must be regular files: type is checked with `symlink_metadata`, so a FIFO is refused before it can block and a symlink is refused too (`a_non_regular_policy_input_is_refused_by_name`, `a_fifo_policy_input_is_refused_without_blocking`, `project::tests::policy_inputs_must_be_regular_files`). |
| B2 | `realism.yaml` and `--config` | `src/cli.rs` (`ensure_outside`) | A policy file inside the repo under review is not applied (`tests/options.rs:71`). Size-capped as a B1 input. |
| B3 | `git` in the working directory | `src/cli.rs:727` (`rev-parse --show-toplevel`, run on every `check`); `src/changed.rs` (`rev-parse`, `ls-tree`, `ls-files`, `hash-object --no-filters`, for `--changed-since`) | `--changed-since` refuses a ref that starts with `-`. Git runs with `core.fsmonitor=false` (`src/git.rs`), and git is never asked to compare the worktree: blob ids are compared in Rust, and files are hashed with no filters (PR #21). See H2. |
| B4 | `git` in `--target` at `--target-commit` | `src/cite.rs:54` (`grep`), `:89` (`rev-parse`, `ls-tree`) | A commit that starts with `-` is refused (`src/cite.rs:40-42`). The commit is resolved to a hash before the other calls. Git runs with `core.fsmonitor=false` (`src/git.rs`, PR #21). See H2 and H3. |
| B5 | `forge` on a copy of the project | launch in `src/verify_fix.rs` (`run_forge`, `wait_forge`); copies made by `copy_of` / `copy_tree` | Foundry's global config and every tree forge could run in are TOML-parsed for `ffi`: the global `~/.foundry/foundry.toml` (`%USERPROFILE%\.foundry\foundry.toml` on Windows), the root, the `--patched` tree, and the copy after `--patch`. Any key named `ffi` with the boolean `true` in the parsed document is refused (`ffi_in_file`, `ffi_set_true`, `refuse_ffi`, `refuse_ffi_file`); a config is read only when it is a regular file of at most 1 MiB; all checks run before `run_forge`. `FOUNDRY_CONFIG` is pinned to the absolute path of a checked config in the copy (a minimal one is created if the tree has none), because Foundry loads dotenv after it starts and dotenvy does not override an already-set variable; a `.env` in any copied tree is refused (`refuse_dotenv`). `FOUNDRY_FFI=false` is set but is not a control: Foundry documents the variable as an explicit exception to env overrides. Path arguments with a separator (`--patch` always, `--forge` when it has one) are made absolute from the caller's cwd. The copy is capped at 50 000 files, 512 MiB and 64 directories, and each forge run at 10 minutes / 64 MiB stdout / 1 MiB stderr. Symlinks are not copied. `--patch` requires `git` and runs `git apply --check` first; there is no fallback applier. The ancestor-directory config search remains uncovered (H5). |
| B6 | A unified diff given to `verify-fix --patch` | `src/verify_fix.rs` (`patch_paths`, `dest_in`, `apply_patch`, `absolute_from_cwd`) | Paths that are absolute, contain `..`, or carry a drive prefix are refused before any apply. A relative `--patch` path is resolved from the caller's current directory before `git apply` runs in the copy (`tests/verify_fix_paths.rs` `a_relative_patch_outside_the_root_is_resolved_from_the_caller_cwd`). `git apply --check` runs first and `git` is required; there is no fallback applier that could accept a different subset. See H4. |
| B7 | MCP arguments turned into a command line | `src/mcp.rs` (`positional`) | Paths follow `--`, and a path that starts with `-` does not run (`src/mcp.rs` test `a_path_that_is_a_flag_does_not_run_and_is_not_a_pass`). The frame itself is bounded: `Content-Length` over 8 MiB is refused before allocation, a header line over 8 KiB is refused, and empty lines are skipped in a loop (`tests/mcp_stdio.rs`). |
| B8 | The host decision for `verify-fix` | `src/verify_fix.rs` (`verify`); `sandbox_attested()` in `src/cli.rs` | `verify` returns the refusal when `Request::host` is `None`, so a library caller cannot skip the CLI decision. The CLI turns `--sandbox`, the environment/marker attestation or `--allow-host` into `Some(policy)`. See H1. |
| B9 | Output files | `src/cli.rs:651`, `:674`, `:692` (report, card); `:288` (`init`); `:372` (`init-claims`) | The path the user passed. **Unverified** whether a symlink at that path is followed. |

The MCP server exposes two tools, `check_poc` and `check_claims`
(`src/mcp.rs:49-79`). It does not expose `verify-fix`.

## Guarantees and the proof for each

"Test" means an automated test that fails if the guarantee breaks. "Reading"
means the code was read and no test pins it.

| ID | Guarantee | Proof | Status |
|---|---|---|---|
| G1 | `check` never starts a compiler or a test runner | Reading: only `src/verify_fix.rs` starts `forge`. | Reading. Not pinned by a test. |
| G2 | The binary has no network code | `.github/workflows/ci.yml`, job `no-network`: dependency graph and a grep of `src/`. | CI. |
| G3 | `check` writes only the files the user named | Reading: writes at `src/cli.rs:651`, `:674`, `:692`, `:288`, `:372`. | Reading. Not pinned by a test. |
| G4 | `verify-fix` does not run forge on the host without an attestation or `--allow-host` | `src/verify_fix.rs` test `no_attestation_is_refused_before_forge_is_started` (library call with `host: None`, fake forge marker absent), and `tests/verify_fix_limits.rs` for the CLI branches. | Test. |
| G5 | `ffi = true` in a Foundry config the guard checks (global, root, `--patched`, post-patch copy) stops `verify-fix` before forge starts | `src/verify_fix.rs` tests `ffi_is_seen_however_toml_spells_it`, `ffi_in_foundry_toml_refuses_to_run`, `a_foundry_toml_that_does_not_parse_is_refused_not_treated_as_off`, `an_oversized_foundry_toml_is_refused_before_it_is_read`, `a_global_config_with_ffi_refuses_before_forge_is_started`; `tests/verify_fix_ffi.rs` `ffi_is_refused_in_the_root_the_patched_tree_and_after_a_patch` (8 TOML spellings × 3 trees), `a_global_config_that_enables_ffi_is_refused_before_forge_starts` (fake HOME), `a_patch_that_adds_dotenv_and_an_evil_config_is_refused`, `a_clean_dotenv_is_still_refused`, `the_forge_child_gets_the_pinned_absolute_config` (the child gets the checked absolute path, not an inherited one), and `an_oversized_foundry_toml_is_refused_before_forge_starts`; each refusal asserts the fake-forge marker is absent. `FOUNDRY_FFI` is not treated as a control; the ancestor-directory search is H5. | Test. |
| G6 | A patch path that is absolute or contains `..` is not applied | `src/verify_fix.rs` test `a_patch_that_escapes_the_tree_is_refused`. | Test. |
| G7 | The `verify-fix` verdict comes from forge's JSON and exit code, not log text | `src/verify_fix.rs` tests `a_log_line_that_says_success_false_does_not_prove_a_fix` and `status_success_is_parsed_without_reading_log_strings`. | Test. |
| G8 | A commit that cannot be read is an error, not a pass | `tests/cite.rs` `an_unreadable_commit_is_an_error` (`:117`). | Test. |
| G9 | An MCP path that looks like a flag does not run | `src/mcp.rs` test `a_path_that_is_a_flag_does_not_run_and_is_not_a_pass`. | Test. |
| G10 | A config file inside the repo under review is not policy | `tests/options.rs:71` (comment and test). | Test. |
| G11 | An MCP frame cannot make the server allocate the client's declared length, and a refused frame is not resynchronised | `src/mcp.rs` tests `an_oversized_frame_is_refused_before_any_allocation_is_answered_then_stops`, `a_header_line_over_the_cap_is_refused_by_name`, `empty_lines_are_skipped_in_a_loop_not_by_recursion`; `tests/mcp_stdio.rs` sends a 9 MB `Content-Length` with its body and a following ping, and asserts one JSON-RPC error frame, no ping response, and exit 2. | Test. |
| G12 | Control characters, line/paragraph separators and bidi controls from the files under review do not reach text, Markdown or card output | `render::sanitize` replaces `\n`, `\r`, `\t`, U+2028 and U+2029 with a space and drops other Unicode control characters plus U+202A-U+202E and U+2066-U+2069. Tests: `tests/adversarial.rs` `control_characters_in_source_never_reach_text_or_markdown_output` (raw ESC; zero `0x1b` with `--color never`, tool codes still present with `--color always`); `src/render/text.rs` `a_newline_or_bidi_in_a_path_cannot_fabricate_a_report_line` (a finding path and a parse-error path containing a newline and bidi text) and `control_characters_are_dropped_from_claim_cells`; `tests/docs.rs` re-checks the generated examples. JSON and SARIF are unchanged (serde escapes). | Test. |
| G13 | A large input is refused by name, and the trace does not grow quadratically with test count | `tests/adversarial.rs` `an_oversized_source_file_is_refused_by_name_before_it_is_read`, `twelve_thousand_test_functions_finish_within_the_bound` (serialised with the other size tests; the old quadratic code exceeds the bound), and the previously ignored 5 MB case `a_five_megabyte_source_file_finishes_and_does_not_panic`. | Test. |

## Hypotheses

A hypothesis is a candidate, not a finding. The status column records what
has been observed.

| ID | Hypothesis | Status |
|---|---|---|
| H1 | **Sandbox spoofing.** `sandbox_attested()` returns true when `REALISM_GATE_SANDBOX=1`, or when `/.dockerenv` or `/run/.containerenv` exists (`src/cli.rs:419-423`). An agent with a shell can set the variable or create the file, then run `verify-fix` on the host. | Confirmed by reading and by test. Not reachable through MCP (`src/mcp.rs:49`). Reachable for an agent with a shell (T2). The policy is unchanged. `tests/verify_fix_limits.rs` (PR #22) pins it, and `docs/verify-fix.md` says what it trusts. |
| H2 | **git in an untrusted tree.** A `.git/config` in the working directory or in `--target` sets `core.fsmonitor`, a pager, an alias, `diff.external`, or a filter driver, and `check` runs it. | **Tested** (`tests/security.rs`, PR #21). Not exploitable: fsmonitor, pager, alias and `diff.external` in `--target`. **Exploitable, fixed**: `core.fsmonitor` and a clean filter in the working tree under `--changed-since`, with the filter named in `.gitattributes` or in `.git/info/attributes`. A first fix that passed an empty `--attr-source` still ran a filter named in `.git/info/attributes`. Every git call now goes through `src/git.rs`, which sets `core.fsmonitor=false`, and `--changed-since` compares blob ids from `ls-tree` with `hash-object --no-filters` instead of running `git diff` on the worktree. |
| H3 | **Argument injection through `--target-commit` or `--changed-since`.** A value that starts with `-` is read as a git option. | **Tested** (`tests/security.rs`, PR #21). Refused with exit 2, no file written. Guarded by `src/cite.rs:40` and `src/changed.rs:33`. |
| H4 | **Patch escape.** A patch writes or creates a path outside the temp copy. | **Tested** (`src/verify_fix.rs` tests, PR #22). Path spellings (absolute, drive, verbatim, UNC, backslash `..`) were already refused. **Exploitable as stated, now refused**: a patch with a mode-120000 entry (a symlink) was accepted. A project symlink is not copied. |
| H5 | **Config outside the copy.** `forge` reads configuration or a binary from outside the copied tree, for example a `solc` path in `foundry.toml`, a global Foundry profile, an ancestor `foundry.toml`, or a dotenv file. | **Partly covered, residual stated.** The global config (`~/.foundry/foundry.toml`) is parsed with the same `ffi` rule; `FOUNDRY_CONFIG` is pinned to a checked absolute config in the copy, and a `.env` in any copied tree is refused, which closes the dotenv redirect found in review (`tests/verify_fix_ffi.rs`). **Unverified:** a `foundry.toml` in an ancestor directory of the temp copy (Foundry searches upward and the copy is under the temp dir), a `solc` path, a global profile that changes other behaviour, and a `--forge` binary. The container boundary is the mitigation. |
| H6 | **Resource exhaustion.** A very large or deeply nested input exhausts time or memory. | **Capped, with named residuals.** Per-file (8 MiB), total (128 MiB), file count (20 000), discovery depth (64) and per-input (16 MiB) limits are checked from metadata and named in the error. The suite trace base is computed once, so the 5 MB / 40 000-function case is linear and no longer ignored. The parser's own nesting limits are pinned by `tests/adversarial.rs` (`a_very_deeply_nested_expression_does_not_overflow_the_stack`). In `verify-fix`: a Foundry config over 1 MiB is refused before the read (`an_oversized_foundry_toml_is_refused_before_forge_starts`); the copied tree is capped at 50 000 files, 512 MiB and 64 directories before and while copying (`a_tree_larger_than_the_copy_limit_is_refused_before_forge_starts`, `a_tree_deeper_than_the_copy_limit_is_refused_before_forge_starts`); each forge run has a 10-minute wall-clock limit and 64 MiB stdout / 1 MiB stderr caps, and exceeding either kills the child and is a `VerifyError` (exit 2), never a fail verdict (`a_forge_that_runs_past_the_timeout_is_killed_and_is_not_a_verdict`, `a_forge_that_floods_stdout_is_killed_and_is_not_a_verdict`). **Not bounded:** the disk forge itself uses beyond the copied tree, network a fork test opens, descendant processes after the direct child is killed, and a pathological expression that defeats the parser's own limits. |

## Out of this model

- Bugs in Foundry, solc or git themselves.
- A compromised `forge` or `git` on `PATH`. `verify-fix --forge PATH` names the binary explicitly.
- Whether a verdict is correct about the bug. The tool is a heuristic; see the limitations in `README.md`.
