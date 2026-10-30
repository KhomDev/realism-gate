# How realism-gate works

realism-gate never runs a test. It reads the Solidity, rebuilds what the test
would do in the order Foundry would do it, and checks that sequence against the
rules. This page describes each stage and, for each, what it can and cannot
see.

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

## What it never does

- It never runs Solidity, compiles it, or calls `forge`.
- It never opens a network connection. RPC URLs in a test are text to it.
- It never writes anywhere except the file named by `--output` and, for
  `init`, `./realism.yaml`.
