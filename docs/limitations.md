# What realism-gate cannot tell you

A tool that checks proofs should be exact about what it does not check.

## It reads the test, not the protocol

- It does not know the deployed contract's access control. "This address is
  privileged" is inferred from names, labels, getters and grants in the test.
  A role it cannot see is a role it cannot flag, and a name that merely sounds
  privileged will be flagged (`docs/rules/RG02.md` lists the known cases).
- It does not know whether the configuration a test sets up matches the real
  deployment. RG02 flags a privileged step in the exploit; a privileged step in
  `setUp()` is treated as configuration and not reported at all.
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

## A clean result is not a valid PoC

realism-gate finds known ways a PoC proves less than it claims. It cannot tell
you that the bug is real, that the impact is what the report says, that the
program pays for it, or that nobody reported it first. A PoC with zero findings
can still be wrong, and a PoC with findings can still be right once the author
has justified each one.

For the other half of that question (whether a written finding survives
triage), see [viability-gate](https://github.com/KhomDev/viability-gate).
