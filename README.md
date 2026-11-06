# realism-gate

[![ci](https://github.com/KhomDev/realism-gate/actions/workflows/ci.yml/badge.svg)](https://github.com/KhomDev/realism-gate/actions/workflows/ci.yml)
[![license: MIT](https://img.shields.io/badge/license-MIT-informational.svg)](LICENSE)

**A static checker for proof-of-concept tests, for Foundry (Solidity) and
for Solana programs (Rust, and Anchor's TypeScript). It asks one question: does this PoC show what an
unprivileged attacker can do, or only what the harness made true?**

A PoC that passes `forge test` can still prove nothing. The exploit step runs
under `vm.prank(admin)`. The "donation" is a `deal()` that overwrites the
vault's balance. The broken state is written with `vm.store`. The profit check
is `assertGt(balance, 0)` on an address that already held tokens. A Solana PoC
fails the same ways: the exploit transaction is signed by the admin keypair,
the vault's balance is written with `set_account`, the pool is read from a
live RPC. Each of these passes locally and fails triage, and each is visible
in the test's source.

realism-gate reads the source, rebuilds the order the test runs in, works out
who sends or signs every call, and reports those patterns with the line, the
reason and the fix.

- **Reads, never runs.** No compilation, no `forge`, no `anchor test`, no RPC. Your PoC never
  leaves the machine; the binary has no network code (CI checks the dependency
  graph and the source).
- **A real parser.** It uses [solar](https://github.com/paradigmxyz/solar), the
  Solidity parser built for the Foundry toolchain,
  [syn](https://github.com/dtolnay/syn) for Rust and
  [oxc](https://github.com/oxc-project/oxc) for TypeScript, not regular
  expressions. It follows inheritance, `super.setUp()`, modifiers, internal
  helpers and flash-loan callbacks; in a Solana test it follows the test's
  helpers and shared test files, with each helper's parameters bound to what
  the caller passed, and in a mocha suite the hooks and the tests that run
  first.
- **Every finding can be argued with.** Each rule documents why it exists and
  how it can be wrong. A finding is silenced with a comment that must give a
  reason.

```bash
realism-gate check test/     # Foundry
realism-gate check tests/    # a Solana program's Rust or Anchor TypeScript tests
```

## What it looks like

This is real output. `tests/docs.rs` regenerates it from
[`examples/vault-drain`](examples/vault-drain/test/VaultDrain.t.sol) and fails
if the README has drifted.

<!-- BEGIN:example -->

```text
realism-gate 0.4.0  ·  1 file  ·  1 suite  ·  1 test

test/VaultDrain.t.sol
  warn   RG11 unpinned-fork  test/VaultDrain.t.sol:17:9  VaultDrainTest::test_attackerDrainsVault
         The fork has no block number, so it runs against the chain as it is today. Balances,
         prices and the bug itself can change before a triager runs it.
         ↳ vm.createSelectFork(vm.envString("RPC_URL"))
         → Pass the block: `vm.createSelectFork(url, BLOCK)`, and say in the report why that
         block is representative.

  warn   RG02 privileged-step-in-exploit  test/VaultDrain.t.sol:24:9  VaultDrainTest::test_attackerDrainsVault
         The exploit runs a step as `admin`: the name `admin` reads as the `admin` role. An
         attacker cannot sign as this address. Move it to setUp() if it is configuration, or
         declare it under `privileged:` in realism.yaml if the claim relies on that role acting.
         ↳ vm.prank(admin)
         → Move configuration into `setUp()`.

  error  RG06 balance-injection  test/VaultDrain.t.sol:28:9  VaultDrainTest::test_attackerDrainsVault
         deal() sets the `token` balance of `vault` (Vault) in the exploit. It transfers nothing
         and overwrites the balance the contract had, so its internal accounting never sees it.
         Fund the attacker and transfer instead.
         ↳ deal(address(token), address(vault), 1_000_000e18)
         → Fund the attacker with `deal`, then `transfer` from the attacker to the contract.

  warn   RG10 no-baseline  test/VaultDrain.t.sol:33:9  VaultDrainTest::test_attackerDrainsVault
         The asserted value `token.balanceOf(attacker)` is read only after the attack. Without a
         before-value the test cannot tell a gain from a balance that was already there.
         ↳ assertGt(token.balanceOf(attacker), 0)
         → Read the same quantity before and after, and assert on the difference.

SUMMARY  1 error  ·  3 warnings  ·  0 notes  ·  0 suppressed
RESULT   FAIL  (fails on error and above)

realism-gate reads the test; it does not run it. A clean result means no known
defect pattern was found, not that the PoC is valid. `realism-gate explain <RULE>`
shows why a rule exists and how it can be wrong.
```

<!-- END:example -->

## Solana programs (Rust and Anchor TypeScript)

Solana tests are checked with the same rule ids, the same output and the same
`allow` comments:

- **Rust:** tests written with [solana-program-test](https://crates.io/crates/solana-program-test),
  [LiteSVM](https://github.com/LiteSVM/litesvm),
  [Mollusk](https://github.com/anza-xyz/mollusk),
  [anchor-client](https://crates.io/crates/anchor-client) or
  [anchor-litesvm](https://crates.io/crates/anchor-litesvm). A `#[test]` is
  checked when it drives one of those harnesses; other Rust tests in the tree
  are skipped.
- **TypeScript:** Anchor's mocha or jest tests, the ones `anchor test` runs,
  with the Anchor client (`program.methods.x().signers([..]).rpc()`),
  web3.js, [bankrun](https://github.com/kevinheavey/solana-bankrun) or
  LiteSVM. Only files that import a Solana package are read, so a Hardhat
  suite in the same tree is left alone. `describe` bodies, `before` hooks and
  the tests above a test run first, as mocha runs them; a test titled for
  setup (`it("is initialized")`) is the next test's setup, not a PoC of its
  own.

This is real output for
[`examples/solana-vault-drain`](examples/solana-vault-drain/tests/vault_drain.rs),
regenerated by `tests/docs.rs` like the one above:

<!-- BEGIN:example-rust -->

```text
realism-gate 0.4.0  ·  1 file  ·  1 suite  ·  1 test

tests/vault_drain.rs
  warn   RG11 unpinned-fork  tests/vault_drain.rs:21:15  vault_drain::test_attacker_drains_vault
         The test creates a client for a live cluster, so every account it reads through it is
         the chain as it is today. Balances, prices and the bug itself can change before a
         triager runs it.
         ↳ RpcClient::new(std::env::var("RPC_URL").unwrap())
         → Dump each account the PoC needs at a recorded slot (`solana account <ADDRESS>
         --output-file <FILE>`) and load it from the file, and say in the report why that slot
         is representative.

  warn   RG02 privileged-step-in-exploit  tests/vault_drain.rs:30:5  vault_drain::test_attacker_drains_vault
         The exploit sends a transaction signed by `admin`: the name `admin` reads as the
         `admin` role. An attacker cannot produce that signature. Move it before the exploit if
         it is configuration, or declare it under `privileged:` in realism.yaml if the claim
         relies on that role acting.
         ↳ send(&mut svm, set_withdraw_fee_ix(&admin.pubkey(), 0), &[&admin])
         → Move configuration before the attacker's first transaction.

  error  RG06 balance-injection  tests/vault_drain.rs:32:5  vault_drain::test_attacker_drains_vault
         set_account sets the lamports of `vault_state` (a program-derived address) in the
         exploit. It transfers nothing and overwrites what the account held, so the program's
         own accounting never sees it. Fund the attacker and transfer instead.
         ↳ svm.set_account(vault_state, Account { lamports: 1_000_000 * LAMPORTS_PER_SOL, ..state })
         → Fund the attacker's own accounts, then transfer from the attacker to the program's
         account.

  warn   RG10 no-baseline  tests/vault_drain.rs:34:5  vault_drain::test_attacker_drains_vault
         The asserted value `svm.get_balance(&attacker.pubkey())` is read only after the attack.
         Without a before-value the test cannot tell a gain from a balance that was already
         there.
         ↳ assert!(svm.get_balance(&attacker.pubkey()).unwrap() > 0);
         → Read the same quantity before and after, and assert on the difference.

SUMMARY  1 error  ·  3 warnings  ·  0 notes  ·  0 suppressed
RESULT   FAIL  (fails on error and above)

realism-gate reads the test; it does not run it. A clean result means no known
defect pattern was found, not that the PoC is valid. `realism-gate explain <RULE>`
shows why a rule exists and how it can be wrong.
```

<!-- END:example-rust -->

The same PoC in Anchor TypeScript, run on bankrun
([`examples/anchor-vault-drain`](examples/anchor-vault-drain/tests/vault-drain.ts)):

<!-- BEGIN:example-anchor -->

```text
realism-gate 0.4.0  ·  1 file  ·  1 suite  ·  1 test

tests/vault-drain.ts
  warn   RG11 unpinned-fork  tests/vault-drain.ts:13:21  vault::lets the attacker drain the vault
         The test creates a client for a live cluster, so every account it reads through it is
         the chain as it is today. Balances, prices and the bug itself can change before a
         triager runs it.
         ↳ new Connection(process.env.RPC_URL!)
         → Dump each account the PoC needs at a recorded slot (`solana account <ADDRESS>
         --output-file <FILE>`) and load it from the file, and say in the report why that slot
         is representative.

  warn   RG02 privileged-step-in-exploit  tests/vault-drain.ts:27:11  vault::lets the attacker drain the vault
         The exploit sends a transaction signed by `admin`: the name `admin` reads as the
         `admin` role. An attacker cannot produce that signature. Move it before the exploit if
         it is configuration, or declare it under `privileged:` in realism.yaml if the claim
         relies on that role acting.
         ↳ program.methods.setWithdrawFee(new BN(0)).accounts({ admin: admin.publicKey, vaultState }).signers([admin]).rpc()
         → Move configuration before the attacker's first transaction.

  error  RG06 balance-injection  tests/vault-drain.ts:29:5  vault::lets the attacker drain the vault
         setAccount sets the lamports of `vaultState` (a program-derived address) in the
         exploit. It transfers nothing and overwrites what the account held, so the program's
         own accounting never sees it. Fund the attacker and transfer instead.
         ↳ context.setAccount(vaultState, { ...state!, lamports: 1_000_000 * LAMPORTS_PER_SOL })
         → Fund the attacker's own accounts, then transfer from the attacker to the program's
         account.

  warn   RG10 no-baseline  tests/vault-drain.ts:31:5  vault::lets the attacker drain the vault
         The asserted value `context.banksClient.getBalance(attacker.publicKey)` is read only
         after the attack. Without a before-value the test cannot tell a gain from a balance
         that was already there.
         ↳ expect(Number(await context.banksClient.getBalance(attacker.publicKey))).to.be.greaterThan(0)
         → Read the same quantity before and after, and assert on the difference.

SUMMARY  1 error  ·  3 warnings  ·  0 notes  ·  0 suppressed
RESULT   FAIL  (fails on error and above)

realism-gate reads the test; it does not run it. A clean result means no known
defect pattern was found, not that the PoC is valid. `realism-gate explain <RULE>`
shows why a rule exists and how it can be wrong.
```

<!-- END:example-anchor -->

| Rule | In a Solana test, fires when |
|---|---|
| RG01 | an instruction that initializes or reconfigures the program makes the attacker `admin` or `authority` (a struct field, or a field of `.accounts({ .. })`), or `set_authority` hands it a role |
| RG02 | an exploit transaction is signed by a keypair that reads as a role or was given one |
| RG03 | the exploit is signed only by the key that set the program up, usually `payer` or `provider.wallet` |
| RG04 | `set_account` / `setAccount`, `add_account` or bankrun's `startAnchor` writes an account of the program: a PDA, a fixed address, one it owns |
| RG05 | a mock program is loaded at a real program's address |
| RG06 | the same writes set the lamports or token balance of an account of the program |
| RG07 | not checked: Solana harnesses have no call mocking |
| RG08 | the test asserts nothing; a transaction that succeeds (`.unwrap()`, `await ..rpc()`) is not an assertion |
| RG09 | an assertion (`assert_eq!`, `assert.equal`, `expect(..).to.equal`) cannot fail |
| RG10 | a gain is asserted on a balance read only after the attack |
| RG11 | the test reads accounts from a live cluster through an `RpcClient`, an anchor-client `Client` or a web3.js `Connection` |
| RG12-RG14 | the same as for Solidity: they read `realism.yaml` |

The exploit starts at the first transaction that does not set the world up (a
setup helper or hook, or an `init`/`create` instruction nobody who reads as
the attacker signs); a write before it is a warning, after it an error.
Anchor's provider wallet signs every `.rpc()` as the fee payer; it counts as
a signer only when it is the only one or the instruction names it. Each rule
page has a Solana section with its fix, Rust and TypeScript examples and the
ways it can be wrong, and [docs/how-it-works.md](docs/how-it-works.md#rust-solana-tests)
describes the frontends. The Solana rules are new (Rust in 0.3.0, TypeScript
in 0.4.0) and their precision has not been measured yet; see
[docs/precision.md](docs/precision.md#rust-solana-tests).

## Install

With [cargo-binstall](https://github.com/cargo-bins/cargo-binstall), a
prebuilt binary in seconds, no Rust toolchain needed:

```bash
cargo binstall realism-gate
```

Or download a release archive for Linux (static musl), macOS or Windows, and
check it against the release's checksums and build provenance:

```bash
gh release download --repo KhomDev/realism-gate --pattern '*x86_64-unknown-linux-musl*' --pattern SHA256SUMS
sha256sum -c SHA256SUMS --ignore-missing
tar xzf realism-gate-*-x86_64-unknown-linux-musl.tar.gz
```

In GitHub Actions, one step (details in [docs/ci.md](docs/ci.md)):

```yaml
- uses: KhomDev/realism-gate@v0
  with:
    path: test/
```

From source with Rust 1.95 or newer:

```bash
cargo install realism-gate --locked
```

Two binaries are installed, `realism-gate` and the short `rgate`. They are the
same program.

## The rules

| Rule | Severity | Fires when |
|---|---|---|
| [RG01](docs/rules/RG01.md) `attacker-holds-privilege` | error | the harness grants the attacker a role, or the attacker *is* the owner |
| [RG02](docs/rules/RG02.md) `privileged-step-in-exploit` | warn | the test body pranks as an owner, admin, keeper, or a protocol contract |
| [RG03](docs/rules/RG03.md) `deployer-is-attacker` | warn | the test deploys the protocol and then attacks it as the deployer |
| [RG04](docs/rules/RG04.md) `storage-write` | error | `vm.store` writes a slot of a contract under review |
| [RG05](docs/rules/RG05.md) `code-replacement` | warn | `vm.etch` or `deployCodeTo` replaces a dependency's code |
| [RG06](docs/rules/RG06.md) `balance-injection` | error | `deal()` writes a protocol contract's balance |
| [RG07](docs/rules/RG07.md) `mocked-call` | warn | `vm.mockCall` makes a dependency return a value it would not |
| [RG08](docs/rules/RG08.md) `no-assertion` | error | the test asserts nothing (warn if it only logs) |
| [RG09](docs/rules/RG09.md) `tautological-assertion` | error | an assertion compares constants or an expression with itself |
| [RG10](docs/rules/RG10.md) `no-baseline` | warn | a gain is asserted on a value read only after the attack |
| [RG11](docs/rules/RG11.md) `unpinned-fork` | warn | a fork has no block number |
| [RG12](docs/rules/RG12.md) `attacker-in-foreign-transaction` | error | the declaration puts an attacker step inside the victim's transaction |
| [RG13](docs/rules/RG13.md) `unordered-same-block` | warn | a same-block step has no ordering mechanism |
| [RG14](docs/rules/RG14.md) `dependent-precondition` | note | the attack depends on a victim, a role or a third party acting |

Severities are graded by phase: a cheat in `setUp()` is usually configuration
and is reported one level lower than the same cheat in the exploit.
`realism-gate explain RG06` prints a rule's full page.

## Usage

```text
realism-gate check [PATHS]...      default: ./test, else ./tests, else .
    --format text|json|sarif|markdown
                                   json and sarif are the stable interfaces
    --output FILE
    --fail-on error|warn|note|never   default: error
    --rules RG04,RG06              run only these
    --skip RG08                    skip these
    --severity RG05=error          set a rule's level (error|warn|note|off)
    --exclude 'test/legacy/**'     do not check or report these paths
    --changed-since origin/main    only report findings in changed files
    --match-test REGEX
    --match-contract REGEX         a Solidity contract, a Rust file and module,
                                   or a TypeScript describe block
    --declaration FILE             default: ./realism.yaml if present
    --no-declaration
    --config FILE                  default: ./realism-gate.toml if present
    --no-config
    --show-suppressed
realism-gate rules [--format json]
realism-gate explain RULE [--markdown]
realism-gate init                  write a realism.yaml template
```

Exit status: `0` no active finding at or above `--fail-on`; `1` there is one;
`2` usage, input, configuration or declaration error, or no test suite found
at all. The JSON format is described by
[docs/output-schema.json](docs/output-schema.json).

### Configuration

An optional `realism-gate.toml` next to where you run it:

```toml
exclude = ["test/legacy/**"]   # not checked, not reported

[rules]
RG11 = "off"                   # error | warn | note | off
RG05 = "error"
```

Flags override it: `--rules` runs a rule the file turns off, `--severity`
replaces a level, `--exclude` adds patterns.

### Only what changed

`--changed-since REF` still reads every file (inheritance needs them) but
reports only findings in files that differ from `REF`, including uncommitted
and untracked ones. It runs the local `git`; it never fetches.

### Silencing a finding

```solidity
// realism-gate: allow(RG11) the block is pinned in foundry.toml
vm.createSelectFork("mainnet");
```

The same comment works in Rust and TypeScript.

`// realism-gate: allow RG11 <reason>` (no parentheses) works too. The comment
applies to its own line and the next one. It must give a reason; a bare
`allow(RG11)` is ignored, and the output counts it (`suppressions_ignored`). Silenced findings
stay in the JSON and SARIF output, marked as suppressed.

### Declaring the attacker model

The rules infer who is who from names and data flow. When that inference is
wrong, or when the attack depends on someone else acting, say so in
`realism.yaml`:

```yaml
version: 1
attacker:
  identities: [mallet]
privileged:
  - identity: keeper
    role: keeper
    reason: harvest() runs on a schedule from a public keeper network
preconditions:
  - step: deposit right before the harvest
    performed_by: attacker
    boundary: same_block
    ordering_guarantee: public mempool; the attacker back-runs the keeper
```

A declaration replaces the name heuristics, silences RG02 for declared roles,
and enables the boundary rules RG12-RG14. See
[docs/declaration.md](docs/declaration.md).

## Measured on public corpora

On the 886 tests of [DeFiHackLabs](https://github.com/SunWeb3Sec/DeFiHackLabs)
at a pinned commit (reproductions of real exploits, not bounty submissions):

- **611 tests (69.0%) assert nothing.** 595 log values a reader has to trust.
- **20 tests** have a finding beyond that.
- The whole corpus is analysed in under a second on a laptop
  ([eval/perf](eval/perf)).

For v0.2.0 a sample of every rule's flags was labelled by reading the source
around each one, on that corpus, on 70 files added to it later, and on a
second corpus never used for tuning. Most rules were right on every labelled
flag. Two were not: RG10 was right on 2 of 5 in-sample flags in 0.1.0 (the
fix brings it to 2 of 2, at the cost of one true positive on the later
files), and RG05 is right on 2 of 5 overall, so it is now a warning, never an
error. Small counts carry wide intervals, and nothing here measures recall.
The tables, the intervals and what the numbers do not cover:
[docs/precision.md](docs/precision.md). Everything is regenerated and compared
in CI.

## What it cannot tell you

It reads the test, not the protocol: it does not know the deployed contract's
access control, only what the test says about it. It does not see
`foundry.toml`. It flattens control flow. In a Solana test it resolves no
types: harness calls are recognised by name, helpers outside the test's file
and `tests/` are not read, and `Anchor.toml` is not read. A clean result means no known defect
pattern was found, not that the PoC is valid, the bug is real, or the program
pays for it. The full list is in [docs/limitations.md](docs/limitations.md),
and how each stage works is in [docs/how-it-works.md](docs/how-it-works.md).

## Related

I build these as one independent researcher, around a single question: why do
security findings not get paid?

| Repository | Question |
|---|---|
| [rejection-taxonomy](https://github.com/KhomDev/rejection-taxonomy) | Why did findings die, and which gate would have caught each? |
| [bounty-economics](https://github.com/KhomDev/bounty-economics) | Can this finding pay at all on this program? |
| [viability-gate](https://github.com/KhomDev/viability-gate) | Will the write-up survive triage? |
| **realism-gate** | **Does the PoC prove what the write-up claims?** |

## Contributing

The most useful contribution is a false positive or false negative reduced to
a synthetic test. See [CONTRIBUTING.md](CONTRIBUTING.md). Do not attach a PoC
for a live, unfixed finding anywhere in this repository.

## License

MIT. See [LICENSE](LICENSE).
