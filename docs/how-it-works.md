# How realism-gate works

realism-gate never runs a test. It reads the Solidity, rebuilds what the test
would do in the order Foundry would do it, and checks that sequence against the
rules. This page describes each stage and, for each, what it can and cannot
see. Rust tests for Solana programs go through the same stages with their own
parser and vocabulary; [the last section](#rust-solana-tests) describes how
each one differs.

## 1. Parsing

Every `.sol` file under the given paths is parsed with
[`solar`](https://github.com/paradigmxyz/solar), the Solidity parser written
for the Foundry toolchain. `lib/`, `node_modules/`, `out/`, `cache/`,
`broadcast/`, `target/`, `artifacts/` and hidden directories are skipped unless
named explicitly.

The parser's AST lives in an arena that is freed when the parse ends, so each
file is lowered into a small owned model (`src/ir.rs`): contracts, state
variables, functions, modifiers, statements and expressions, each with its
source text and position. Control flow is flattened: the bodies of `if`,
`for`, `while`, `try` and nested blocks appear inline in source order. The
rules reason about what a test does, not which branch runs, so the flattening
is deliberate.

A file that does not parse cleanly is not dropped. The parser recovers, and
whatever it recovered is checked; the report lists the file under "could not
be parsed completely".

## 2. Suites and inheritance

A suite is a concrete contract with at least one public or external function
whose name starts with `test`. Its bases are resolved by name, a definition in
the same file first, then the first definition in path order. The same-file
preference matters for PoC collections that reuse one suite name in hundreds
of files.

The linearization is depth first, most derived first. That approximates
Solidity's C3 order closely enough to find the most derived definition of
`setUp`, a helper or a modifier, and to resolve `super.setUp()`.

## 3. The trace

For each test, `src/trace.rs` builds the ordered list of what runs:

1. State-variable initializers and constructors, base to derived.
2. `setUp()`, with `super.setUp()` and `Base.setUp()` followed.
3. The test function, wrapped in its modifiers. A modifier's body runs in
   place and its `_;` runs the rest of the chain, so a `balanceLog` modifier's
   before and after reads land around the test body.
4. Callbacks: externally callable functions of the suite that are not tests or
   `setUp` (`onFlashLoan`, `uniswapV2Call`, `receive`, `fallback`). A protocol
   calls these during the exploit, so they are part of it.

Calls to the test contract's own functions are inlined at the call site, up to
eight levels deep, with recursion cut. Expressions are visited in evaluation
order, so in `vault.withdraw(token.balanceOf(me))` the read comes before the
withdrawal.

Every call is classified: a `vm.*` cheatcode, a forge-std helper (`deal`,
`hoax`, `makeAddr`), an assertion (`assert*`, `require`, `vm.expect*`), a log,
a builtin or library call, a cast, an internal call, an external call, or a
deployment. Cheatcode receivers named `vm`, `cheats`, `hevm` and `cheat` are
recognised, which covers harnesses that declare the cheatcode interface
themselves.

## 4. Who sends each call

The trace is replayed to work out the sender of every external call and
deployment. `vm.prank` and `hoax` switch the sender for the next call;
`vm.startPrank`, `changePrank` and `startHoax` until stopped; `broadcast` and
`startBroadcast` likewise. Each phase starts as the test contract, which is
how Foundry runs `setUp()`, the test, and a callback.

## 5. Who is who

`src/identity.rs` decides what each address is presumed to be:

| Class | Evidence |
|---|---|
| attacker | declared in `realism.yaml`; or a name or `makeAddr` label such as `attacker`, `exploiter`, `hacker`; or an instance of an attack contract written next to the test |
| privileged | a role word in the name or label (`owner`, `admin`, `governance`, `keeper`, `guardian`, ...); assigned from a getter such as `vault.owner()` or `getRoleMember`; or granted a role by the harness (`grantRole`, `transferOwnership`, `setOperator`, ...) |
| user | a user word (`alice`, `victim`, `whale`, `depositor`, ...), or made with `makeAddr`, `vm.addr` or `address(uint160(n))` |
| mock, helper | a mock word in the name or type, or a contract defined in the test's own file |
| protocol | deployed by the harness with `new`, or a contract-typed variable, or a hard-coded address constant that is none of the above |

"Owner" counts as a role only on its own or with a protocol word
(`protocolOwner`), never as `tokenOwner` or `nftOwner`. A declaration replaces
the attacker heuristic entirely: once `attacker.identities` is set, names stop
counting.

This is the part of the tool most likely to be wrong, and the part a
declaration exists to correct. It cannot read the deployed contract's access
control; it reads what the test says about it.

## 6. Rules

Each rule reads the trace, the senders and the identity facts, and emits raw
findings. The engine then:

- drops rules excluded by `--rules` and `--skip`;
- de-duplicates by rule and source position, so a `setUp()` line shared by ten
  tests is one finding that lists ten tests;
- applies inline `allow` comments, ignoring any that give no reason;
- sorts by file, line and rule.

The rule catalog in `src/rules/catalog.rs` is the single source of truth for
rule text: `realism-gate rules`, `realism-gate explain`, the SARIF rule table
and the pages in `docs/rules/` are all generated from it, and a test fails if
the pages drift.

## Rust (Solana) tests

`.rs` files are parsed with [`syn`](https://github.com/dtolnay/syn) and lowered
into the same expression and statement model as Solidity (`src/solana/`).
Blocks are flattened the same way: the bodies of `if`, `match`, loops and
closures run inline, in source order. `if cond { panic!() }` counts as
`assert!(!cond)`.

**Which tests.** A function with an attribute whose last segment is `test`
(`#[test]`, `#[tokio::test]`, `#[test_log::test]`), `#[rstest]` or
`#[test_case]`, in any module. It is checked only if its trace drives a
Solana harness: it builds a `ProgramTest`, `LiteSVM` or `Mollusk`, builds or
sends a transaction, or writes, loads or airdrops to an account. A suite is
the tests of one module of one file, named by the file stem and module path.

**The trace.** As in Solidity, helper calls are inlined at the call site, up
to eight levels deep, with recursion cut. A helper is a function in the test's
own file, or in shared test code: any `.rs` file under a directory named
`tests` or `test`, such as `tests/common/mod.rs`. Program source is never
inlined; a test reaches it through transactions. A call resolves to a helper
only when the name and the number of arguments match, and a harness method
(`set_account`, `send_transaction`) never resolves to a fixture method of the
same name. Each inlined parameter is bound to the caller's argument, so a
transaction a generic `send(svm, ix, signers)` wrapper signs with `signers`
is seen as signed by whatever the test passed.

**Harness calls** are recognised by name, not type (`src/solana/harness.rs`):

| Event | Calls |
|---|---|
| transaction, with its signers | `Transaction::new_signed_with_payer`, `Transaction::new`, `VersionedTransaction::try_new`, `tx.sign(..)`; a Mollusk `process_*` run; an anchor-client `program.request()` .. `.signer(&kp)` .. `.send()`, also signed by the client's payer; anchor-litesvm's `ctx.execute_instruction(ix, signers)`; a `send_transaction` / `process_transaction` whose transaction was built where the trace cannot see |
| account write | `set_account`, `add_account` |
| account copied in | `add_account_with_file_data`, `add_account_with_base64_data`; a `set_account` whose value comes from an RPC fetch, `include_bytes!`, a file or JSON |
| program load | `ProgramTest::new`, `add_program`, `add_program_from_file`, `add_upgradeable_program_to_genesis`, `Mollusk::new`, `AnchorLiteSVM::build_with_program` |
| live cluster | `RpcClient::new*` and anchor-client's `Client::new`, unless the URL is `localhost` or `127.0.0.1` or the cluster `Cluster::Localnet` |

An `assert_*` helper method (`svm.assert_token_balance(..)`) is an assertion;
`assert_success()` only checks that the transaction did not fail and is not.

**Where the exploit starts.** A Solana test has no `setUp()`. The exploit
starts at the first transaction that does not set the world up. A
transaction sets the world up when it is sent from a helper whose name says
so (`setup`, `init_*`, `create_*`, `TestEnv::new`, ...), or when nobody who
reads as the attacker signs it and its instructions are named `init`,
`initialize`, `create`, `setup`, `configure` or the like. Everything before
it is setup, and a cheat there is reported one level lower, as in `setUp()`.

**Who is who** (`src/solana/env.rs`). An account the test derives with
`find_program_address` is the program's, whoever it describes; an associated
token account belongs to its owner's class; a `pubkey!` or `Pubkey::from_str`
constant is a fixed address of the protocol; then names: attacker words, the
harness's `payer`, user words, role words (with `user_authority` and
`attacker_owner` excluded), and protocol-state words (`vault`, `pool`,
`config`, `market`, ...). A role is also granted by a `set_authority` or
`set_admin`-style call, or by a role field (`admin`, `authority`, ...) in a
struct whose name initializes or reconfigures (`Initialize`, `UpdateConfig`).
A `realism.yaml` declaration overrides the attacker heuristic, as in Solidity.

**Placement.** A finding about a transaction is placed at the call that
sends it from the test (or from the helper the test calls), with that call as
the evidence, rather than inside a generic `send` wrapper.

## Anchor TypeScript tests

`.ts`, `.mts`, `.cts`, `.js`, `.mjs` and `.cjs` files (not `.d.ts`) are
parsed with [oxc](https://github.com/oxc-project/oxc), the parser of the Oxc
JavaScript toolchain, and read only if they import a Solana package
(`@coral-xyz/anchor`, anything under `@solana/`, `solana-bankrun`,
`anchor-bankrun`, `litesvm`, `anchor-litesvm`), by `import` or `require`.
Other JavaScript in the tree, a Hardhat suite or a bundler config, is not
read and not counted.

Each file is lowered into the same model as Rust (`src/solana/parse_ts.rs`),
spelled the way the Rust harnesses spell it, so everything after the parser
is shared with Rust:

- every called name is in snake case (`svm.setAccount` is `set_account`);
  variables and fields keep their names;
- a call on an imported name or a class is a path call
  (`anchor.web3.Keypair.generate()` is `anchor::web3::Keypair::generate`),
  `new T(..)` is `T::new(..)`, `this` is `self`;
- `kp.publicKey` is `kp.pubkey()`, `new PublicKey("..")` is a fixed address,
  `getAssociatedTokenAddress(mint, owner)` takes the owner first as in Rust;
- `await`, `as`, `!`, `new BN(x)`, `BigInt(x)`, `.toNumber()`, `.toString()`
  are their operand; an object literal is a struct literal with no type name;
- chai, node and jest assertions are `assert!`, `assert_eq!` and `assert_ne!`
  when they compare (`expect(a).to.be.greaterThan(b)` is `assert!(a > b)`),
  BN comparisons (`a.gt(b)`) are operators, and `if (c) throw` is
  `assert!(!c)`; `console.log` is a log.

**Which tests.** Every `it(..)`, `test(..)` and `specify(..)` callback,
including `.only`; `.skip`, `.todo`, `xit` and `xdescribe` are not run and not
read. A suite is a `describe` block, named by its titles (`vault > withdraw`).
As in Rust, a test is checked only if its trace drives a Solana harness.

**What runs first.** Before each test the trace walks what mocha runs before
it: the file's top level, the enclosing `describe` bodies and their `before`
hooks, every test above it in the file (mocha runs them in order, and the
test inherits their state), then the `beforeEach` hooks. The hooks build the
world. An earlier test builds the world when its title's leading verb says so
(`it("initializes the pool")`, `it("is initialized")`, `it("funds the
vault")`); such a test is not checked on its own when later tests build on it.

**Helpers** are function declarations, functions bound to a name
(`const send = async (..) => ..`) and class methods, in the test's file or in
any TypeScript file under a `tests` or `test` directory. A call resolves when
the number of arguments fits, counting defaults and rest parameters.

**Harness calls:**

| Event | Calls |
|---|---|
| transaction, with its signers | `program.methods.x(..)` .. `.rpc()`, signed by `.signers([..])` and the provider wallet; `program.rpc.x(.., { accounts, signers })`; `provider.sendAndConfirm(tx, signers)`; `sendAndConfirmTransaction(connection, tx, signers)`; `connection.sendTransaction(tx, signers)`; `tx.sign(..)` before `svm.sendTransaction(tx)` or `banksClient.processTransaction(tx)` |
| account write | `svm.setAccount`, `context.setAccount`, each account handed to bankrun's `startAnchor` or `start` |
| program load | `svm.addProgram`, `svm.addProgramFromFile`, each program handed to `startAnchor` or `start` |
| airdrop | `connection.requestAirdrop`, `svm.airdrop` |
| live cluster | `new Connection(url)`, unless the URL, or the constant holding it, is `localhost`, `127.0.0.1` or the provider's own `rpcEndpoint` |

The provider wallet (`provider.wallet`, named `wallet`) signs every `.rpc()`
and `sendAndConfirm` as the fee payer. It counts as a signer of a transaction
only when it is the only signer or the instruction names it in its accounts:
a wallet that is the admin and also pays for the attacker's transaction did
not authorize it.

**Who is who** is the Rust logic: a `findProgramAddressSync` result is the
program's, an associated token account is its owner's, a
`new PublicKey("..")` constant is a fixed address. `provider.wallet` and
other names with `wallet` or `provider` read as the payer, after attacker
and user names. A role is granted by a role field in the `.accounts({ .. })`
of an instruction that initializes or reconfigures (`initialize`,
`setConfig`, ...). `TOKEN_PROGRAM_ID` and the other spl-token and web3.js
program constants are not the program under test.

## What it never does

- It never runs Solidity, compiles it, or calls `forge`. It never builds or
  runs Rust either, and never calls `cargo`; it never runs TypeScript, and
  never calls `node`, `anchor` or `yarn`.
- It never opens a network connection. RPC URLs in a test are text to it.
- It never writes anywhere except the file named by `--output` and, for
  `init`, `./realism.yaml`.
