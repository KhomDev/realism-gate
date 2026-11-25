# Changelog

All notable changes to this project are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[semantic versioning](https://semver.org/). Until 1.0, the JSON and SARIF
output (schema 1) is the stable interface; the library API and the text output
may change in any minor release.

## [Unreleased]

## [1.4.0] - 2026-11-25

### Changed (severity)

- **RG02 and RG03 ship at `note` on Solana.** The 1.3.0 fresh held-out round
  measured RG02 at 0/16 and RG03 at 0/5: the remaining flags are the fee payer
  of an instruction whose accounts mark the role account non-signing, the
  attacker's own fixture key, or a setup helper. A rule at `warn` with zero
  fresh precision is noise. RG01, RG04 and RG06 were already notes. The
  Solidity severities are unchanged.
- **RG08 and RG09 ship at `warn` on Solana; no Solana rule ships at `error`
  in this release.** The floor is a fresh held-out Wilson lower bound of at
  least 0.80. RG08 is right 10/10 on the v2 held-out split, but 10/10 has a
  lower bound of 0.7225, so `error` is not supported yet; it returns there at
  16/16, where the lower bound reaches 0.8064. RG09's only fresh flag was
  labelled unclear, so it has no rate at all. `warn` is reported and is not a
  CI failure under the default `--fail-on error`.
- The floor is now enforced against the fresh measurement: `measure.py` in
  `eval/solana-poc-v2/` regenerates `measured.json` from the committed
  held-out labels, and `tests/precision_floor.rs` reads it. A Solana rule at
  `error` with no fresh cell of n >= 5 fails the test, so an unmeasured error
  rule cannot ship. Previously the test read a file with no fresh cells and
  passed vacuously.

### Fixed (CI)

- **The weekly `fuzz` workflow names the GNU triple.** The prebuilt
  cargo-fuzz installed by `install-action` is a musl binary, and cargo-fuzz
  derives its default build target from its own triple; a musl target links
  libc statically, which the sanitizer refuses ("sanitizer is incompatible
  with statically linked libc"). `cargo fuzz run --target
  x86_64-unknown-linux-gnu` pins the dynamic-libc target. The fix is proven
  locally by inspection only: this branch has not been pushed, so no green
  `fuzz` run id exists yet.

### Fixed (P1-C precision round, proposed 1.3.0)

- **A helper file with no test is no longer read as a fragment.** The script
  reader wrapped every import-only helper passed alongside the PoC as
  `it("fragment")` and flagged RG08 on it. Script mode now requires an
  executable top-level statement (or a `main` call), and auto-detection only
  wraps the unit's first file, not its imported helpers.
- **A test title that says the attack is blocked is a control test.** Titles
  with `blocks`, `blocked`, `denies`, `cannot`, `does not allow`, `not
  allowed` or `must fail` join the existing `secure`/`control`/`prevents`/
  `rejects`/`fails when` class, so RG01/RG02/RG03/RG04/RG06 do not fire on
  them. A leading ✓/✔ (the secure-suite convention) is included.

## [1.2.0] - 2026-11-25

### Added (P1-C, 1.2.0)

- **Four readers for the PoCs judges receive.** A Rust `#[test]` that calls
  program functions directly is checked (the direct call is modelled as the
  exploit transaction); a Rust `fn main` that drives a PoC framework
  (`poc-framework`, `LocalEnvironment`, `Environment::…_client`) is read as one
  test; a TypeScript script that drives an Anchor/web3.js/LiteSVM client is read
  as one test, with `main()` inlined; and `check --fragment <file>` wraps a
  pasted fragment as one test body. A `.rs`/`.ts`/`.js` file given explicitly
  that has no top-level test is auto-detected. The report gains
  `fragment: true` when the boundary is the tool's.
- **`tests/precision_floor.rs`** enforces the severity floor in
  `docs/precision.md`: a rule ships at `error` on Solana only when its fresh
  held-out Wilson lower bound is at least 0.80.
- **`eval/solana-poc-v2/`** is the sealed fresh corpus: 81 counted units (28
  judged, 53 exploit-by-construction), pinned before the first `src/` change.

### Changed (P1-C, proposed 1.2.0)

- **RG01, RG04 and RG06 ship at `note` on Solana.** Their P1 PoC held-out
  precision was 16.7%, 0% and 0%; a rule at `error` now needs a fresh held-out
  lower bound of at least 0.80. The Solidity severities are unchanged. The
  catalog gains `solana_severity()`; the generated rule pages say which ids
  moved.
- **RG01 no longer counts an attacker-signed grant as a harness grant.** An
  attacker that initialises the program as its own admin, or re-initialises
  itself, took the role through the bug; that is the exploit, not the harness.
- **RG04/RG06 fixtures and preconditions.** A write of an account a later
  initialise/create instruction sets up is not reported. A precondition
  written while setting up is reported at `note` with `Precondition
  fabricated:` and says the drained balance never passed through the program.
- **RG10** accepts a read of the same account before the attack as a baseline,
  whatever the local is called, not only a name like `before`.
- **Control tests** (expects failure, or a title saying `secure` / `control` /
  `prevents` / `rejects` / `fails when`) no longer fire RG01, RG02, RG03, RG04
  or RG06; RG08 still applies.
- **A fresh keypair** the test never gives a role (by a grant, a role field, an
  initialise/configure call, or a declaration) is not privileged. A role
  granted by another signer, or shown by a role-named signer of a gated
  instruction, still is.

### Fixed (P1-C, proposed 1.2.0)

- **Fragments**: bare statements and `it()` blocks pasted into an issue parse.
- **Scripts**: an Anchor script with `main()` is no longer silent.
- **PoC-framework binaries**: `execute_as_transaction` is a transaction with
  its signers and instructions; `LocalEnvironment` is a harness and
  `Environment::devnet_client` is a live cluster.
- **RG08**: the rule's scope is the exploit part of the trace, matching its own
  definition; the study rubric is updated to agree.
- **Docs**: the rule pages and README example blocks are regenerated for the
  1.2.0 severities and the four readers.

## [1.1.0] - 2026-11-25

### Added

- **`rules` and `explain` print measured precision per language.** The rule
  catalog embeds `eval/solana/measured.json`; a rule with no fresh or held-out
  label prints `unmeasured`, never 100%. The JSON rule rows gain a `measured`
  object keyed by language with `status`, `tp`, `fp`, `labelled`, `precision`,
  `wilson95`, `split`, `population` and, for cells with fewer than five labels,
  `count_only` (printed as `k/n (n<5, count only)`, never a percentage). The
  Rust and Anchor TypeScript numbers are the held-out split of the Solana PoC
  study (`eval/solana-poc/`), not the earlier test-suite measurement, which
  stays published in `eval/solana/RESULTS.md`; the Solidity number uses only the
  fresh (out-of-sample) corpus. `docs/precision.md` states the population and
  the limits, and the generated rule pages carry the same line.

### Changed

- **`rules --format json`: `languages` lists `javascript`** for every rule
  that checks Solana tests. The tool already read JavaScript Anchor/web3.js
  tests; the list said only `rust` and `typescript`. `eval/results/rules-v0.6.0.json`
  is regenerated (only `languages` and the new `measured` objects change; the
  precision tables and the R3 check are unchanged).

### Fixed

- **Release workflow:** the `checksums` job did not list `verify` in `needs`, so
  the version it repacks with was empty; and `scripts/package-release.sh` wrote
  the zip to a relative output directory from inside the staging directory.
  Both were caught by the 1.1.0 dry runs, before any tag. A regression case in
  `scripts/test-package-release.sh` covers the relative output directory.
- **Attestation test on Linux/macOS:** the fake `gh` in
  `scripts/test-verify-attestation.sh` was not executable, so `command -v gh`
  found the runner's real `gh`. The Action itself runs the script with `bash`
  and was not affected.
- **CI package check** now allows `eval/solana/measured.json`, which the crate
  embeds for `rules`/`explain`.

### Documentation

- **Solana PoC measurement (P1-A redo, 2026-11-25).** `eval/solana-poc/`
  measures the frozen 1.0.4 rules on 99 public Solana PoC units (12 judged,
  82 exploit-by-construction, 5 patched negative controls) from 45 pinned
  sources, preregistered before any run or label. The tool reads 1 of 12
  judged PoCs (8.3% [1.5-35.4]) and 72 of 82 exploit-by-construction ones;
  held-out precision over all rules is 46/106 = 43.4% [34.4-52.9], with RG08
  29/30 and RG09 7/7 against RG01 2/12, RG02 8/30, RG04 0/16 and RG06 0/6.
  `docs/precision.md` carries the summary (checked by `tests/docs.rs`).
  No rule logic changed.
- **Correction.** `eval/solana/{README,RESULTS}.md` and `docs/precision.md`
  said a public Solana PoC corpus does not exist in a form the tool reads.
  That was wrong: the search checked three repositories and stopped, and the
  Neodyme workshop PoCs it cited are unsolved templates. The text now says so
  and links the PoC measurement.
- **P1 Rust/Anchor TypeScript measurement.** Four pinned public test suites,
  174 flags, 133 sampled, 63 held-out labels, Wilson 95% intervals per rule and
  language. The report says plainly that the corpus is test suites, not PoCs,
  and records the missing PoC corpus as a gap rather than widening the claim.
- **Correction.** `docs/precision.md` said the `264ca72` solana-program-library
  pin had five `program/tests` directories; at that commit only `token-lending`
  and `governance` have one. The text is corrected.

## [1.0.4] - 2026-11-22

A security and robustness release, from a self-review of 1.0.3. No rule id,
flag, default or output field changed; the JSON and SARIF formats are
unchanged. Reporter: self-review.

### Security

- **`verify-fix` reads `ffi` from TOML under a conservative key
  normalization.** Foundry resolves case and separator aliases: `FFI`, `Ffi`,
  `ffi_` and `ffi__` all set `ffi` (verified against forge 1.5.1). The guard
  applies a deliberately coarser normalization than Foundry (lowercase ASCII;
  keeps ASCII alphanumerics only, dropping Unicode too), which catches every alias Foundry
  resolves and also refuses some spellings Foundry treats as unknown (`f_f_i`). Only
  exactly `ffi` with the boolean value `false` is accepted, a boolean `true`
  is refused, any other value is refused conservatively, and every
  non-canonical spelling is refused whatever its value. The scan is document-wide: spacing, tabs, quoted keys,
  dotted keys, inline tables, the top-level table and profiles the program has
  never heard of are all covered. Foundry's global config (`~/.foundry/foundry.toml`, or
  `%USERPROFILE%\.foundry\foundry.toml`), the root tree, the `--patched` tree
  and the copy after `--patch` are checked before the tree is used, with the
  global config and both copies parsed again immediately before each forge
  start. A `foundry.toml` that does not parse is
  refused rather than assumed off. A config in an ancestor directory of the
  temp copy is **not** checked; on Windows Foundry may resolve the OS known
  profile folder while this check uses `USERPROFILE`/`HOME`, so the two can
  differ; `docs/threat-model.md` H5 records both residuals.
  `FOUNDRY_FFI=false` is not described as a control: Foundry documents the
  variable as an explicit exception to its environment overrides. Tests:
  `ffi_is_seen_however_toml_spells_it`,
  `foundry_key_aliases_and_non_boolean_ffi_values_are_refused`,
  `ffi_is_refused_in_the_root_the_patched_tree_and_after_a_patch` (17 exact
  and alias spellings),
  `a_global_config_with_an_ffi_alias_is_refused_before_forge_starts`,
  `a_minimal_top_level_ffi_alias_is_refused_before_forge`
  (`src/verify_fix.rs`, `tests/verify_fix_ffi.rs`).
- **Write `fs_permissions` and its aliases are refused, closing the
  config-rewrite path.** A PoC could set `fs_permissions = [{ access =
  "read-write", path = "C:/" }]` and use `vm.writeFile` to rewrite the other
  copy's or the global config to an ffi alias between the two runs; the key
  itself has aliases too (`FS_PERMISSIONS`, `FS-PERMISSIONS`, `fsPermissions`,
  `fs_Permissions`, `Fs-Permissions`, `fs__permissions`, the quoted key
  `"fs.permissions"`, and spaces), which Foundry resolves to the same setting.
  The unquoted dotted form `fs.permissions = ...` parses as nested
  `fs`/`permissions` keys; neither Foundry nor the guard resolves it as
  `fs_permissions`, so there is no gap. Any key normalized to `fs_permissions` must be
  the exact spelling, and every entry in every checked config must grant
  exactly `access = "read"`; a write or read-write entry, a missing or
  non-string access, a non-array value, a non-table entry, or an alias is
  refused before any forge run. Both copies and the global config are also
  re-parsed immediately before each run, which catches a simple rewrite; the
  `fs_permissions` refusal is the primary control. Tests:
  `fs_permissions_must_be_read_only`,
  `write_fs_permissions_are_refused_in_every_config_the_guard_reads`,
  `fs_permissions_aliases_are_refused_in_every_config_the_guard_reads`,
  `a_global_config_with_a_fs_permissions_alias_is_refused_before_forge_starts`,
  `the_alias_cross_run_combination_is_refused_before_forge`,
  `read_only_fs_permissions_reach_forge`,
  `a_rewritten_tree_or_global_config_is_refused_before_the_next_run`,
  `a_global_config_rewritten_by_the_first_run_is_refused_before_the_second`.
- **The dotenv redirect is closed.** Foundry loads a `.env` after it starts,
  and dotenvy does not override an already-set variable. Removing
  `FOUNDRY_CONFIG` therefore let a `--patch` that created `.env` +
  `evil.toml` point forge at an unparsed `ffi = true` config. `FOUNDRY_CONFIG`
  is now pinned to the absolute path of a config the guard checked inside the
  copy (a minimal `[profile.default]` file is created when the tree has none),
  and any `.env` in a copied tree is refused before each forge run. Tests:
  `a_patch_that_adds_dotenv_and_an_evil_config_is_refused`,
  `a_clean_dotenv_is_still_refused`,
  `the_forge_child_gets_the_pinned_absolute_config` (`src/verify_fix.rs`,
  `tests/verify_fix_ffi.rs`).
- **Relative `--patch` and `--forge` paths are resolved before the copy's
  working directory applies.** `--patch ../fix.diff` (outside `--root`) and a
  `--forge` value containing a separator keep working; a bare `forge` keeps
  PATH lookup (`src/verify_fix.rs`, `tests/verify_fix_paths.rs`).
- **The host policy is enforced in the library.** `verify` refuses a request
  with no `HostPolicy` before copying a tree or starting forge, so a library
  caller is covered like the CLI (`src/verify_fix.rs`,
  `no_attestation_is_refused_before_forge_is_started`).
- **`--patch` requires git.** `git apply --check` runs first and the
  hand-written fallback applier is gone, so a tree the patch does not describe
  is never run (`src/verify_fix.rs`, `docs/verify-fix.md`).
- **A Foundry config is size-checked before it is parsed.** A tree or global
  `foundry.toml` must be a regular file of at most 1 MiB; a larger one is
  refused before the read (`src/verify_fix.rs`,
  `an_oversized_foundry_toml_is_refused_before_forge_starts`).
- **Each forge run is time- and output-bounded.** A run longer than 10 minutes,
  or one that prints more than 64 MiB of stdout or 1 MiB of stderr, is killed
  and is a `VerifyError` (exit 2), never a fail verdict; the verdict is not
  taken from a truncated log (`wait_forge`,
  `a_forge_that_runs_past_the_timeout_is_killed_and_is_not_a_verdict`,
  `a_forge_that_floods_stdout_is_killed_and_is_not_a_verdict`).
- **The copied tree is bounded.** The project or `--patched` tree is
  attacker-controlled; the copy is capped at 50 000 files, 512 MiB in total,
  50 000 directories and 64 levels deep, checked from metadata while copying,
  so a hostile tree cannot fill the disk with files or empty directories before
  forge's own limits apply (`copy_tree`, `copy_tree_with_limits`,
  `the_copy_file_byte_and_directory_caps_are_enforced`,
  `a_tree_larger_than_the_copy_limit_is_refused_before_forge_starts`,
  `a_tree_deeper_than_the_copy_limit_is_refused_before_forge_starts`).
- **MCP framing is bounded.** `Content-Length` is refused above 8 MiB before
  any allocation. The server writes a JSON-RPC error and stops (exit 2): it
  does not claim to resynchronise past a body it refused, so a following ping
  is not served. A header line is capped at 8 KiB; empty lines are skipped in a
  loop instead of by recursion (`src/mcp.rs`, `tests/mcp_stdio.rs`).
- **Control characters and bidi formatting from source and claims do not
  reach terminal output.** The text, Markdown and evidence-card renderers
  replace newline, carriage return, tab and U+2028/U+2029 with a space, and
  drop other Unicode control characters plus the bidi controls U+061C, U+200E,
  U+200F, U+202A-U+202E and U+2066-U+2069, from finding messages, evidence,
  paths, suite and test names, claim text and evidence, suppression notes and
  parse errors, before any colour wrapper. A Linux file name containing a
  newline can no longer fabricate a report line. JSON and SARIF keep their
  serde escaping (`src/render/mod.rs`,
  `control_characters_in_source_never_reach_text_or_markdown_output`,
  `a_newline_or_bidi_in_a_path_cannot_fabricate_a_report_line`).

### Fixed

- **Input caps.** 8 MiB per source file (from metadata, before the read),
  128 MiB and 20 000 files per run, 64 levels deep, and 16 MiB for a
  `--report`, `--claims`, `--declaration` or `--config` file. Every error names
  the limit (`src/project.rs`, `an_oversized_source_file_is_refused_by_name_before_it_is_read`).
- **Policy inputs must be regular files.** The 16 MiB cap alone did not bound a
  FIFO: it has length zero, so `read_to_string` blocked forever. The type is
  now checked with `symlink_metadata`, a FIFO is refused before it is opened,
  and a symlink is refused too. The check is not atomic with the open; policy
  files must come from a trusted location (`src/project.rs`,
  `a_non_regular_policy_input_is_refused_by_name`,
  `a_fifo_policy_input_is_refused_without_blocking`).
- **The trace is no longer quadratic in test count.** A suite base (state
  initializers, constructors, `setUp()` and callbacks) and the RG15 whole-file
  fee-concession fact are computed once per suite; the test-function
  duplicate check uses a set. The 5 MB / 40 000-function adversarial case is
  enabled instead of ignored, and a 12 000-function case (serialised with the
  other size tests so the measurement is stable) has a 5 s release bound and a
  120 s debug bound the old quadratic run cannot meet (`src/trace.rs`,
  `src/project.rs`, `src/engine.rs`, `tests/adversarial.rs`).
- Deleted the dead no-op context-line branch in the removed fallback applier.
- `verify-fix` config errors name the offending alias key and display paths
  with one separator style on Windows.

### Changed

- `verify_fix::Request::host` is `Option<HostPolicy>`; `None` is refused by
  `verify`. The library API is documented as unstable, and this is the change
  that puts the host decision inside the library.
- `#![forbid(unsafe_code)]` on the library and all three binaries.
- CI has an MSRV job that reads `rust-version` from `Cargo.toml` (`1.95`, the
  1.95.0 minimum) and runs `cargo test --locked` on that toolchain. The weekly,
  non-gating `cargo-mutants` job over `src/rules/`, `src/claims/`, `src/bind/`
  and `src/verify_fix.rs` lives in its own `mutants.yml` workflow with a
  schedule and `workflow_dispatch`, so the regular CI jobs do not run weekly
  (`docs/test-quality.md`).

### Tests

- `tests/verify_fix_ffi.rs`: 17 exact and alias spellings × the root, the
  `--patched` tree and the post-patch tree, each asserting exit 2 and an
  absent fake-forge marker, plus a benign control that reaches forge; a fake
  HOME whose global config enables ffi, one per spelling; the
  `fs_permissions` alias matrix (root with write and read-only values,
  `--patched`, post-patch, global); the alias global-config cases; the
  minimal top-level `FFI = true` and the `FS_PERMISSIONS` + `FFI` combination;
  a first run that rewrites the global config, asserting exit 2 and exactly
  one forge invocation; a patch that adds `.env` + an ffi config (refused); a
  clean `.env` (refused by policy); the pinned absolute `FOUNDRY_CONFIG` the
  child actually receives; a 2 MiB sparse `foundry.toml` refused before the
  read; write/read-write `fs_permissions` refused in the root, `--patched`,
  post-patch and global configs; and a read-only control that reaches forge.
- `tests/verify_fix_paths.rs`: a relative `--patch` outside `--root`, a
  relative `--forge` with a separator, and a patch that `git apply --check`
  refuses (exit 2, no forge, original tree untouched).
- `tests/verify_fix_copy_limits.rs`: a sparse tree over 512 MiB and a tree 70
  levels deep, both refused before forge starts.
- `tests/mcp_stdio.rs`: the stdio binary answers a 9 MB `Content-Length` with
  a JSON-RPC error, stops on that refusal (not on the body's line cap), does
  not serve the following ping, and exits 2.
- Adversarial: raw ESC in a Solidity string literal, a 1 GiB sparse file, a
  non-regular policy input, a FIFO policy input with a deadline (Unix), the
  enabled 5 MB case and the 12 000-function bound, with the size/timing tests
  serialised.
- Unit tests for the TOML `ffi` scan under Foundry's key normalization, the
  alias and non-boolean refusal, the host refusal, the global config,
  `fs_permissions` read-only enforcement (including non-table entries and
  non-string access), per-run revalidation of a rewritten tree/global config,
  the copy file/byte/directory/depth caps, relative forge paths, regular-file
  inputs, the source count/total/depth caps, forge timeout and stdout flood,
  bounded frames, empty-line looping, newline/bidi sanitization and claim-cell
  sanitization.
- `docs/threat-model.md` guarantees and hypotheses, and `docs/limitations.md`,
  updated to what is now tested, with the remaining unverified items marked.

## [1.0.3] - 2026-11-21

A correctness release. Five of the eight adversarial cases recorded as ignored
in 1.0.2 now pass and are enabled; the parser no longer lowers statically dead
branches or statements after an unconditional `return`/`revert`, and a claim
field named in both places is the error docs/claims.md always said it was. No
rule id, flag, default or output field changed.

### Fixed

- **Statically dead code is not lowered.** `if (false) { vm.prank(admin); }`
  used to reach the trace, so a dead prank could contradict an attacker claim
  (RG02). A branch whose condition is a boolean literal is now lowered only when
  it runs; a dynamic condition keeps both sides, as before. `while (false)` and
  `for (…; false; …)` bodies are skipped too.
- **Statements after an unconditional `return` or `revert` are dropped.** An
  assertion after `return;` could count as impact evidence and mark a drain
  claim SHOWN. A return in one branch of a dynamic `if` still falls through, so
  the branch is not treated as ending the function.
- **A claim field named both inside and outside a nested `claims:` block is an
  error.** `Raw::flatten` silently preferred the outer value; docs/claims.md
  says the duplicate is an error, and now it is.
- **A `claims.yaml` that disagrees with the report's front matter is an error.**
  The file used to win silently, so one finding could state two claim sets.
  When both parse as claims and differ, `check` exits 2 naming both sources;
  agreement is accepted.
- **Source discovery is case-insensitive on Windows and elsewhere.** `.SOL`,
  `.Rs`, `.Move` and upper-case TypeScript extensions are found, and `.d.ts`
  declaration files remain excluded.
- Simplified the `vm.label` classification in `identity.rs` (clippy's
  `nonminimal_bool`).

### Tests

- Enabled five ignored adversarial cases: the dead prank, the assertion after
  `return`, the assertion inside `if (false)`, the duplicate claim field and
  the claims/front-matter disagreement.
- New parser unit tests for statically dead branches and unreachable code.
- New claims unit tests for the duplicate-field error.
- New property test: a literal-condition branch reaches the trace exactly when
  it can run.
- New unit test for case-insensitive source extensions.

Three tracked gaps remain ignored and unchanged: a privileged prank inside a
helper contract the test calls, an address round-tripped through
`abi.encode`/`abi.decode`, and super-linear time on very large files.

## [1.0.2] - 2026-11-19

A security and fix release. RG02 now catches a prank as a contract the PoC
deploys from its own file (#17). `--changed-since` and `verify-fix` no
longer let an untrusted repository or patch run a program or write outside
the copy. New adversarial, fuzz-corpus and property tests ship with it; eight
adversarial cases the checker gets wrong today are committed as ignored tests
with their inputs (`cargo test -- --ignored`), so the gaps are on record. No
rule id, flag, default or output field changed, and the published precision
numbers are unchanged (re-measured on this build).

### Security

- **`--changed-since` no longer lets a repository's own config run a program.**
  A `.git/config` in the working tree could set `core.fsmonitor`, which git
  runs on an index refresh, and a `filter` attribute in `.gitattributes` or
  `.git/info/attributes` could name a clean filter, which git runs for a
  worktree comparison. Both ran. Every git call now goes through one helper
  that turns `core.fsmonitor` off. git is no longer asked to compare the
  worktree: realism-gate lists the blob ids at the revision (`ls-tree`) and
  hashes the current files with `hash-object --no-filters`, then compares the
  ids itself. A file that differs from the revision only in line endings is
  listed, which is checked rather than missed; a symbolic link is listed
  without being read. `tests/security.rs` plants each program and asserts it
  does not run.
- **`--target-commit` and `--changed-since` values that start with `-` are
  refused** (exit 2). Tested; this was already enforced, and is now a regression
  test.
- **`verify-fix` refuses a patch that creates a symbolic link, and a patch
  over 8 MiB.** `git apply` turns a mode `120000` entry into a link whose target
  is not checked, so a later read could leave the temp tree. The check runs
  before any apply. The size cap stops a huge patch being read whole into
  memory, with a clear error. A project symlink is not copied (already true;
  now tested), and drive-letter, verbatim and UNC spellings of an absolute path
  are tested as refused.
- **The host decision for `verify-fix` is pinned by tests.**
  `REALISM_GATE_SANDBOX=1` (and the `/.dockerenv` and `/run/.containerenv`
  markers) still count as an attestation, and `docs/verify-fix.md` says so. The
  policy is unchanged; `tests/verify_fix_limits.rs` makes a change visible.
- **Every third-party action in the workflows and `action.yml` is pinned to a
  commit SHA.** Each `uses:` line names the commit and its version in a comment
  (`# v7.0.1`), and the major version is unchanged. Two `taiki-e/install-action`
  tags are named after the tool, with no version tag on the same commit, so
  their comment says `cargo-audit tag` and `cargo-binstall tag`. Local `./`
  references are not affected.

### Fixed

- **RG02 fires on a prank as a contract the test deployed from a type in its
  own file (#17).** `vm.prank(address(vault))` and `vm.startPrank(address(vault))`
  with `vault = new Vault()`, where `Vault` is defined in the PoC file, was
  read as a helper and not as the protocol, so the attacker claim was SHOWN.
  It is now flagged, and the claim is CONTRADICTED. Three cases stay quiet: a
  contract named for the attacker, a contract the attacker deploys under
  `startPrank(attacker)`, and a hard-coded literal address (unchanged, out of
  scope). The published measurements do not change; see the PR for the
  before and after numbers.

### Documentation

- **SECURITY.md states the guarantee per command.** It said the tool "runs no
  Solidity", which was true of `check` and false of `verify-fix`, which runs
  `forge` on the PoC. The table now says what each command executes and what
  it reaches on the network.
- **`docs/threat-model.md` is new.** A short STRIDE-light model: assets,
  attackers, trust boundaries with `file:line` references, the guarantees and
  what proves each one, and the open hypotheses with their status.

### Tests

- **`tests/adversarial.rs`: PoCs and reports written to fool the checker.** It
  covers a privileged address reached through a local variable, a helper
  contract, and an `abi.encode`/`abi.decode` round trip; a prank and an
  assertion inside `if (false)` or after `return`; a claims file and front
  matter that disagree; a non-ASCII identifier; a 5 MB file; and a deeply
  nested expression. Each case states what the tool should do. Eight cases fail
  today and are `#[ignore]`d with the reason, and the inputs are listed in the
  pull request. No rule changed.
- **Fuzz targets for every parser, in `fuzz/`.** One cargo-fuzz target each
  for the Solidity, Rust, TypeScript and Move frontends, the claims YAML and
  front matter (with the prose heuristics), and `realism.yaml`. The property is
  that no target panics. The crate is excluded from the main workspace, so a
  normal build needs no nightly. Its seed corpus is copied from the test
  fixtures and examples.
- **`tests/fuzz_corpus.rs` replays the corpus on every `cargo test`.** It runs
  the same target bodies over each seed and over a fixed set of deterministic
  mutations. cargo-fuzz does not run on Windows, so this is the check that runs
  everywhere.
- **`fuzz.yml`, a weekly five-minute fuzz run per target on Linux.** It uploads
  crashing inputs as an artifact.
- **Property tests (`proptest`) in `tests/properties.rs`.** For generated inputs,
  not only the fixtures: the JSON report does not depend on the order the files
  are given; adding an unrelated test function does not change the findings of
  the tests already in the file; every JSON report validates against
  `docs/output-schema.json`, and every SARIF log has the keys SARIF 2.1.0
  requires. The schema checker moved to `tests/support/` so both tests share it.
- **`docs/test-quality.md`: line coverage and the mutant count for the rule
  code.** Whole-crate line coverage is 87.77%; the least-covered rule files are
  listed. Mutation outcomes are not recorded yet: a full `cargo mutants` run
  takes hours here, so the page says so rather than estimating a survival rate.
  Linked from `CONTRIBUTING.md`. Nothing is gated on these numbers.

## [1.0.1] - 2026-11-19

A patch release: fixes, tests and documentation. No rule, flag, default or
output field changed.

### Changed

- **Claim-binding precision is quoted with its out-of-sample number first.**
  `docs/precision.md`, `docs/limitations.md`, the README and
  `eval/claims/REPORT.md` now state 91.0% (n=100, [83.8-95.2]) for the first
  1.0.0 build, before the nine misses this corpus exposed were fixed, next to
  the 98.9% (n=91) measured afterwards on the same pairs. The 1.0.0 entry below
  quoted only the second number. No code change.

### Fixed

- **The verify-fix tests no longer share a temporary directory.** A copy of a
  project tree now gets a name that no other copy in the process can have (the
  process ID and a counter), and `create_dir` refuses an existing directory.
  The verify-fix tests run in `tempfile` directories of their own. This is the
  cause of the intermittent `The current directory is invalid.` failure of
  `a_passing_poc_means_the_fix_does_not_hold` on windows-latest: a test could
  remove a directory another test's forge was running in. The retry around
  spawning forge is kept.
- **Stale version wording in the docs.** `docs/limitations.md` and
  `docs/declaration.md` said "in v0.1" about current behaviour. The statements
  are unchanged; only the version phrase is gone.
- **Empty claims are tested.** An empty `claims.yaml` and an empty front-matter
  block are refused as "no claims" (exit 2), as before, and the tests pin that.
  A privileged prank that contradicts an "any user" attacker claim is tested
  end to end on the command line.
- **Dependencies.** serde-saphyr 2.0.0 to 2.1.1 and toml (Dependabot #15).
  The empty-input tests above pass on both versions.

## [1.0.0] - 2026-11-13

The claim release. `check` still reads the test; with `--report` it now reads
the finding too and binds each claim it makes to the evidence the PoC does or
does not contain. The output is a claim table and, on request, a reproducible
evidence card. It is still not a validity oracle, and there is still no score.

### Added

- **Claim table with `--report`.** The finding's claims are resolved from
  `--claims FILE`, then `claims.yaml` next to the report, then a YAML
  front-matter block, then heuristics labelled inferred. One row per claim
  field, statuses `SHOWN`, `NOT SHOWN`, `CONTRADICTED`, `NOT CHECKABLE`. The
  predicate behind every field is documented in
  [docs/claims.md](docs/claims.md#the-predicate-table).
- **`claims.yaml` (schema `realism-gate.claims/v1`):** `attacker`, `impact`,
  `asset`, `amount`, `preconditions`, `cited`, `severity`, `fix`. The same
  block is accepted as front matter, flat or under a `claims:` key.
- **`realism-gate init-claims FINDING [--out FILE] [--force]`:** writes a
  commented template from the report's headings and backticked identifiers.
- **`--card FILE` and `--card-json FILE`:** the evidence card, schemas
  `realism-gate.card/v1` for the JSON twin. Tool version, rule-set hash, PoC
  sha256, report sha256, claims sha256, target commit, the claim table and the
  exact rerun command. Signed by nothing; reproducible by command. See
  [docs/evidence-card.md](docs/evidence-card.md).
- **The additive JSON fields `claims` and `claim_binding`** in
  `realism-gate.report/v1`; the schema's existing fields are unchanged. SARIF
  and Markdown keep their rule findings (claims are not SARIF results).
- **MCP `check_claims`**: takes a report, an optional claims file, an optional
  PoC and `fail_on`, and returns the full report JSON, whose `claims` field is
  the table. `check_poc` is unchanged.
- **Action inputs `report`, `claims` and `card`**, and outputs
  `claim-binding`, `claims-outcome`, `card`, `card-json`.
- **`examples/rain-usdr/`**: the RAIN-USDR case study as a runnable acceptance
  fixture — the report, its structured `claims.yaml`, the four-behaviour
  Foundry test, the cited target sources, `make-target-repo.sh`, and the golden
  `card.md`/`card.json` generated by `tests/docs.rs`.
- **Claim-binding precision published** in
  [docs/precision.md](docs/precision.md#claim-binding): impact field
  98.9% on 91 flagged rows, Wilson 95% [94.0-99.8], 120 public PoC +
  incident-record pairs. Corpus, rubric and freeze in
  [`eval/claims/`](eval/claims/REPORT.md).
- Regression tests for claims parsing, binding predicates, exit codes, the
  generated README block, the golden card and schema validation of real
  structured and inferred output.

### Changed

- **`check` exit status.** `1` now also means any `CONTRADICTED` claim or an
  inferred binding. `--fail-on` does not suppress a claim failure;
  `--no-fail-on-claims` is the documented escape hatch. `2` still means a
  usage, input, configuration or declaration error.
- **The `RESULT` line is claims-aware.** A claims failure prints
  `RESULT FAIL (claims: ...)` instead of `RESULT PASS`, even with
  `--fail-on never`.
- **MCP drops `check_finding` and the `vg` shell-out.** `VG_BIN` is gone. The
  server has `check_poc` and `check_claims`.
- **The action's floating tag target is `v1`**, and the action's default
  version is `v1.0.0`. The `v0` tag stays on the last 0.x release.
- **`combined/action.yml` and `examples/workflows/gates.yml` are deleted.**
  The combined action ran `viability-gate`; there is no cross-repo pin left.
- **`RELEASING.md` records the 1.0 criteria** (schema v1, stable rule ids,
  published claim-binding precision, crates.io equal to the GitHub release,
  Action tag `v1`) and has a checklist step for the precision page.

### Unchanged

- **Rule ids RG01-RG20, their slugs, severities and semantics.** No rule was
  added, removed, renamed or re-graded in 1.0.0.
- **JSON schema 1 and SARIF for rule findings**, additive fields aside.
- **`check` never runs a test, never builds one and never touches the
  network.** `verify-fix` is still the only command that runs forge.
- **No score.** Four claim statuses and a table, not a number.

### Not claimed

- **The 98.9% is one field.** It is impact (drain) on 91 flagged rows of 120
  public PoC + incident-record pairs. The corpus labels are a frozen mechanical
  rubric, not truth. The attacker field is **not interpretable** with those
  labels, so no attacker precision is claimed; `amount` is `NOT CHECKABLE` by
  design. There is no recall number and no overall-outcome number.
- **The three realism-gate-killed rejection rows (R-034/35/36)** are pointers
  plus expected status only. Their inputs are private and are not in this
  repo, so they are not run here.
- **A card is not a proof.** It binds files; it does not sign them, date them,
  or say the bug is real or payable.
- **A non-fix `NOT SHOWN` row does not by itself exit non-zero** in 1.0.0. The
  outcome says `INCOMPLETE`; gate on `claims.outcome` from the JSON if you need
  every claim shown.
- **A `SHOWN` row is not a validity verdict.** It means the evidence the
  predicate looks for is present, and no more.

## [0.6.1] - 2026-11-10

A packaging patch. 0.6.0 was tagged and its release workflow failed, so no
0.6.0 GitHub release exists; 0.6.1 ships the same checker with the archive
fixed. The v0.6.0 tag is not moved or deleted.

### Fixed

- **The release archives carry all three declared binaries.** 0.6.0's archives
  contained `realism-gate` and `rgate` but not `realism-gate-mcp`. The release
  run [42700992290](https://github.com/KhomDev/realism-gate/actions/runs/42700992290)
  passed `verify`, all five `build` jobs and `checksums`, then failed all three
  `install test` jobs with
  `cargo-binstall: ERROR resolve: When resolving realism-gate bin realism-gate-mcp is not found`;
  `publish the release` was skipped. `cargo binstall` installs every binary
  a crate declares, so an archive missing one is not installable. The build
  step now reads the binary list from `Cargo.toml` (`scripts/declared-bins.sh`)
  and packages every entry.
- **The install check asserts the binaries.** `scripts/install-check.sh` reads
  the same list from `Cargo.toml` and fails the run if an archive, or an
  install from it, is missing a declared binary. A dry run of the release
  workflow now catches this class of failure before a tag.
- The release notes said "Two binaries". They now name all three.

### Added

- **[RELEASING.md](RELEASING.md)**: the published cadence and compatibility
  policy. Patch releases for fixes only; at most one minor per week; a minor
  needs a named milestone and a changelog entry; the 1.0 criteria; the
  mandatory `workflow_dispatch` dry run before a tag; a published tag is
  never moved. Linked from the README and CONTRIBUTING.

### Changed

- `action.yml` and `combined/action.yml` default to `v0.6.1`, so `@v0` runs
  this release. `tests/docs.rs` keeps that equal to the crate version.

## [0.6.0] - 2026-11-09

The gate review. The checks are still a PoC-structure filter, not a validity oracle. What changed is the bypasses that filter used to miss, and the ways a repo under review could turn the filter off.

### Added

- **RG04** now flags `stdstore` (`checked_write` / `write`), `setArbitraryStorage` and `copyStorage` the same way as `vm.store`. A direct storage write is an error in the exploit and in `setUp()`.
- **RG09** flags `assertGt(balance + 1, before)` and the same shape through a local. The comparison passes when the balance did not fall.
- **RG19 `setup-hides-storage-write`.** `setUp()` pranks as a role and also writes protocol storage. Moving both out of the test body is an error.
- **RG20 `missing-citation`.** With `--target` and `--target-commit`, import paths, contract names and called functions have to exist in that tree at that commit. The check reads git and does not run the protocol. Without those flags it does not run.
- **`verify-fix` is two-sided.** The PoC must pass on a copy of the unpatched tree and fail on the patched tree. A PoC that fails on both sides is not reported as a fix. The verdict is forge's JSON plus its exit code.
- Regression fixtures for each of those bypasses.

### Changed

- Inline `realism-gate: allow` comments are ignored unless `--trust-inline-allows`. A `realism.yaml` or `realism-gate.toml` inside the repo under review is not applied. `--declaration` and `--config` have to point outside that tree.
- `verify-fix` refuses a patch path that is absolute or contains `..`, refuses `foundry.toml` with `ffi = true`, and refuses to run on the host unless `--sandbox` or `--allow-host`. See `docs/verify-fix.md`.
- The MCP server rejects a `path`, `poc`, `finding` or `declaration` that starts with `-`, puts paths after `--`, and does not load config or a declaration from the working tree. Tool output is fenced as data.
- The GitHub Action splits `args` without globbing and passes paths after `--`.
- RG02's fix text no longer says to move a privileged prank into `setUp()`.

### Not a validity oracle

A clean `check` still means no known pattern fired. `verify-fix` still only compares one forge test before and after a patch.

## [0.5.0] - 2026-11-09

Economic realism, less RG03 noise, Move tests, and an opt-in way to run the PoC against a patch. `check` still does not execute a test and still has no network code.

### Added

- **RG15 `profit-ignores-costs`.** A profit assertion that does not subtract a fee, slippage or gas cost the test itself computes, or that a sibling test shows disappears once a fee applies.
- **RG16 `implausible-amount`.** `deal` / `mint` of at least 10^30 raw units, or at least 10^27 on a fork pinned to a block. The bounds are fixed. The chain is not read.
- **RG17 `unevidenced-setup`.** `ship`, `dock`, `push`, or a stand-in deployment, with no `deployment.evidence` entry in `realism.yaml` for that contract or actor. `source` is required. The tool does not read protocol docs.
- **RG18 `revert-only`.** The only assertion is `vm.expectRevert`, or a Move `#[expected_failure]` with no `assert!`.
- **Move tests** (Sui `test_scenario` and Aptos `#[test(signer)]`): RG01, RG02, RG06, RG08, RG10, RG18. Not a compiler.
- **`verify-fix`.** Opt-in. Applies a patch to a copy of the tree, or uses a patched directory, runs `forge test`, and requires the PoC to fail. A compile error is not a failed PoC.
- **Combined action** (`combined/action.yml`) that runs realism-gate and `vg check`, writes one SARIF file and one pull-request comment. Example workflow in `examples/workflows/gates.yml`.
- **Agent integration.** `skills/realism-gate/SKILL.md` and `realism-gate-mcp` (`check_poc`, `check_finding`). Install notes in `docs/agents.md`.
- A vhs tape at `docs/demo.tape`.

### Changed

- **RG03** is a warning only when the deployer (or the Solana setup key) calls something named like an access-controlled function. The same pattern on an ordinary call is a note. On the six review PoCs that used to raise 9 RG03 warnings, all 9 are now notes.
- `deployment.evidence` in `realism.yaml` (version stays 1). An entry silences RG17 for that name.

## [0.4.0] - 2026-11-06

Anchor. realism-gate now checks Anchor's TypeScript proof-of-concept tests,
the mocha or jest suites `anchor test` runs, and the two Anchor test crates
in Rust, with the same rule ids, output and suppressions. The Solidity and
Rust findings, and the JSON/SARIF output, are unchanged.

### Added

- **Anchor TypeScript tests.** `.ts`, `.mts`, `.cts`, `.js`, `.mjs` and
  `.cjs` files that import a Solana package (`@coral-xyz/anchor`, anything
  under `@solana/`, bankrun, LiteSVM) are parsed with oxc. Other JavaScript
  in the tree, such as a Hardhat suite, is not read. Each `it(..)` /
  `test(..)` is checked when it drives a harness: an Anchor program client,
  web3.js, bankrun or LiteSVM.
- Mocha's order is part of the trace: the file's top level, `describe`
  bodies, `before` hooks and the tests above a test run before it, as mocha
  runs them, then `beforeEach`. A test whose title says it sets up
  (`it("is initialized")`) is the setup of the tests after it, not a PoC of
  its own. Helpers (functions, arrow functions bound to a name, class
  methods) are inlined from the test's file and TypeScript files under
  `tests/`.
- The TypeScript forms of the Solana rules: `.methods.x(..).signers([..]).rpc()`,
  the legacy `program.rpc.x(.., { accounts, signers })`, `sendAndConfirm`
  and `sendAndConfirmTransaction` as transactions with their signers; a role
  field in the `.accounts({ .. })` of an initializing instruction as a grant
  (RG01); `setAccount` and the accounts and programs handed to bankrun's
  `startAnchor` as writes and loads (RG04-RG06); chai, node and jest
  assertions (RG08-RG10); `new Connection(url)` as a live cluster (RG11).
- Anchor's provider wallet signs every `.rpc()` as the fee payer. It counts
  as a signer only when it is the only one or the instruction names it in
  its accounts, so a wallet that is the admin does not make every attacker
  transaction it paid for a privileged one (RG02), and an exploit signed by
  the wallet alone is RG03.
- **Rust:** anchor-client's `program.request()` .. `.signer(&kp)` .. `.send()`
  is a transaction signed by its signers and the client's payer, and
  `Client::new` for a public cluster is a live client (RG11); anchor-litesvm's
  `AnchorLiteSVM::build_with_program` and `execute_instruction(ix, signers)`
  are a program load and a transaction; an `assert_*` helper method
  (`svm.assert_token_balance(..)`) is an assertion.
- Every rule page and `realism-gate explain` show a TypeScript example next
  to each Rust one. `tests/typescript.rs` runs them and fails if one does not
  do what its page says.
- [`examples/anchor-vault-drain`](examples/anchor-vault-drain/tests/vault-drain.ts),
  the Rust example's PoC in Anchor TypeScript, with its output in the
  README.

### Changed

- The Solana pipeline moved from `src/rust/` to `src/solana/`, with the Rust
  frontend in `parse_rust.rs` and the TypeScript one in `parse_ts.rs`. The
  library API is not stable; the CLI and outputs are unaffected.
- `realism-gate rules --format json` lists `typescript` in the `languages`
  of every rule that checks Solana tests.
- `--match-contract` also matches TypeScript suite names, the `describe`
  titles (`vault > withdraw`), which are also the `suite` field.
- RG04 and RG06 name the method as the test spells it (`setAccount`).
- An account owned by the token program is read as a token account (RG06),
  and a value read through a cluster client's `getAccountInfo` or loaded
  with `readFileSync` / `JSON.parse` as copied state, not a write (RG04).
- RG09 no longer reports `assert!(false)` / `assert.ok(false)`, which always
  fails, nor a local that is assigned different values (a flag set in a
  callback).
- RG11 reads a URL held in a local or constant, so `new Connection(ENDPOINT)`
  with `ENDPOINT = "http://localhost:8899"` is local.
- RG10 treats calls that differ only in string arguments
  (`balances(payer, "before")`, `balances(payer, "after")`) as one reading
  taken twice.
- The Action's default release, the docs and the examples point at `v0.4.0`.

### Not covered

- RG07 is not checked in TypeScript either. `Anchor.toml`, tests generated
  in loops, and the signers of spl-token's JavaScript helpers are not read;
  the full list is in
  [docs/limitations.md](docs/limitations.md#in-anchor-typescript-tests).

### Measured

- The precision of the TypeScript rules is **not measured**, as for Rust. As
  a robustness check, Anchor (`33868ff`), solana-developers/program-examples
  (`c0cafc5`) and LiteSVM's Node tests (`c848c56`): 229 files, 292 tests,
  all parsed; reading their findings found five false-positive patterns,
  fixed and pinned by tests
  ([docs/precision.md](docs/precision.md#anchor-typescript-tests)).
- Rust: the LiteSVM and solana-program-library findings are identical to
  0.3.0's. Solidity: `scripts/bench-corpus.sh --check` matches
  `bench/defihacklabs.json`, and `eval/run.sh --check` reproduces the flags
  and precision tables on all three corpora unchanged.
- Release binary on the same Windows x86-64 laptop: 6,497,280 -> 7,243,776
  bytes (+11.5%), the TypeScript parser.
- The minimum supported Rust stays 1.95: oxc is pinned to 0.146, its last
  release that builds on it.
- One new licence in the dependency graph: `dragonbox_ecma`, through oxc, is
  under `Apache-2.0 WITH LLVM-exception OR BSL-1.0`. `deny.toml` allows
  BSL-1.0 for that crate alone.

## [0.3.0] - 2026-11-06

Solana programs. realism-gate now checks Rust proof-of-concept tests written
with solana-program-test, LiteSVM or Mollusk, with the same rule ids, the same
output and the same suppressions. The Solidity checks, findings and JSON/SARIF
output are unchanged.

### Added

- **Rust (Solana) tests.** `.rs` files are parsed with `syn`. A `#[test]`
  (`#[tokio::test]`, `#[rstest]`, `#[test_case]`, ...) is checked when it
  drives a Solana harness: it builds a `ProgramTest`, `LiteSVM` or `Mollusk`,
  builds or sends a transaction, or writes, loads or airdrops to an account.
  Other Rust tests in the tree are skipped and not counted.
- The Rust trace inlines helpers from the test's file and from shared test
  code (any `.rs` under a `tests/` directory, such as `tests/common/mod.rs`),
  binding each helper's parameters to the caller's arguments, so
  `send(&mut svm, ix, &[&admin])` is seen as signed by `admin`. Program
  source is never inlined.
- Ten code rules check Rust tests, RG01-RG06 and RG08-RG11, and the
  declaration rules RG12-RG14 apply unchanged. In a Solana test: RG01, the
  attacker is made `admin` or `authority` by an initializing instruction or
  `set_authority`; RG02, an exploit transaction is signed by a role holder;
  RG03, the exploit is signed only by the key that set the program up; RG04,
  `set_account` writes a program account; RG05, a mock program is loaded at a
  real program's address; RG06, `set_account` sets a program account's
  lamports or token balance; RG08, no assertion (`.unwrap()` on a transaction
  is not one); RG09, an assertion that cannot fail; RG10, a gain asserted
  without a before-value; RG11, accounts read from a live `RpcClient`.
- A transaction finding is placed at the call that sends it in the test, not
  inside a generic `send` wrapper, and quotes that call.
- Every rule page and `realism-gate explain` have an "In Rust (Solana) tests"
  section with the Rust fix, examples and false-positive modes. A Rust
  finding's `help` is the Rust fix. `tests/rust.rs` runs each page's Rust
  examples and fails if one does not do what the page says.
- `realism-gate rules --format json` lists each rule's `languages`.
- [`examples/solana-vault-drain`](examples/solana-vault-drain/tests/vault_drain.rs),
  with its output in the README, regenerated by `tests/docs.rs`.

### Changed

- Directories are searched for `.sol` and `.rs` files. Rust files that drive
  no Solana harness change nothing: the same Foundry run reports the same.
- Without paths, `check` reads `./test`, then `./tests` (Cargo's), then `.`.
- `--match-contract` also matches Rust suite names: the file stem and module
  path (`exploit`, `lib::tests`), which is also the `suite` field.
- The Action's default release, the docs and the examples point at `v0.3.0`.

### Not covered

- RG07 (mocked call) is not checked in Rust: Solana harnesses have no call
  mocking. The limits of the Rust frontend are listed in
  [docs/limitations.md](docs/limitations.md#in-rust-solana-tests).

### Measured

- The precision of the Rust rules is **not measured**: there is no labelled
  corpus of Solana PoCs yet, and the 0.2.0 precision figures are Solidity
  only. As a robustness check, LiteSVM (`c848c56`) and five
  solana-program-library test suites (`264ca72`), 164 Rust files and 451
  tests, all parsed and ran in under a second per repository; reading their
  findings found four false-positive patterns, fixed and pinned by tests
  ([docs/precision.md](docs/precision.md#rust-solana-tests)).
- DeFiHackLabs: `scripts/bench-corpus.sh --check` matches
  `bench/defihacklabs.json` unchanged.
- Release binary on the same Windows x86-64 laptop: 4,940,288 -> 6,497,280
  bytes (+31.5%), the Rust parser.

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

[Unreleased]: https://github.com/KhomDev/realism-gate/compare/v1.1.0...HEAD
[1.1.0]: https://github.com/KhomDev/realism-gate/compare/v1.0.4...v1.1.0
[1.0.4]: https://github.com/KhomDev/realism-gate/compare/v1.0.3...v1.0.4
[1.0.3]: https://github.com/KhomDev/realism-gate/compare/v1.0.2...v1.0.3
[1.0.2]: https://github.com/KhomDev/realism-gate/compare/v1.0.1...v1.0.2
[1.0.1]: https://github.com/KhomDev/realism-gate/compare/v1.0.0...v1.0.1
[1.0.0]: https://github.com/KhomDev/realism-gate/compare/v0.6.1...v1.0.0
[0.6.1]: https://github.com/KhomDev/realism-gate/compare/v0.6.0...v0.6.1
[0.6.0]: https://github.com/KhomDev/realism-gate/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/KhomDev/realism-gate/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/KhomDev/realism-gate/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/KhomDev/realism-gate/compare/v0.2.1...v0.3.0
[0.2.1]: https://github.com/KhomDev/realism-gate/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/KhomDev/realism-gate/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/KhomDev/realism-gate/releases/tag/v0.1.0
