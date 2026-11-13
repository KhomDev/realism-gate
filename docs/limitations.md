# What realism-gate cannot tell you

A tool that checks proofs should be exact about what it does not check.

## It reads the test, not the protocol

- It does not know the deployed contract's access control. "This address is
  privileged" is inferred from names, labels, getters and grants in the test.
  A role it cannot see is a role it cannot flag, and a name that merely sounds
  privileged will be flagged (`docs/rules/RG02.md` lists the known cases).
- It does not know whether the configuration a test sets up matches the real
  deployment. A privileged setter alone in `setUp()` is still read as
  configuration. A direct storage write is an error in `setUp()` and in the
  exploit, including when it is spelled `stdstore`, `setArbitraryStorage` or
  `copyStorage`. A privileged prank in `setUp()` next to that write is RG19.
  RG20, when `--target` and `--target-commit` are set, checks that cited
  names exist in that tree. It does not run the protocol.
- It does not know whether a mocked or etched dependency behaves differently
  from the real one. RG05 and RG07 report that the PoC stopped testing the real
  thing, not that the result is wrong.

## It reads Solidity, not the toolchain

- Fork settings in `foundry.toml` or on the command line are invisible, so a
  fork pinned there still trips RG11. Use an `allow` comment.
- Code in other contracts the test calls externally is not followed. An
  assertion inside an external helper contract does not count for RG08.
- `invariant_*` and stateful fuzz entry points are not checked in v0.1.
- Yul blocks are skipped.

## It approximates

- Control flow is flattened. A prank inside an `if` counts whether or not the
  branch runs.
- Data flow is followed through plain assignments and a few levels of aliasing,
  not through storage, return values of external calls, or arrays.
- Solidity's C3 linearization is approximated depth first. For ordinary test
  hierarchies the most derived definition is the same.

## In Rust (Solana) tests

- No types are resolved. Harness calls are recognised by method and path
  names (`set_account`, `Transaction::new_signed_with_payer`); a wrapper in a
  crate the tool does not read hides what it does. A transaction such a
  wrapper builds and the test sends is still seen, but without its signers.
- Only the test's own file and Rust files under a `tests/` or `test/`
  directory are inlined. A helper crate (a dev-dependency) is not read.
- Instructions are opaque. The tool does not decode instruction data or
  account metas, so it does not know which accounts an instruction writes or
  which signatures the program checks; it reads the transaction's signers.
- Roles are read from names, `set_authority`-style calls and role fields of
  struct literals. An instruction builder that takes the admin as a
  positional argument (`init_ix(&attacker.pubkey())`) grants nothing the
  tool can see.
- Where the exploit starts is inferred from transaction and helper names
  (see [how-it-works](how-it-works.md#rust-solana-tests)). A setup written
  inline, with instructions not named for setup, moves it earlier.
- Mollusk tests construct every account by design. Their account lists are
  not read for RG04 and RG06.
- Cluster settings (`solana-test-validator --clone`, a snapshot) are
  invisible, as `foundry.toml` is for Solidity.
- RG07 is not checked: Solana harnesses have no call mocking.
- The precision of the Rust rules has not been measured
  ([precision](precision.md#rust-solana-tests)).

## In Anchor TypeScript tests

- No types are resolved, as in Rust: harness calls are recognised by method
  and path names (`setAccount`, `.rpc()`), and the provider's wallet by the
  name `wallet` or `provider`.
- Only files that import a Solana package are read, and only the test's own
  file and TypeScript files under a `tests/` or `test/` directory are inlined.
  A helper package (`app/`, a workspace dependency) is not read.
- Mocha's order is approximated by source order: the top level, `describe`
  bodies, `before` hooks, every earlier test, then `beforeEach` hooks. Tests
  created in a loop or by a function (`[..].forEach(x => it(..))`,
  `describe.each`) are not found.
- Whether an earlier test builds the world is read from its title's leading
  verb (`initializes`, `creates`, `funds`, ...). A setup test with another
  title moves the start of the exploit earlier.
- spl-token's JavaScript helpers (`createMint`, `mintTo`, `transfer`) send
  their own transactions; the tool sees the calls, not their signers.
- `Anchor.toml` is not read: `[test.validator]` clones and the provider
  cluster are invisible, as `foundry.toml` is for Solidity.
- Assertions are chai's `assert` and `expect`, node's `assert` and jest's
  `expect`. `should`-style assertions and custom assertion helpers outside
  `tests/` are not recognised.
- The precision of the TypeScript rules has not been measured
  ([precision](precision.md#anchor-typescript-tests)).

## Economic rules (RG15-RG18)

- RG15 cannot see a fee the test never mentions. A token balance is often already net of a pool fee. The rule fires when the test computes a cost and the profit assertion ignores it, or when another test in the same file asserts the flow loses money under a fee.
- RG16 does not read the chain. The bounds are fixed (10^30 raw units anywhere, 10^27 on a pinned fork). A deal under the bound can still be far larger than the pinned block, and a comment that mislabels a unit is not detectable.
- RG17 does not read a protocol's documentation. It flags `ship`, `dock`, `push` and a stand-in deployment when `deployment.evidence` does not cover them. A citation is trusted, not checked. A local `new Protocol()` by itself is not this rule.
- RG18 cannot tell a revert that locks funds from a revert that locks nothing, unless the test asserts the balance.

## Move tests

The Move reader is not a compiler. It finds `#[test]` functions and the calls in their bodies. Specs, types and control flow are not reconstructed. Sui `test_scenario` and Aptos `#[test(signer = @addr)]` are both read. The checked rules are RG01, RG02, RG06, RG08, RG10 and RG18.

## `verify-fix` runs the test twice

`check` does not. `verify-fix` is opt-in. It runs the PoC on a copy of the unpatched tree (that run has to pass) and on the patched tree (that run has to fail). A PoC that fails on both sides is not a fix. The verdict is forge's JSON plus its exit code, not a search of the log text. A compile error is a failure to run, not a failed PoC.

The patch is applied inside the temp copy. Paths that are absolute or contain `..` are refused. `foundry.toml` with `ffi = true` is refused. On a host, the command refuses unless `--sandbox` (or a container / `REALISM_GATE_SANDBOX=1`) or `--allow-host`. See [verify-fix](verify-fix.md). Forge may open a connection if the test forks; this program still has no network code of its own.

## Claims and binding

`--report` binds a report's claims to the PoC. What that cannot do, field by
field:

- **Heuristic extraction is deliberately small.** With no `claims.yaml` and no
  front-matter block, it reads backticked identifiers, the first heading's
  impact verb, and "any user"/"unprivileged"/role words. A claim phrased any
  other way is not found, the binding is labelled **inferred**, and an
  inferred run exits non-zero however good the rows look. Write the block.
- **A claim it cannot parse is `NOT CHECKABLE`, not wrong.** An
  `impact: other:<text>` that names neither a revert nor a value movement, a
  `precondition` whose words match no class, and an `attacker: role:<NAME>`
  all have no predicate. A triager sees `NOT CHECKABLE` and has to read the
  argument; the tool does not supply one.
- **`attacker: role:<NAME>` is never shown.** The test cannot prove an
  outside attacker holds a protocol role. State the role in `realism.yaml`
  and argue it in the report.
- **An asset has to be named in the PoC or in `cited`.** The predicate is a
  name match over the PoC source and the `cited` list; an asset the PoC calls
  something else is `NOT CHECKABLE`, and a name match with no value assertion
  is `NOT SHOWN`.
- **An amount has to be measured.** Equality between two quantities, or a
  read before the exploit and a read after it. A loss figure in prose is not
  an amount the tool can check.
- **A revert impact needs a negative control.** `freeze`/`halt`/`dos` are
  shown only when a sibling test in the same suite performs the same external
  call without the exploit, asserts success and does not expect a revert.
  Without it the revert may have nothing to do with the exploit: `NOT SHOWN`.
- **Contradictions inherit the rule heuristics.** `any-user` is contradicted
  by `RG01`/`RG02` or by a sender the identity reader thinks is privileged.
  Those are the same name-and-label guesses the rules use, so a false positive
  there becomes a `CONTRADICTED` claim here. Read the evidence column.
- **Citations need a git target.** `cited` is `NOT CHECKABLE` without
  `--target DIR --target-commit REV`; the working tree is not trusted, and a
  target that is not a git checkout fails closed (`RG20`).
- **`fix` is never shown by `check`.** `check` never applies a patch. Until
  `verify-fix` is run on the patch and its verdict quoted, the fix row is
  open.
- **`severity` is accepted but not bound.** The tool records it so a report's
  claim round-trips, and does not judge it.
- **The table is not a verdict.** A `SHOWN` row means the evidence the
  predicate looks for is present. It does not mean the bug is real, that the
  claim is the interesting one, that the severity is right, or that a program
  pays. It is not a duplicate check and it is not a validity oracle.
- **A non-fix `NOT SHOWN` row does not, by itself, set exit 1.** The outcome
  says `INCOMPLETE`; the exit status is reserved for a contradiction, an
  inferred binding and `--fail-on`. Gate on `claims.outcome` in the JSON if
  every claim has to be shown.
- **Claim-binding precision is measured for one field.** [Precision](precision.md#claim-binding)
  publishes impact (drain) at 98.9%, n=91 flagged rows, Wilson 95%
  [94.0-99.8], on 120 public PoC + incident-record pairs. The attacker field
  is not interpretable with that corpus's labels, `amount` is not checkable by
  design, and no overall-outcome or recall number is claimed.

## A clean result is not a valid PoC

realism-gate finds known ways a PoC proves less than it claims. It cannot tell
you that the bug is real, that the impact is what the report says, that the
program pays for it, or that nobody reported it first. A PoC with zero findings
can still be wrong, and a PoC with findings can still be right once the author
has justified each one. The same is true of a claim table: `SHOWN` rows mean
the predicates found their evidence, not that the finding is sound.

For the other half of that question (whether a finding is new, in scope and
payable), see [priorart](https://github.com/KhomDev/priorart).
