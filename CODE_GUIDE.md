# CODE_GUIDE: the realism-gate codebase, line by line

This document is the technical companion to the [README](README.md). The README
says what the tool is for; this says how the code works, where each behaviour
lives, and which numbers are measured rather than asserted. It describes the
tree exactly as tagged **v0.4.0**, commit `c65845bc46da0670a17fe16cf22e6911f4e7c03f`. 0.5.0 adds RG15-RG18, the RG03 note, Move tests, `verify-fix` and the MCP server; those are in the changelog, not in the line numbers below.
(the commit the annotated tag `v0.4.0` dereferences to). In one line: a
static checker that reads Foundry (Solidity) and Solana (Rust, Anchor
TypeScript) proof-of-concept tests and flags the patterns that make a passing
PoC prove less than it claims.

Everything below was checked against the v0.4.0 source in this repository.
Line references are to files at that commit. Where a count is given (rules,
tests, fixtures, CI contexts), it was counted from the source, not copied from
the documentation.

## Contents

1. [The crate and its two binaries](#1-the-crate-and-its-two-binaries)
2. [Command-line surface](#2-command-line-surface)
3. [File discovery and the owned model](#3-file-discovery-and-the-owned-model)
4. [The three parsers and how they are wired](#4-the-three-parsers-and-how-they-are-wired)
5. [The trace: what runs, in what order, in which phase](#5-the-trace-what-runs-in-what-order-in-which-phase)
6. [The Solana pipeline](#6-the-solana-pipeline)
7. [Identity: who is who in a test](#7-identity-who-is-who-in-a-test)
8. [The engine: findings, merge semantics, summary](#8-the-engine-findings-merge-semantics-summary)
9. [The rule set (14 rules)](#9-the-rule-set-14-rules)
10. [Suppressions, configuration, declarations](#10-suppressions-configuration-declarations)
11. [Renderers](#11-renderers)
12. [The changed-since git integration](#12-the-changed-since-git-integration)
13. [The GitHub Action](#13-the-github-action)
14. [Measurement infrastructure](#14-measurement-infrastructure)
15. [Tests](#15-tests)
16. [CI, release and verification pipelines](#16-ci-release-and-verification-pipelines)
17. [Known limitations at v0.4.0](#17-known-limitations-at-v0.4.0)

## 1. The crate and its two binaries

`Cargo.toml` declares one package, `realism-gate`, version 0.4.0, edition
2024, MSRV `rust-version = "1.95"` (Cargo.toml:3-12). The published crate
carries only the program and its licence: the `include` list (Cargo.toml:13-15)
is `src/**/*.rs`, the manifest, `Cargo.lock`, `README.md`, `LICENSE` and
`CHANGELOG.md`; the CI `package` job enforces exactly that (see
[section 16](#16-ci-release-and-verification-pipelines)).

Two binary targets are built from three-line mains
(src/bin/realism-gate.rs:1-3, src/bin/rgate.rs:1-3); both call
`realism_gate::cli::main()`, so `realism-gate` and `rgate` are the same
program (asserted by tests/cli.rs:270-277).

The parser dependencies are the load-bearing choices (Cargo.toml:26-45):

- **Solidity**: `solar-parse` 0.2.0, the parser written for the Foundry
  toolchain.
- **Rust**: `syn` 2.0.119 (default features off, `full`/`parsing`/`printing`
  on) plus `proc-macro2` with `span-locations`, which is what gives every
  node a line, column and byte range.
- **TypeScript**: the Oxc parser, pinned exactly (`=0.146.0` for
  `oxc_allocator`, `oxc_ast`, `oxc_parser`, `oxc_span`) because oxc breaks
  its API on every minor release and 0.146 is the last that builds on the
  1.95 MSRV (Cargo.toml:28-34).
- `clap` (derive) for the CLI, `serde`/`serde-saphyr` (YAML)/`toml` for the
  declaration and config files, `serde_json` for the JSON output, `regex`
  for filters, globs and suppression comments.

The release profile is `opt-level = 3`, LTO, one codegen unit, stripped
(Cargo.toml:47-51). `[package.metadata.binstall]` (Cargo.toml:56-63) spells
out the release archive layout that `cargo binstall` reads: a `.tar.gz` per
target, a `.zip` for `x86_64-pc-windows-msvc`; the release workflow's dry
run installs through this metadata (.github/workflows/release.yml:194-206).

`src/lib.rs` (35 lines) is the module list and the public API surface:
`engine::{Options, Report, analyze}`, `finding::{Finding, Severity}` and
`project::Project` are re-exported (src/lib.rs:33-35). The doc comment
states the contract: the API is not yet stable; the JSON and SARIF outputs
are the supported interface (src/lib.rs:14-15).

### Module map

| Module | Lines | What it does |
|---|---:|---|
| `src/cli.rs` | 504 | Subcommands, flags, exit codes, orchestration of one `check` run |
| `src/project.rs` | 335 | File discovery, loading, contract resolution and linearization |
| `src/ir.rs` | 214 | The owned, parser-independent expression/statement model |
| `src/parse.rs` | 542 | solar AST to owned model (Solidity frontend) |
| `src/trace.rs` | 555 | Solidity execution trace: phases, call classification, inlining |
| `src/identity.rs` | 746 | Solidity identity classification and privilege evidence |
| `src/rules/mod.rs` | 208 | Rule plumbing: `Raw`, sender replay, `check_test` dispatch |
| `src/rules/catalog.rs` | 536 | The rule catalog, single source of rule metadata |
| `src/rules/privilege.rs` | 291 | RG01-RG03 (who the exploit runs as) |
| `src/rules/injection.rs` | 211 | RG04-RG07 (state the PoC creates) |
| `src/rules/evidence.rs` | 651 | RG08-RG11 (whether the test demonstrates anything) |
| `src/engine.rs` | 311 | Runs rules, merges findings, assembles the report |
| `src/finding.rs` | 73 | `Finding`, `Severity`, `Suppression` |
| `src/suppress.rs` | 123 | Inline `allow` comment directives |
| `src/config.rs` | 179 | `realism-gate.toml` and the `--severity`/`--exclude` flags |
| `src/declaration.rs` | 318 | `realism.yaml`: parsing and the RG12-RG14 checks |
| `src/changed.rs` | 55 | `--changed-since`: the two git invocations |
| `src/render/mod.rs` | 17 | Shared constants (`TOOL`, `VERSION`, rule URLs) |
| `src/render/text.rs` | 226 | Human output |
| `src/render/markdown.rs` | 153 | Job-summary / PR-comment output |
| `src/render/json.rs` | 29 | JSON, schema 1 |
| `src/render/sarif.rs` | 71 | SARIF 2.1.0 |
| `src/solana/mod.rs` | 114 | Solana suite discovery and the per-test gate |
| `src/solana/ir.rs` | 109 | The Rust/TypeScript function model |
| `src/solana/parse_rust.rs` | 801 | syn AST to owned model (Rust frontend) |
| `src/solana/parse_ts.rs` | 1816 | oxc AST to owned model (TypeScript frontend) |
| `src/solana/trace.rs` | 772 | Solana trace with parameter binding |
| `src/solana/harness.rs` | 396 | Recognised harness calls mapped to semantic events |
| `src/solana/env.rs` | 650 | Solana identity classification |
| `src/solana/rules.rs` | 1325 | The Solana rule implementations (RG01-RG06, RG08-RG11) |

The two `src/bin/*.rs` files are 3 lines each. There are 33 `.rs` files
under `src/` (counted from the tree).

## 2. Command-line surface

`src/cli.rs` defines the interface. Exit codes are a contract stated in the
module doc: 0 when no active finding reaches `--fail-on`, 1 when one does,
2 on a usage, input or declaration error (src/cli.rs:1-4).

`Command` (src/cli.rs:39-62) has four subcommands:

- `check` — the analysis; with no path arguments it defaults to `./test` if
  that directory exists, else `./tests`, else the current directory
  (src/cli.rs:228-237; the comment: "Foundry keeps tests in test/, Cargo in
  tests/").
- `rules` — list all rules, `--format text|json` (src/cli.rs:44-47).
- `explain` — one rule's full page by id or slug; `--markdown` prints the
  page published under `docs/rules/` (src/cli.rs:48-55, 182-191).
- `init` — write a commented `realism.yaml` template; refuses to overwrite
  without `--force` (src/cli.rs:56-61, 192-204).

`CheckArgs` (src/cli.rs:64-120) is the flag surface: `--declaration` /
`--no-declaration` (mutually exclusive; default: `./realism.yaml` if
present, src/cli.rs:251-257), `--format text|json|sarif|markdown`,
`--output`, `--fail-on error|warn|note|never` (default `error`), `--rules`
and `--skip` (comma-separated ids or slugs, resolved at src/cli.rs:208-218),
`--match-test` and `--match-contract` regexes, `--show-suppressed`,
`--config` / `--no-config` (default: `./realism-gate.toml`), `--severity
RULE=LEVEL` entries, `--exclude` globs, `--changed-since REF`, and
`--color auto|always|never`. Colour auto is on only for a terminal stdout
with no `NO_COLOR` in the environment (src/cli.rs:346-351).

Precedence is explicit: a rule named in `--rules` runs even if the config
file turns it off, and `--severity` replaces a level the file sets
(src/cli.rs:294-315). `--exclude` flags add to the config's `exclude`
(src/cli.rs:316-321).

The end of a run enforces one distinction: "nothing checked is not the same
as nothing found." If `report.summary.suites == 0` the run is an error
(exit 2), with a message that separates "no concrete contract with a public
test* function, and no Rust or TypeScript test that drives a Solana harness"
from parse errors (src/cli.rs:376-388).

`explain` text output (src/cli.rs:439-504) prints the rule's summary, why,
fix, fires-on / does-not-fire-on examples, known false-positive modes, the
Solana section, and the documentation URL. The `rules` text output is one
line per rule: id, severity, slug, title (src/cli.rs:397-411); the JSON
output adds `severity_note`, `summary`, `languages` (`["solidity"]` or
`["solidity", "rust", "typescript"]` per `checks_solana()`,
src/cli.rs:422-428) and `url`.

## 3. File discovery and the owned model

`project::discover` (src/project.rs:40-55) walks each given path; a path
given explicitly is always included, even inside a skipped directory. The
walk (src/project.rs:57-76) skips directories named `lib`, `node_modules`,
`out`, `cache`, `broadcast`, `target`, `artifacts` (`SKIP_DIRS`,
src/project.rs:10-18) and any hidden directory, and collects files matching
`is_source` (src/project.rs:78-80): `.sol`, `.rs`, or a script path the
TypeScript frontend claims (see
[section 4](#4-the-three-parsers-and-how-they-are-wired)). A missing path is
a usage error (src/project.rs:42-44). Results are sorted and de-duplicated
(src/project.rs:52-54).

Every path is displayed relative to the working directory with forward
slashes, so output never carries an absolute host path (`display_path`,
src/project.rs:84-91; asserted by tests/rules.rs:432-445).

`parse_any` (src/project.rs:96-104) picks the frontend by extension: `.rs`
to `solana::parse_rust::parse_source`, a script path to
`solana::parse_ts::parse_source` (which may return `None` for a script that
imports no Solana package), everything else to the Solidity parser.

`src/ir.rs` is the owned model all three frontends lower into. The design
reason is stated in its doc: the parsers' ASTs borrow from arenas that live
only as long as a parse session, and rules need several files at once (a
test suite and the base contracts it inherits), so each file is lowered
once and the arena dropped (src/ir.rs:1-8). The model keeps only what the
rules read: names, call structure, assignments, literals, source text and
positions (src/ir.rs:8-9).

Key types:

- `Pos` — 1-based line and column (src/ir.rs:10-15).
- `Expr` / `ExprKind` (src/ir.rs:17-51): identifiers, member accesses, calls
  (with `{value: .., gas: ..}` call options), `new T`, literals, indexing,
  binary/unary/assignment/ternary, tuples, arrays, type expressions, and an
  `Other` bucket whose sub-expressions are kept "so that calls nested inside
  it are still visited" (src/ir.rs:48-50).
- `Stmt` / `StmtKind` (src/ir.rs:105-132): statements are flattened — the
  bodies of `if`, `for`, `while`, `try` and nested blocks appear inline, in
  source order, because "the rules reason about what a test does, not about
  which branch runs" (src/ir.rs:111-114).
- `Function` (src/ir.rs:144-154) with `FnKind` (constructor, function,
  modifier, fallback, receive), visibility, params, modifiers, body.
- `Contract` (src/ir.rs:183-190) with `ContractKind`; `Unit`
  (src/ir.rs:203-214) adds `source`, the lowered Solana file
  (`solana: Option<...>`) and parser `errors` — "a file with errors may
  still yield contracts: the parser recovers, and whatever it recovered is
  checked" (src/ir.rs:211-213).

Suites and inheritance (src/project.rs:107-266): a suite is a concrete
contract with at least one public or external function whose name starts
with `test` (`is_test_function`, src/project.rs:270-275; invariant and fuzz
entry points are out of scope). Base contracts are resolved by name with a
same-file-first preference (src/project.rs:182-199) — "many PoC collections
reuse one suite name in every file, so the same-file preference is what
keeps them apart." The linearization is depth-first, most derived first
(src/project.rs:201-219), which "approximates Solidity's linearization
closely enough to find the most derived definition" of any function and to
resolve `super.name()` (src/project.rs:143-149).

## 4. The three parsers and how they are wired

### Solidity: `src/parse.rs`

`parse_source` (src/parse.rs:19-56) builds a solar `Session` with a buffer
emitter and no colour, parses the file, and lowers it through `Lower`
(src/parse.rs:98-433). It never panics on malformed input; parse errors are
collected in `Unit::errors`. Rendered diagnostics are reduced to
`message (line N, column M)` by `summarize_diagnostics`
(src/parse.rs:64-96), which deliberately strips source excerpts and notes
naming the parser's own build-host files — "neither belongs in a report
that may be shared" (src/parse.rs:58-63).

The lowering keeps call options (`{value: 1}`) as named options
(src/parse.rs:342-359), lowers `payable(x)` to a cast-shaped call
(src/parse.rs:416-424), and flattens control flow in source order
(src/parse.rs:259-321). Five unit tests pin the behaviour, including
"modifiers and placeholders are kept" and "control flow is flattened in
source order" (src/parse.rs:435-541).

### Rust: `src/solana/parse_rust.rs`

`parse_source` (src/solana/parse_rust.rs:21-43) strips a byte-order mark,
parses with `syn::parse_file`, and invalidates proc-macro2's thread-local
span map after each file so positions never leak between files
(src/solana/parse_rust.rs:33-35; tested at
src/solana/parse_rust.rs:794-800). A file that does not parse yields no
functions and one error entry.

Items are walked (src/solana/parse_rust.rs:146-194): free functions, `impl`
methods (owner recorded), nested modules (module path recorded), and
`const`/`static` items with their values. A function is a test when an
attribute's last segment is `test`, `rstest` or `test_case`
(src/solana/parse_rust.rs:227); `#[should_panic]` is recorded separately
(src/solana/parse_rust.rs:228).

The lowering maps Rust onto the shared model (the mapping table is
src/solana/ir.rs:6-18): paths become identifiers, `a.f(x)` a member call, a
macro (`assert_eq!(a, b)`) a call whose callee is `assert_eq!` with the
macro's comma-separated expressions as arguments
(src/solana/parse_rust.rs:486-525), and a struct literal a call whose named
options are the fields. `if cond { panic!(..) }` with no else is lowered to
`assert!(!cond)` (src/solana/parse_rust.rs:571-598) — it "fails the test
exactly when `cond` holds" — with `panic`, `unreachable`, `todo` and
`unimplemented` as the failing macros (src/solana/parse_rust.rs:17).

### TypeScript: `src/solana/parse_ts.rs`

Only files that import a Solana package are read. `parse_source`
(src/solana/parse_ts.rs:84-114) first applies a cheap text filter
(`MARKERS`: `@solana/`, `anchor`, `bankrun`, `litesvm`;
src/solana/parse_ts.rs:54), parses with oxc, records up to three parse
errors with positions, and returns `None` unless an import or `require`
comes from a Solana package (`is_solana_package`, src/solana/parse_ts.rs:57-69:
`@solana/*`, `@coral-xyz/anchor`, `@project-serum/anchor`,
`solana-bankrun`, `anchor-bankrun`, `litesvm`, `anchor-litesvm`). Claimed
extensions are `.ts .mts .cts .js .mjs .cjs`, never `.d.ts`
(src/solana/parse_ts.rs:76-79). The effect — a Hardhat suite in the same
tree is not read — is asserted by tests/typescript.rs:27-40.

The lowering (documented at src/solana/parse_ts.rs:12-42) spells TypeScript
the way the Rust frontend spells Rust, so everything after the parser is
shared between the two Solana dialects:

- mocha structure is structure, not calls: each `it`/`test`/`specify`
  callback is a test (including `.only`); `describe`/`context`/`suite` open
  blocks; `before`/`beforeAll`/`beforeEach` are hooks; `after*`, `xit`,
  `xdescribe`, `.skip` and `.todo` are ignored (`mocha_kind`,
  src/solana/parse_ts.rs:159-170).
- called names are snake-cased (`setAccount` becomes `set_account`,
  src/solana/parse_ts.rs:118-144); calls on imported names or classes become
  path calls (`anchor.web3.Keypair.generate()` becomes
  `anchor::web3::Keypair::generate`, src/solana/parse_ts.rs:418-431);
  `new T(..)` becomes `T::new(..)`; `this` becomes `self`.
- `kp.publicKey` becomes `kp.pubkey()`; `new PublicKey("..")` becomes
  `PublicKey::from_str("..")`, a fixed address
  (src/solana/parse_ts.rs:1184-1214); `getAssociatedTokenAddress(mint, owner)`
  swaps its arguments to Rust's owner-first order
  (src/solana/parse_ts.rs:1372-1377).
- `await`, `as`, `!`, `new BN(x)`, `BigInt(x)`, `Number(x)` and the
  conversion methods (src/solana/parse_ts.rs:328-331, 1362-1369) are their
  operand; an object literal is a struct literal with no type name
  (src/solana/parse_ts.rs:1158-1182); `===`/`!==` become `==`/`!=`
  (src/solana/parse_ts.rs:1038-1049).
- chai/node/jest assertions lower to the Rust assertion macros:
  `assert.equal(a, b)` and `expect(a).to.equal(b)` become `assert_eq!(a, b)`,
  `expect(a).to.be.greaterThan(b)` becomes `assert!(a > b)`, assertions that
  do not compare become `assert_that!`, and `if (c) throw` becomes
  `assert!(!c)` (`assertion`, src/solana/parse_ts.rs:1238-1325; pinned by
  the shapes test at src/solana/parse_ts.rs:1694-1752). BN comparisons
  (`a.gt(b)`) become operators (src/solana/parse_ts.rs:317-326).
- `console.*` becomes `println!` (src/solana/parse_ts.rs:1332-1336).
- The program and account lists handed to bankrun's
  `startAnchor`/`start` become one `add_program`/`set_account` call per
  entry (`bankrun_entries`, src/solana/parse_ts.rs:1395-1445).

What runs before a test is recorded in `Function::before` and rebuilt in
`Lower::before` (src/solana/parse_ts.rs:472-491): the file's top level, the
enclosing `describe` bodies, the `before` hooks, every earlier test ("mocha
runs them first and the test inherits their state"), then the `beforeEach`
hooks. A test whose title says it sets up (`it("is initialized")`,
`it("funds the vault")`) is marked as the setup of the tests after it
(`is_setup_title`, src/solana/parse_ts.rs:251-279, stem list at
src/solana/parse_ts.rs:252-265).

## 5. The trace: what runs, in what order, in which phase

`src/trace.rs` builds, for one Solidity test, the ordered list of what runs.
The doc states the order (src/trace.rs:1-9): state-variable initializers
and constructors, then `setUp()`, then the test function (helpers inlined at
the call site, "so a prank inside `_attack()` is seen exactly where it
happens"), and externally callable functions that are neither tests nor
`setUp` (flash-loan callbacks, `receive`, `fallback`) — they "run when a
protocol calls back into the test contract; they form a third phase."

`Phase` is `Setup` / `Test` / `Callback` (src/trace.rs:15-22); `is_exploit()`
is true for the last two (src/trace.rs:24-29). This is the phase split every
severity decision reads: a cheat in the exploit is an error, the same cheat
while setting up is one level lower.

`Trace::build` (src/trace.rs:176-234) walks, in order:

1. Each contract in the linearization, base to derived (reverse order,
   src/trace.rs:185-210): state-variable initializers and constructors,
   filed as `<state>` ops in the `Setup` phase.
2. `setUp()`, resolved to the most derived definition.
3. The test function, wrapped in its modifiers: `walk_modified`
   (src/trace.rs:249-278) runs each modifier body in place, and its `_;`
   placeholder runs the rest of the chain, "so a `balanceLog` modifier's
   before/after reads land around the test body."
4. Callbacks: fallback/receive functions and externally callable functions
   that are neither `test*`, `invariant*` nor `setUp` (src/trace.rs:217-232).

Calls to the suite's own functions are inlined at the call site, up to eight
levels (`MAX_INLINE_DEPTH`, src/trace.rs:166), with recursion cut by
checking the (linearization index, function name) stack
(src/trace.rs:237-244). Expressions are visited post-order, so in
`vault.withdraw(token.balanceOf(me))` the read comes before the withdrawal
(src/trace.rs:327-354).

Every call is classified into `CallClass` (src/trace.rs:32-52): `Cheat` (a
`vm.*` call), `StdCheat` (a forge-std helper called without a receiver),
`Assert`, `Log`, `Builtin`, `Cast`, `Internal`, `External`, or
`Deploy(String)`. The cheat receivers `vm`, `cheats`, `hevm`, `cheat`,
`cheatcodes` and `VM` are recognised (`CHEAT_RECEIVERS`, src/trace.rs:101),
"which covers harnesses that declare the cheatcode interface themselves"
(docs/how-it-works.md:63-65). The bare forge-std helpers are `deal`, `hoax`,
`startHoax`, `changePrank`, `makeAddr`, `makeAddrAndKey`, `makeAccount`,
`skip`, `rewind`, `bound`, `deployCode`, `deployCodeTo`, `label`
(`STD_CHEATS`, src/trace.rs:104-118). Assertion names (`is_assert_name`,
src/trace.rs:139-148) are `assert*`, `require`, `fail`, `expectRevert*`,
`expectPartialRevert*`, `expectEmit`, `expectEmitAnonymous`, `expectCall`.
`super.f()` resolves to the next definition after the current position in
the linearization (src/trace.rs:426-430).

### Sender replay

`replay_senders` (src/rules/mod.rs:68-123) replays the trace to compute the
sender of every external call and deployment: `vm.prank`/`hoax` switch the
sender for the next external call or deployment; `vm.startPrank`,
`changePrank`, `startHoax` switch it persistently until stopped;
`broadcast`/`startBroadcast` behave the same way; `stopPrank`/`stopBroadcast`
clear the persistent switch. Each phase starts as the test contract, "which
is how Foundry runs `setUp()`, the test, and a callback the protocol
triggers" (src/rules/mod.rs:64-67). The result feeds `Ctx::senders` and
`Ctx::switches` (src/rules/mod.rs:37-49).

## 6. The Solana pipeline

`src/solana/mod.rs` states the architecture: the pipeline mirrors the
Solidity one — parse (syn, oxc) into one model, trace with helpers inlined,
env decides who each keypair and account is, rules report — and "nothing
after the parsers depends on the language" (src/solana/mod.rs:9-15).

A Solana suite is the tests of one Rust module (named by file stem and
module path, e.g. `exploit`, `lib::tests`) or one `describe` block (named by
its titles, e.g. `vault > withdraw`) — `suites()`, src/solana/mod.rs:52-76.
A test is checked only when it drives a harness: `check_test` returns
`None` for a test that is the declared setup of later tests
(`is_setup_for_later_tests`, src/solana/mod.rs:78-88) or whose trace raises
no harness event (`rules::uses_harness`, src/solana/rules.rs:1323-1325).
The harnesses recognised are, in Rust, `solana-program-test`, LiteSVM,
Mollusk, anchor-client and anchor-litesvm, and in TypeScript the Anchor
program client, web3.js, bankrun and LiteSVM (src/solana/mod.rs:3-8).

### Trace with parameter binding

`src/solana/trace.rs` builds a flat op sequence. Its distinctive mechanism
is the scope: when a helper is inlined, its parameters are bound to the
caller's arguments (src/solana/trace.rs:12-16), so inside
`fn send(svm: &mut LiteSVM, ix: Instruction, signer: &Keypair)` the name
`signer` stands for whatever the call site passed — "a transaction signed
by `signer` is seen as signed by `admin` when the test calls
`send(&mut svm, ix, &admin)`" (src/solana/trace.rs:1-16).
`Op::key`/`Op::name_key`/`Op::resolved`/`Op::keys`/`Op::exprs`
(src/solana/trace.rs:137-173) resolve identities through those bindings.

Helpers resolve from the test's own file, then from shared test code: any
same-language file under a directory named `tests` or `test` (`Scopes`,
src/solana/trace.rs:286-340). "Program source is never inlined: it is the
code under review, and a test reaches it through transactions"
(src/solana/trace.rs:288-290). A call resolves only when the name, owner and
argument count fit (`Function::accepts`, src/solana/ir.rs:93-100;
`resolve`, src/solana/trace.rs:586-624), and a harness method name never
resolves to a fixture method of the same name (`harness::METHODS`,
src/solana/trace.rs:611-614).

A helper whose name says it builds the world flags its ops
(`in_setup_helper`): `setup`, `init*`, `create*`, `fixture`, `deploy`,
`bootstrap`, `prepare`, `new`, `fund`, `configure`, `genesis`, and `set_up`
(`SETUP_WORDS`/`is_setup_name`, src/solana/trace.rs:222-245).

`key` (src/solana/trace.rs:191-203) is the canonical identity of a
keypair/address expression: references, clones and `.pubkey()` are peeled,
`Some(x)` is `x`, and a field is named by its last segment
(`&ctx.payer.pubkey()` is `payer`).

### Harness events

`src/solana/harness.rs` recognises harness calls by method and path names,
not types — "because the tool never resolves packages: `svm.set_account(..)`
is a direct account write whatever `svm` is" (src/solana/harness.rs:1-10).
Both languages reach it in the same spelling (`svm.setAccount` is matched
as `set_account`). `event()` (src/solana/harness.rs:240-372) maps calls to
the `Event` enum (src/solana/harness.rs:72-115):

- `Transaction { signers, implicit, instructions, accounts }` —
  `Transaction::new_signed_with_payer`, `Transaction::new*`,
  `VersionedTransaction::try_new`, `tx.sign(..)`, a Mollusk `process_*` run,
  an Anchor builder (`program.methods.x(..).signers([..]).rpc()`; the
  legacy `program.rpc.x(.., { accounts, signers })` namespace;
  `provider.sendAndConfirm`; `sendAndConfirmTransaction`;
  `connection.sendTransaction`), and anchor-litesvm's
  `ctx.execute_instruction(ix, signers)`.
- `WriteAccount` — `set_account`, `add_account`.
- `LoadAccount` — `add_account_with_file_data`,
  `add_account_with_base64_data`.
- `Airdrop` — `airdrop`, `request_airdrop` ("a real system transfer, the
  same one an attacker could make with their own lamports",
  src/solana/harness.rs:99-104).
- `LoadProgram` — `ProgramTest::new`, `add_program`,
  `add_program_from_file`, `add_upgradeable_program_to_genesis`,
  `Mollusk::new`, `AnchorLiteSVM::build_with_program`.
- `LiveRpc` — `RpcClient::new*`, an anchor-client `Client::new`, a web3.js
  `Connection`.
- `Send` — a transaction sent whose building the trace cannot see.
- `Harness` — a harness construction.

The implicit fee payer is a first-class fact: the Anchor TypeScript
provider's wallet and the anchor-client payer sign every request, but count
as a signer of a transaction "only when it is the only signer or the
instruction names it" (src/solana/rules.rs:120-129) — "a wallet that is
the admin and also pays for the attacker's transaction did not authorize
it" (docs/how-it-works.md:231-234).

Where the exploit starts: `find_exploit_start` (src/solana/rules.rs:192-202)
takes the first transaction that does not build the world — one sent from a
setup helper, or one whose instructions say `init`/`create`/`setup` and that
nobody who reads as the attacker signs — falling back to the attacker's
first transaction. Everything before it is setup; a cheat there is reported
one level lower, as in `setUp()`.

## 7. Identity: who is who in a test

### Solidity: `src/identity.rs`

The approach is stated up front: name- and data-flow-based, reading
identifier names, `makeAddr`/`vm.label` labels, assignments, role grants and
deployments; "it cannot see a deployed contract's access control, so every
classification is a presumption the rule pages state, and a `realism.yaml`
declaration overrides it" (src/identity.rs:1-8).

The vocabularies (src/identity.rs:16-151): 30 role words (`admin` through
`sequencer`), holder words that neutralise `owner` (`tokenOwner`,
`nftOwner`), 13 attacker words, 45 user words, mock words (`mock`, `fake`,
`dummy`, `stub`, `harness`), helper words, token hints, 15 exact grant
methods (`grantRole`, `transferOwnership`, `rely`, ...) and grant verbs.
`words()` (src/identity.rs:156-184) splits identifiers on case boundaries
and non-alphanumerics, lowercases and drops trailing digits (`user1` reads
as `user`).

- `role_word` (src/identity.rs:191-200): `owner` is a role only without a
  holder word.
- `is_privileged_getter` (src/identity.rs:221-237): `owner()`, `getAdmin()`,
  `getRoleMember`; explicitly not `ownerOf` or the `is*`/`has*`/`set*`/
  `grant*`/`revoke*`/`renounce*`/`transfer*`/`accept*` forms.
- `is_grant_method` (src/identity.rs:240-249): the exact list, or a grant
  verb followed by a role word (`setKeeper`, `addOperator`).
- `key` (src/identity.rs:253-270) peels casts: `address(vault)`,
  `payable(vault)` and `IVault(vault)` are all `vault`.

`Env` (src/identity.rs:311-571) gathers, per test trace: assignments and
declared types, labels (`vm.label`, and `makeAddr("admin")`-style
constructors via `make_addr_label`, src/identity.rs:631-645), grants the
harness makes (a second pass once types are known, filtering role constants
and `bytes32`s out with `identity_arg`, src/identity.rs:590-604), constants,
test parameters, and contract types defined in the suite's own file.

`class_of` (src/identity.rs:492-536) decides `Class`
(`This`/`Attacker`/`Eoa`/`Mock`/`Helper`/`Token`/`Protocol(String)`/`Unknown`)
in a fixed order: the test contract; declared or name/label-inferred
attacker (once `attacker.identities` is declared, names stop counting,
src/identity.rs:403-416); mock names; helper types or names; user names,
user labels, `makeAddr`-made EOAs or declared victims; token names;
contracts deployed by the harness; contract-typed variables that are not the
file's own helpers; and address-typed constants initialised from address
literals. `privileged` (src/identity.rs:426-477) walks the same evidence to
at most six levels of aliasing: declared identities, role words in names or
labels, harness grants, assignments from privileged getters, and plain
aliases.

### Solana: `src/solana/env.rs`

`Class` here is `Payer`/`Attacker`/`User`/`Mock`/`Protocol(String)`/
`Program`/`Unknown` (src/solana/env.rs:21-35). The classification order
(`class_depth`, src/solana/env.rs:290-358) puts derivation before names: an
account the test derives with `find_program_address` is the program's
"whoever it describes (`attacker_position`)" (src/solana/env.rs:298-305);
an associated token account belongs to its owner's class; then attacker
names (unless declared attackers exist), the harness's `payer`, mock names,
user names, `wallet`/`provider` words, loaded program ids, fixed addresses
(`pubkey!`, `Pubkey::from_str`, `from_str_const`, `new_from_array`,
`new_const`), the 18 protocol-state words (`vault`, `pool`, `reserve`,
`market`, `config`, ...), and aliases.

Role grants come from three shapes (src/solana/env.rs:211-260): an
`is_grant_method` call (with spl-token's `set_authority` granting only its
third argument), a role field in a struct or call that initializes or
reconfigures (`Initialize { admin: attacker.pubkey() }`), and a role field
in the `.accounts({ .. })` of such an Anchor instruction. `role_of`
(src/solana/env.rs:161-168) excludes `payer`, attacker names and user
names, so `user_authority` is not a role. `owner`/`authority` fields whose
value reads as a user say "whose account it is", not a role
(src/solana/env.rs:246-252).

`is_program_under_test` (src/solana/env.rs:418-438) recognises the loaded
program's id, `crate::ID`, `program_id` and `<crate>::ID` for crates not in
`LIBRARY_CRATES` (system program, spl-token and friends); the well-known
web3.js and spl-token ids are excluded by `LIBRARY_IDS`
(src/solana/env.rs:99-110).

## 8. The engine: findings, merge semantics, summary

`engine::analyze` (src/engine.rs:109-311) runs everything and assembles the
report. The pipeline:

1. Suppression directives are collected per file (src/engine.rs:114-118).
2. A `BTreeMap` keyed on `(file, line, column, rule)` merges raw findings —
   "so a `setUp()` line shared by many tests is reported once, listing them
   all" (src/engine.rs:120-122): a later finding with the same key appends
   its test name to the existing entry's `tests` (src/engine.rs:141-147).
3. Solidity suites are walked (`project::suites`); per test,
   `trace::build`, then `Env::new`, then the declared identities are copied
   in, then `rules::check_test` (src/engine.rs:184-219). `--match-contract`
   and `--match-test` filter suites and tests before they are counted
   (src/engine.rs:185-200).
4. Solana suites are walked the same way through `solana::suites` and
   `solana::check_test` (src/engine.rs:221-267); a language (`Lang::Rust` or
   `Lang::TypeScript`) is attached per finding so the help text can vary
   (src/engine.rs:259-264).
5. Declaration-only rules run once per run against the declaration text
   (src/engine.rs:269-273).
6. Findings are sorted by `(file, line, column, rule)` (src/engine.rs:276-278)
   and counted: `suppressions_ignored` for allow comments without a reason
   — their findings stay active (src/engine.rs:279-282) — then
   `errors`/`warnings`/`notes` for active findings only.

Per raw finding, `add` (src/engine.rs:128-182) applies the rule filters
(`only`/`skip`, src/engine.rs:37-39), files the declaration's findings
under `DECLARATION_UNIT = usize::MAX` (src/engine.rs:16), applies `exclude`
and the `--changed-since` scope (findings in changed files, or raised in a
suite defined in one of them, src/engine.rs:41-49), resolves the severity
override (`opts.severity`, src/engine.rs:165), applies the suppression
directive (only with a reason, src/engine.rs:148-159), and selects the
language's fix text via `rules::help` (src/rules/mod.rs:175-181 — the first
sentence of the fix).

`Report` (src/engine.rs:87-96) carries `summary`, `declaration`, `config`,
`changed_since`, `findings`, `parse_errors`. `Report::fails(level)`
(src/engine.rs:104-107) drives the exit code: any active finding at or
above the level. `Severity` is ordered `Note < Warn < Error`
(src/finding.rs:6-15) with the meaning of each level documented on the
enum: error — "the PoC, as written, does not show what an unprivileged
attacker can do"; warn — "it needs a justification it does not give"; note —
context that never fails a run on its own.

## 9. The rule set (14 rules)

The count is 14, verified three ways: the `RULES` array
(src/rules/catalog.rs:71-402) lists exactly RG01-RG14 (with the
ids-unique-and-ordered test at src/rules/catalog.rs:492-497);
`realism-gate rules` prints 14 lines (asserted at tests/cli.rs:220-228); and
the SARIF rule table has 14 entries (asserted at tests/cli.rs:101). The
README's table of 14 rules matches the catalog, and the rule pages under
`docs/rules/` are generated from the same catalog (14 files, asserted at
tests/docs.rs:51-57).

The catalog is the single source of truth for rule metadata: "`realism-gate
rules`, `realism-gate explain`, the SARIF rule table and the pages under
`docs/rules/` are all generated from it" (src/rules/catalog.rs:1-4). Lookup
is by id or slug, case-insensitive (src/rules/catalog.rs:404-409).

The table below is read off the catalog entries and the rule code. Severity
notes are quoted verbatim from `severity_note`.

| Id | Slug | Severity | Severity note (verbatim) | Trigger (as the code defines it) |
|---|---|---|---|---|
| RG01 | `attacker-holds-privilege` | error | "always error" | The harness grants the attacker identity a role, or the attacker is assigned from a privileged getter (src/rules/privilege.rs:84-136); Solana: a grant the harness makes, or the attacker keypair is a role holder's (src/solana/rules.rs:295-355) |
| RG02 | `privileged-step-in-exploit` | warn | "warn; silenced by a realism.yaml `privileged:` entry" | A prank/broadcast switch in the exploit phase to a role, a protocol contract, or a hard-coded address (src/rules/privilege.rs:140-190); Solana: an exploit transaction signed by a role holder (src/solana/rules.rs:358-380) |
| RG03 | `deployer-is-attacker` | warn | "warn" | The test contract deploys the protocol and runs non-read calls on it as itself in the test phase (src/rules/privilege.rs:194-291); Solana: the exploit is signed only by keys that also signed the setup (src/solana/rules.rs:383-417) |
| RG04 | `storage-write` | error | "error in the exploit; warn in setUp; one step lower when the slot looks like an attacker or user balance" | `vm.store` into a contract under review (src/rules/injection.rs:58-114); Solana: `set_account`/`add_account` writing non-balance state of a program account (src/solana/rules.rs:698-707) |
| RG05 | `code-replacement` | warn | "warn; capped below error since 0.2.0 because its measured precision is under 80% (docs/precision.md)" | `vm.etch` or `deployCodeTo` over a non-own address (src/rules/injection.rs:117-147); Solana: a mock-named program loaded at a real program's address (src/solana/rules.rs:713-764) |
| RG06 | `balance-injection` | error | "error in the exploit; warn in setUp" | `deal`/`vm.deal` pointed at a contract under review (src/rules/injection.rs:150-180); Solana: a write setting an account's lamports or token balance (src/solana/rules.rs:681-697) |
| RG07 | `mocked-call` | warn | "warn" | `vm.mockCall`/`mockCallRevert`/`mockCalls`/`mockFunction` on a dependency (src/rules/injection.rs:183-211); not checked in Solana (src/rules/catalog.rs:250-252) |
| RG08 | `no-assertion` | error | "error; warn when the test logs values it never checks" | No assertion anywhere in the exploit phases (`testFail*` tests exempt) (src/rules/evidence.rs:54-100); Solana: same, with `#[should_panic]` tests exempt (src/solana/rules.rs:776-825) |
| RG09 | `tautological-assertion` | error | "error" | An assertion comparing two constants, an expression with itself, or a literal `true` (src/rules/evidence.rs:103-157); Solana: the `assert!`/`assert_eq!`/`assert_ne!` forms (src/solana/rules.rs:857-886) |
| RG10 | `no-baseline` | warn | "warn" | A directional assertion over a measurement read only once, after the attack, with no funding or freshness baseline (src/rules/evidence.rs:408-621); Solana: the same over balance reads (src/solana/rules.rs:1088-1276) |
| RG11 | `unpinned-fork` | warn | "warn" | `vm.createSelectFork(url)`/`createFork(url)` with one argument and no later `vm.rollFork` (src/rules/evidence.rs:624-650); Solana: a live-cluster client whose URL is not local (src/solana/rules.rs:1288-1319) |
| RG12 | `attacker-in-foreign-transaction` | error | "error" | Declaration: an attacker step with `boundary: victim_transaction` (src/declaration.rs:165-177) |
| RG13 | `unordered-same-block` | warn | "warn" | Declaration: an attacker step with `boundary: same_block` and no `ordering_guarantee` (src/declaration.rs:178-194) |
| RG14 | `dependent-precondition` | note | "note" | Declaration: a precondition performed by a victim, privileged role or third party (src/declaration.rs:195-211) |

Rules RG01-RG11 are code rules; RG12-RG14 read only `realism.yaml`
(`SolanaNotes::Declaration`, src/rules/catalog.rs:49-52; asserted at
src/rules/catalog.rs:524-535). RG07 is the only code rule not checked in
Solana tests: "Solana test harnesses have no cheat that makes a program
answer with a value its code would not produce" (src/rules/catalog.rs:250-252).

Every catalog entry also carries `summary`, `why`, `fix`, `bad`/`good`
example code, and known false-positive modes; the
`every_code_rule_documents_a_false_positive_mode` test requires all of
RG01-RG11 to list at least one, for Solidity and for Solana
(src/rules/catalog.rs:506-522). Fix texts are per-language where the Solana
form differs (`fix_for`, src/rules/catalog.rs:55-63): a Rust or TypeScript
finding of a checked rule carries the Solana fix (asserted at
src/rules/mod.rs:187-201 and tests/rust.rs:79-89).

Rule dispatch is a fixed list in `rules::check_test` (src/rules/mod.rs:147-159):
`attacker_holds_privilege`, `privileged_step_in_exploit`,
`deployer_is_attacker`, `storage_write`, `code_replacement`,
`balance_injection`, `mocked_call`, `no_assertion`,
`tautological_assertion`, `no_baseline`, `unpinned_fork`. The Solana
dispatch (src/solana/rules.rs:218-231) is the equivalent list minus RG07.

Two rule severities encode measurement decisions rather than judgement
calls: RG05 is capped at warn because "on the labelled corpora it was right
on 2 of 5 flags, below the 80% floor for an error"
(src/rules/injection.rs:133-136), and the release ruling R3 is enforced
mechanically by `eval/precision.py` (see
[section 14](#14-measurement-infrastructure)).

## 10. Suppressions, configuration, declarations

### Inline allow comments: `src/suppress.rs`

A directive is `// realism-gate: allow(RG04) <reason>` on the finding's line
or the line above (src/suppress.rs:1-13). The parentheses are optional and
multiple rules may be listed (`allow RG04, RG06 <reason>`). The grammar is
one regex (src/suppress.rs:25-33) that only matches comment-shaped text
(`//`, `/*`, `*`), so a string containing the words is not a directive
(asserted at src/suppress.rs:99-101). A directive without a reason is parsed
but ignored: the finding stays active and the summary counts it in
`suppressions_ignored` (src/engine.rs:149-159, 279-282). Silenced findings
stay in the JSON and SARIF output, marked suppressed
(docs/output-schema.json:63-72; tests/options.rs:192-213).

### `realism-gate.toml`: `src/config.rs`

Two keys: `exclude` (a list of globs) and `[rules]` (rule id or slug to
`error|warn|note|off`) (src/config.rs:1-19). Unknown rules, unknown levels
and unknown keys are errors (src/config.rs:149-153). The glob-to-regex
translation (src/config.rs:100-131) gives `**/` the meaning "any number of
directories, including none", `*` and `?` a single path segment, and a bare
directory pattern everything under it. Flags win over the file: `--rules`
runs a rule the file turns off, `--severity` replaces a level, `--exclude`
adds patterns (src/config.rs:17-19; src/cli.rs:294-321).

### `realism.yaml`: `src/declaration.rs`

The declaration is version-1 YAML parsed by `serde-saphyr` with
`deny_unknown_fields` on every struct (src/declaration.rs:16-30): `version`
(required, must be 1), `chain` (`evm`, the only chain model), `attacker`
and `victim` `identities` lists, `privileged` entries (identity, role and
reason — an empty reason is a parse error, "the reason is the point of
declaring it", src/declaration.rs:126-133), and `preconditions` (step,
`performed_by`: attacker|victim|privileged|third_party|state, `boundary`:
own_transaction|victim_transaction|same_block|callback_from_victim_call|
external_trigger, optional `ordering_guarantee`) (src/declaration.rs:57-103).

Identities are canonicalised the way the code reads them: `address(x)` and
`payable(x)` name `x` (src/declaration.rs:217-226). The template
`realism-gate init` writes is the `TEMPLATE` constant
(src/declaration.rs:236-266). The effects a declaration has: declared
attackers replace the name heuristic entirely (src/identity.rs:403-416),
declared privileged identities silence RG02 for those keys
(src/rules/privilege.rs:149; src/solana/rules.rs:364-366), declared victims
are user accounts (src/identity.rs:514), and the preconditions enable
RG12-RG14 (src/declaration.rs:152-214), whose findings are filed against the
yaml file itself, at the line the step text is found
(src/declaration.rs:227-233).

## 11. Renderers

`src/render/mod.rs` holds the shared constants: the tool name, the version
(`env!("CARGO_PKG_VERSION")`), the repository URL, and the rule
documentation URL (`<repo>/blob/main/docs/rules/<ID>.md`,
src/render/mod.rs:15-17).

- **JSON** (src/render/json.rs): `schema` is the constant `1`
  (src/render/json.rs:8); the document is `{ tool, version, schema, ...report }`
  with the serialized `Report` flattened in. "Fields are only ever added
  within a schema version; a removal or a change of meaning bumps `schema`"
  (src/render/json.rs:1-3). The published shape is
  `docs/output-schema.json` (JSON Schema draft 2020-12, 90 lines), and
  tests/schema.rs validates four real runs against it with a hand-written
  validator covering the subset of JSON Schema the file uses, "so the check
  needs no extra dependency" (tests/schema.rs:1-114).
- **SARIF 2.1.0** (src/render/sarif.rs): one run; the full 14-rule table
  with `defaultConfiguration.level` mapped from severity
  (src/finding.rs:27-33); each result carries
  `partialFingerprints.realismGate/v1` = `rule:file:evidence`
  (src/render/sarif.rs:38-40) so code scanning tracks a finding across
  commits (docs/ci.md:207-208); suppressed findings get
  `suppressions: [{ kind: "inSource", justification }]`
  (src/render/sarif.rs:43-48).
- **Text** (src/render/text.rs): the header
  `realism-gate 0.4.0 · N files · N suites · N tests`, per-file grouping,
  severity-coloured rule ids, the message wrapped at 96 columns, the
  evidence after the arrow glyph, the help after the fix arrow, suppressed
  findings shown as `allow` with their reason, a parse-errors section, the
  `SUMMARY` and `RESULT PASS/FAIL` lines, and the closing disclaimer
  (src/render/text.rs:83-209).
- **Markdown** (src/render/markdown.rs): a job summary / PR comment with a
  PASS/FAIL heading, an active-findings table capped at 50 rows (`MAX_ROWS`,
  "A GitHub comment holds 65,536 characters", src/render/markdown.rs:9-10),
  a collapsible table of silenced findings, and the same disclaimer.

Both text and Markdown are explicitly unstable: "Like the text format it is
for people and may change between releases; parse the JSON"
(src/render/markdown.rs:1-2).

## 12. The changed-since git integration

`--changed-since REF` (src/changed.rs) runs exactly two git invocations
(src/changed.rs:36-54):

```
git diff --name-only --relative --diff-filter=d -z <REF> --
git ls-files --others --exclude-standard -z
```

The first is the committed, staged and unstaged difference from `REF`
(deletions filtered out); the second is untracked-but-not-ignored files.
Paths are made relative and given forward slashes "so they compare equal to
the paths in the report" (src/changed.rs:1-9). A ref starting with `-` is
rejected (src/changed.rs:33-35). The offline claim is in the code: "This
runs the local `git` binary. It reads the repository; it never fetches. In
CI the base revision must already be in the clone" (src/changed.rs:9-10).

Everything is still read — inheritance needs the unchanged files — but only
findings in changed files, or raised in a suite defined in one of them, are
reported (`Options::only_files`/`in_scope`, src/engine.rs:31-49). The
report's `changed_since` object records the ref and the file count
(src/engine.rs:52-58, src/cli.rs:338-344).

The GitHub Action resolves the base itself (action.yml:212-230): a pull
request uses `github.event.pull_request.base.sha`, a push the `before`
commit; if that commit is not in the clone it runs
`git fetch --no-tags --quiet --depth=1 origin <base>` once, and otherwise
warns and checks everything.

## 13. The GitHub Action

`action.yml` (336 lines) is a composite action, "realism-gate PoC check"
(action.yml:1-3). It needs no Rust or Node toolchain: it downloads a
release binary and runs it.

**Inputs** (12, action.yml:8-56): `path` (default `test/`, space-separated
files, directories or globs), `version` (default: the release this action
belongs to, falling back to `RG_DEFAULT_VERSION: v0.4.0` at
action.yml:99), `changed-only` (default `true`), `fail-on` (default
`error`), `comment` (default `true`), `sarif` (default `false`),
`declaration`, `args`, `github-token`, `binary` (a test hook: run this
binary instead of downloading one), and the deprecated 0.1.0 names `paths`
and `upload-sarif`, used when set.

**Outputs** (9, action.yml:58-85): `exit-code`, `errors`, `warnings`,
`notes`, `version`, `report` (the JSON path), `markdown`, `sarif`, and
`comment` (`posted`, `updated`, `skipped` or `failed`).

**Install and verification** (action.yml:90-150): the target is picked from
`RUNNER_OS-RUNNER_ARCH` (five supported: musl Linux x64 and ARM64, macOS x64
and ARM64, Windows x64); the archive and `SHA256SUMS` are downloaded from
the release; then the checksum mechanism — the archive's line is extracted
from SHA256SUMS into a one-line `check.txt`, required to be non-empty, and
checked with `sha256sum -c` (or `shasum -a 256 -c` where sha256sum is
absent) (action.yml:129-138). Extraction puts the binary at a known path,
and `--version` is run before anything else.

**Four-pass execution** (action.yml:152-270): the run step executes
`realism-gate check` four times, once per output:

1. `--format json --output report.json` — the authoritative run; the exit
   code is captured as `exit-code` (action.yml:234-236) and the severity
   counts are read from this JSON (action.yml:262-266).
2. `--format markdown --output summary.md` — appended to
   `$GITHUB_STEP_SUMMARY` (action.yml:256, 267).
3. `--format sarif`, only when `sarif` is on, written with `--fail-on never`
   (action.yml:257-261) and uploaded by
   `github/codeql-action/upload-sarif@v4` with category `realism-gate`
   (action.yml:320-325), skipped for forks (action.yml:321).
4. `--color always --fail-on never` — the findings printed into the log
   (action.yml:269).

Exit code 2 (a path with nothing to check) is surfaced as a "could not
check" summary and a failed step (action.yml:237-253). The comment step
(action.yml:272-318) keeps exactly one comment per job, found by a hidden
marker comment naming the job and edited in place; fork pull requests get a
read-only token, so the comment is skipped, not failed
(action.yml:293-299). The final Result step maps 0/1/2 to pass /
finding-failed / could-not-check (action.yml:327-336).

**Self-test**: `.github/workflows/action-selftest.yml` runs the action from
the checkout (`uses: ./` with the `binary` input, so no release download is
involved) on ubuntu, macOS and Windows over fixtures whose result is known,
and asserts 19 outcome and output pairs (.github/workflows/action-selftest.yml:122-155): the
clean run succeeds with exit-code 0 and errors 0; a warn-level fixture
passes at `fail-on error` and fails at `fail-on warn` with exit-code 1; an
error fixture fails by default and passes with `fail-on none` while still
reporting `errors: 1`; empty and no-test inputs are usage errors (exit 2,
"never a pass"); the legacy `paths` input still works; and each run's
Markdown report is its own file with the expected content
(.github/workflows/action-selftest.yml:157-168).

## 14. Measurement infrastructure

### The corpus benchmark: `bench/`, `scripts/bench-corpus.sh`

`scripts/bench-corpus.sh` measures the release binary against
[DeFiHackLabs](https://github.com/SunWeb3Sec/DeFiHackLabs) at pinned commit
`2c99b565ae24ea2006adf181da20c4419b3edc30`, `src/test` only, fetched (never
vendored) into a gitignored `.corpus/` through a sparse, blob-filtered
clone (scripts/bench-corpus.sh:18-43). It runs
`realism-gate check src/test --no-declaration --format json --fail-on never`
(scripts/bench-corpus.sh:47), reduces the report with
`scripts/summarize_corpus.py`, and in `--check` mode diffs the summary
against `bench/defihacklabs.json` — "CI re-runs it on every pull request
and fails if any number below moves, so a rule change that shifts them has
to update this page" (docs/benchmark.md:20-22).

`bench/defihacklabs.json` (the committed, measured numbers): 891 files, 869
suites, 886 tests, 1 file with parse errors, 627 tests with any finding, 20
tests with a finding other than RG08; per rule: RG01 1 error (1 test), RG02
15 warn (13 tests), RG04 5 error + 2 warn (3), RG05 4 warn (4), RG08 16
error + 595 warn (611), RG10 2 warn (2), RG11 1 warn (1)
(bench/defihacklabs.json:4-55). `docs/benchmark.md` states the headline:
611 tests (69.0%) assert nothing, of which 595 log values a reader has to
trust (docs/benchmark.md:44-55); its reviewed 0.1.0 table totals 33
findings on 23 tests — 28 correct, 2 debatable, 3 false positives
(docs/benchmark.md:64-72) — and notes that in v0.2.0 RG10 raises 2 findings
instead of 5 (docs/benchmark.md:57-62).

### The precision evaluation: `eval/`

`eval/README.md` documents the method so the numbers "can be re-derived by
anyone" (eval/README.md:1-5). The inputs:

- `eval/corpus.json` pins three corpora by commit and by the sha256 of
  their `git ls-tree -r <commit> -- src/test` listing — content-addressed,
  so it "does not depend on the line-ending settings of the checkout"
  (eval/corpus.py:1-8): `in_sample` (DeFiHackLabs, 891 .sol files, the
  commit 0.1.0 was built against), `later` (DeFiHackLabs files added after
  that commit, 70 new .sol files — "a second tuning set, not held-out"),
  and `fresh` (DeFiVulnLabs, 57 files, no licence stated, "fetched for
  measurement only, never vendored, and the committed flag files omit its
  source text").
- `eval/seed.txt`: the sampling seed `realism-gate-v0.2.0-eval-2026-11-04`.
- `eval/samples/*.jsonl`: the flags drawn for labelling — per rule and
  corpus, at most 30, ordered by sha256(seed:id), "deterministic and does
  not depend on any library's RNG" (eval/sample.py:5-7).
- `eval/labels/v0.2.0.jsonl`: one label per sampled flag — `true-positive`,
  `false-positive` or `unclear`, with a one-line reason.

A **flag** is an active (not allow-silenced) finding; its stable id is the
first 16 hex digits of sha256(rule|file|line|column|evidence), "so one
label covers the same flag in both versions" (eval/README.md:41-44).
`eval/precision.py` computes precision = TP/(TP+FP) with Wilson 95%
intervals (Z = 1.959964, eval/precision.py:43), pools per rule over the
corpora, and enforces ruling R3: "a rule whose v0.2.0 precision is below
0.80 on any corpus where it has a TP or FP label may not ship at severity
error" (eval/precision.py:18-21); with `--rules` (the
`rules --format json` output of the binary under test) it fails the run if
one does (eval/precision.py:113-128).

`eval/run.sh` regenerates every flag file and the precision tables from the
pinned corpora and the committed labels, and in `--check` mode fails on any
difference. The "before" side of every before/after figure is the 0.1.0
release binary itself, checked against the release's SHA256SUMS
(.github/workflows/ci.yml:229-246).

The committed results (`eval/results/precision.md`, generated): pooled
v0.2.0 precision — RG01 1/1, RG02 15/16 = 94%, RG04 7/7, RG05 2/5 = 40%,
RG07 1/1, RG08 61/61 = 100%, RG10 3/3 = 100%, RG11 2/2; "Below the 80%
floor in v0.2.0 on some corpus: RG02, RG05" (eval/results/precision.md:36-49)
— both ship as warnings, which is what R3 requires. `docs/precision.md`
adds what the numbers do not cover: no recall is measured anywhere, rules
that never fired (RG03, RG06, RG09 and RG12-RG14) are unmeasured, all three
corpora are public reproductions rather than submissions with known triage
outcomes, and one reader (labelled by an AI agent and marked "pending
operator review", eval/README.md:75-81) did all the labelling
(docs/precision.md:95-108).

The Solana rules are explicitly unmeasured: "Not measured. Every figure
above is for Solidity" (docs/precision.md:122-127). What was done instead,
before each release: the frontend was run over LiteSVM and five
solana-program-library test suites at pinned commits (164 Rust files
parsed, 451 tests drove a harness and were checked,
docs/precision.md:129-147), and over Anchor's own TypeScript tests,
program-examples and LiteSVM's Node tests (229 files that import a Solana
package, 292 tests in 118 suites, docs/precision.md:149-177). Every wrong
pattern found was reduced to a few lines and pinned by a test in
`tests/rust.rs` and `tests/typescript.rs`.

`eval/perf/` holds the timing measurements (v0.1.0/v0.2.0 per corpus, local
and install times), written by `scripts/measure-perf.py`,
`scripts/measure-install.sh` and `scripts/release-assets.sh`; the README's
"the whole corpus is analysed in under a second on a laptop" claim cites it
(README.md:399-400, eval/perf/).

## 15. Tests

The test suite is 7 integration test binaries plus unit tests inside the
source modules. Counted from the run output of `cargo test --locked` on
v0.4.0: 67 unit tests (`src`), 14 in `tests/cli.rs`, 3 in `tests/docs.rs`,
6 in `tests/options.rs`, 40 in `tests/rules.rs`, 19 in `tests/rust.rs`, 1
in `tests/schema.rs`, 16 in `tests/typescript.rs` — 166 tests in total,
all passing; the doc-test count is 0.

| File | Tests | What it enforces |
|---|---:|---|
| `tests/cli.rs` | 14 | Exit codes (0/1/2), the JSON envelope and SARIF shape, `--fail-on` thresholds, `--match-*` filters, `rules`/`explain`/`init` behaviour, both binaries identical |
| `tests/rules.rs` | 40 | Every `bad/` fixture fires exactly its own rule at the expected line and severity, and nothing else; every `good/` fixture is clean; the inherited-`setUp` merge; suppression semantics; the declaration replacing the heuristics |
| `tests/rust.rs` | 19 | Which Rust tests count as PoCs, placement of findings at the call site, declaration and suppression behaviour in Rust, every Rust rule-page example true, the reduced false positives |
| `tests/typescript.rs` | 16 | Hardhat files skipped, mocha order, jest/node/legacy Anchor forms, the provider-wallet signer rule, every TypeScript rule-page example true, the reduced false positives |
| `tests/options.rs` | 6 | `realism-gate.toml` precedence, `--severity`/`--exclude`, `--changed-since` against a scratch git repository, the Markdown format, paren-less allow comments |
| `tests/docs.rs` | 3 | The 14 rule pages equal the catalog output; the three README example blocks equal the binary's output on the three `examples/` projects; `action.yml`'s default version equals the crate version |
| `tests/schema.rs` | 1 | Four real JSON reports validate against `docs/output-schema.json` |

The fixture tree (`tests/fixtures/`, 54 files): `bad/` (32 files — one
`.t.sol`, one `.rs` and one `.ts` per rule RG01-RG11, plus a second RG10
Solidity fixture for the test contract's ether; the
`every_code_rule_has_a_bad_fixture` test requires a `.rs` and a `.ts`
fixture exactly when the rule checks Solana, tests/rules.rs:159-182),
`good/` (10 clean PoCs across the three languages), and the behavioural
directories `declaration` (2), `inherit` (2), `rust_declaration` (1),
`rust_shared` (2), `rust_suppress` (1), `suppress` (1), `ts_shared` (2)
and `ts_suppress` (1).

Numbers the tests assert (a sample, quoted from the source): `rules`
prints 14 lines and 14 JSON entries (tests/cli.rs:220-228); the SARIF
driver lists 14 rules (tests/cli.rs:101); the inherit fixture produces
exactly 1 finding listing both tests (tests/rules.rs:345-355); the
suppress fixture produces `suppressed = 1` and `errors = 1` with the
ignored allow comment at line 22 (tests/rules.rs:357-373); the declaration
fixture flips from two RG02 warnings to RG01 plus RG12/RG13/RG14 at yaml
lines 21/19/16/13 (tests/rules.rs:387-407); the corpus numbers are pinned
by `bench/defihacklabs.json` in CI rather than by a test.

The three `examples/` projects (`vault-drain` Foundry, `solana-vault-drain`
Rust, `anchor-vault-drain` Anchor TypeScript) are the README's live
examples: `tests/docs.rs` runs the built binary on each and fails if the
README's embedded output blocks drift (tests/docs.rs:59-103, regenerated
deliberately with `REALISM_GATE_BLESS=1`).

## 16. CI, release and verification pipelines

Pull requests run 14 check contexts: 11 from `.github/workflows/ci.yml`
and 3 from `.github/workflows/action-selftest.yml` (the self-test matrix
on three operating systems). Counted from the workflow files:

| # | Context | Workflow / job | What it does |
|---|---|---|---|
| 1-3 | `test (ubuntu/macos/windows-latest)` | ci.yml `test` | `cargo fmt --all -- --check`, clippy `--all-targets --all-features -D warnings` (with `RUSTFLAGS: -D warnings`), `cargo test --locked`, and the smoke test (build, `--version`, `rules`, `explain RG06`, good fixtures pass, bad fixtures must fail) (.github/workflows/ci.yml:17-55) |
| 4 | `package` | ci.yml `package` | `cargo package --list --locked` must contain only `src/*.rs` and the six top-level files; `cargo publish --dry-run --locked` (.github/workflows/ci.yml:56-81) |
| 5 | `msrv` | ci.yml `msrv` | reads `rust-version` from Cargo.toml (1.95), installs that toolchain, `cargo +<msrv> test --locked` (.github/workflows/ci.yml:82-104) |
| 6 | `cargo-deny` | ci.yml `deny` | `cargo-deny check --all-features` against `deny.toml`: the licence allow-list (one BSL-1.0 exception for `dragonbox_ecma`, reached through oxc), bans on 13 network crates, sources restricted to crates.io (deny.toml:27-76) |
| 7 | `cargo-audit` | ci.yml `audit` | the ignored advisory crates are absent from the build graph (`cargo tree -i crypto-bigint` must print nothing) and `cargo audit` passes against `.cargo/audit.toml` (.github/workflows/ci.yml:115-146) |
| 8 | `no network code` | ci.yml `no-network` | the resolved normal dependency graph contains none of reqwest/hyper/ureq/curl/isahc/surf/attohttpc/tokio/async-std/native-tls/rustls/openssl/tungstenite, and `src/` contains no socket or request code — "A PoC under review may be for an embargoed finding. The checker must never be able to send it anywhere, so this is a property, not a promise" (.github/workflows/ci.yml:147-180) |
| 9 | `hygiene` | ci.yml `hygiene` | `scripts/check_hygiene.py`: no host paths, no private-workspace markers, no bug-bounty platform names in fixtures/examples/rule docs, no mojibake (.github/workflows/ci.yml:181-196, scripts/check_hygiene.py:1-19) |
| 10 | `corpus benchmark` | ci.yml `corpus` | release build plus `bench-corpus.sh --check` (.github/workflows/ci.yml:197-215) |
| 11 | `precision evaluation` | ci.yml `eval` | release build plus the 0.1.0 release binary verified against SHA256SUMS plus `eval/run.sh --check` (.github/workflows/ci.yml:216-246) |
| 12-14 | `self-test (ubuntu/macos/windows-latest)` | action-selftest.yml | the action from this checkout on fixtures whose result is known, asserting outcomes and outputs (see [section 13](#13-the-github-action)) |

### Release pipeline: `.github/workflows/release.yml`

Triggered by a version tag `v[0-9]+.[0-9]+.[0-9]+` (optionally with a
suffix, which makes a pre-release); the floating `v0` tag "only moves the
GitHub Action, so it does not match the filter" (.github/workflows/release.yml:3-18). A manual
`workflow_dispatch` is a dry run: the same builds, archives and install
test, but "no tag, no release, no attestation" (.github/workflows/release.yml:8-11). Nothing
in the workflow publishes to crates.io — "the crate name is claimed
permanently and the action is irreversible, so `cargo publish` is a
separate, manual step" (.github/workflows/release.yml:13-14).

The jobs: `verify` (the tag must match Cargo.toml's version; fmt, clippy,
test), `build` (five targets — musl Linux x64 and ARM64, macOS x64 and
ARM64, Windows x64 as a zip — each archive containing both binaries plus
README, LICENSE and CHANGELOG, named to match the binstall metadata;
`actions/attest-build-provenance@v4` attests every archive on tag builds,
release.yml:135-137), `checksums` (downloads all five archives, requires
exactly five, writes `SHA256SUMS` with `sha256sum`, release.yml:148-166),
`install-test` (on all three desktop operating systems, through the plain
archive and through cargo-binstall's metadata against a local mirror of
the release layout, via `scripts/install-check.sh`, release.yml:175-206),
and `release` (notes cut from the CHANGELOG's version section plus a fixed
install appendix, published with `softprops/action-gh-release@v3`,
prerelease when the tag contains a hyphen, release.yml:208-260).

### Verification: `.github/workflows/verify-release.yml`

Run by hand after a release and `cargo publish`: a `preflight` job
requires the GitHub release to have its archives and `SHA256SUMS`, and the
version to return HTTP 200 from the crates.io API (.github/workflows/verify-release.yml:21-46);
an `install` matrix (four operating systems) checks `cargo binstall` and
the plain archive (.github/workflows/verify-release.yml:48-74); and an `action` matrix
(three operating systems) runs the action as checked out at the tag,
downloading that release, and asserts `errors = 1` and `v<version> =
<tag>` (.github/workflows/verify-release.yml:76-108). It writes nothing
(.github/workflows/verify-release.yml:1-8).

## 17. Known limitations at v0.4.0

`docs/limitations.md` is the authoritative list; the shape of it:

- **It reads the test, not the protocol.** No knowledge of deployed access
  control ("a role it cannot see is a role it cannot flag"); privileged
  steps in `setUp()` are treated as configuration and not reported; a
  mocked or etched dependency is reported as "the PoC stopped testing the
  real thing, not that the result is wrong" (docs/limitations.md:5-17).
- **It reads Solidity, not the toolchain.** `foundry.toml` is invisible (a
  fork pinned there still trips RG11); assertions in external helper
  contracts do not count for RG08; `invariant_*` and stateful fuzz entry
  points are not checked; Yul blocks are skipped (docs/limitations.md:19-26).
- **It approximates.** Control flow is flattened; data flow follows plain
  assignments and a few levels of aliasing, not storage or the return
  values of external calls; Solidity's C3 linearization is approximated
  depth-first (docs/limitations.md:28-34).
- **In Rust (Solana) tests.** No types are resolved (harness calls are
  matched by name); only the test's own file and `tests/`-directory Rust
  files are inlined, so a helper crate is not read; instructions are
  opaque (the tool reads a transaction's signers, not account metas);
  roles are read from names, `set_authority`-style calls and role fields;
  where the exploit starts is inferred from transaction and helper names;
  Mollusk account lists are not read for RG04 and RG06; cluster settings
  are invisible; RG07 is not checked; the precision is unmeasured
  (docs/limitations.md:36-60).
- **In Anchor TypeScript tests.** The same name-based recognition; only
  Solana-importing files are read; mocha's order is approximated by source
  order and tests created in a loop or by a function are not found; a
  setup test is recognised by its title's leading verb; spl-token's
  JavaScript helpers' own transactions are seen without their signers;
  `Anchor.toml` is not read; `should`-style assertions and custom
  assertion helpers outside `tests/` are not recognised; the precision is
  unmeasured (docs/limitations.md:62-85).
- **A clean result is not a valid PoC** — the closing statement of both
  the README and the limitations page: "It cannot tell you that the bug is
  real, that the impact is what the report says, that the program pays for
  it, or that nobody reported it first" (docs/limitations.md:87-93).

The how-it-works page closes with what the tool never does: it never runs
Solidity, compiles it, or calls `forge`; never builds or runs Rust, never
calls `cargo`; never runs TypeScript, never calls `node`, `anchor` or
`yarn`; "It never opens a network connection. RPC URLs in a test are text
to it"; and it never writes anywhere except the file named by `--output`
and, for `init`, `./realism.yaml` (docs/how-it-works.md:245-252). CI
enforces the network half mechanically (the `no network code` job), and
`SECURITY.md` makes the same three guarantees the security policy's core:
no network reach, no writes or execution, and nothing published that
identifies a live, unfixed target (SECURITY.md:1-16).
