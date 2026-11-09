# realism-gate

**Does your PoC show what an unprivileged attacker can actually do, or only what your harness made true?**

For bounty hunters, contest auditors, and people hunting with an AI agent. A test can pass because the harness granted a role, dealt a balance, shipped a strategy nobody runs, or asserted a profit a fee in the same file wipes out.

```bash
cargo binstall realism-gate
```

From this commit, before a release exists: `cargo install --git https://github.com/KhomDev/realism-gate --locked`.

## Before and after

The test deploys a vault and then drains it as admin, with `deal` writing the vault's balance and no before-value:

<!-- BEGIN:example -->

```text
realism-gate 0.5.0  ·  1 file  ·  1 suite  ·  1 test

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

`tests/docs.rs` regenerates that block from [`examples/vault-drain`](examples/vault-drain/test/VaultDrain.t.sol). A clean result means none of the patterns fired. It does not mean the bug is real or that a program pays for it.

## Who it is for

- A bounty hunter about to submit a PoC.
- A contest auditor whose test deploys the protocol locally.
- An agent that drafts findings. The [skill](skills/realism-gate/SKILL.md) and [`realism-gate-mcp`](docs/agents.md) run this check, and `vg check` from [viability-gate](https://github.com/KhomDev/viability-gate), before the report is written.

## What it does

- Reads Foundry (Solidity, via solar), Solana tests (Rust via syn, Anchor TypeScript via oxc) and Move unit tests (Sui `test_scenario`, Aptos `#[test]`). It does not run them.
- Rebuilds the order the test runs in, including `setUp`, helpers and callbacks, and who sends each call.
- Eighteen rules. RG01-RG14 are the privilege, injection and evidence rules. RG15-RG18 are the economic ones: profit net of costs, implausible amounts, setup with no deployment evidence, revert-only tests.
- RG03 warns only when the deployer calls something named like an access-controlled function. An ordinary local `deposit` is a note.
- JSON, SARIF and Markdown. `realism.yaml` for the attacker model and for `deployment.evidence`.
- `verify-fix` is the opt-in that runs forge: the PoC must fail on a patch or a `poc-patched/` tree.
- A combined GitHub Action runs this and `vg check` and publishes one SARIF file and one PR comment. See [`examples/workflows/gates.yml`](examples/workflows/gates.yml).

## Solana

Same rule ids, same output. Rust harnesses (solana-program-test, LiteSVM, Mollusk, anchor-client, anchor-litesvm) and Anchor's mocha or jest tests.

<!-- BEGIN:example-rust -->

```text
realism-gate 0.5.0  ·  1 file  ·  1 suite  ·  1 test

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

<!-- BEGIN:example-anchor -->

```text
realism-gate 0.5.0  ·  1 file  ·  1 suite  ·  1 test

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

## The rules

| Rule | Severity | Fires when |
|---|---|---|
| [RG01](docs/rules/RG01.md) `attacker-holds-privilege` | error | the harness grants the attacker a role |
| [RG02](docs/rules/RG02.md) `privileged-step-in-exploit` | warn | the exploit pranks as a role or a protocol contract |
| [RG03](docs/rules/RG03.md) `deployer-is-attacker` | warn, or note | the deployer calls the protocol; warn only if the call looks access-controlled |
| [RG04](docs/rules/RG04.md) `storage-write` | error | `vm.store` writes a contract under review |
| [RG05](docs/rules/RG05.md) `code-replacement` | warn | `vm.etch` replaces a dependency |
| [RG06](docs/rules/RG06.md) `balance-injection` | error | `deal()` writes a protocol contract's balance |
| [RG07](docs/rules/RG07.md) `mocked-call` | warn | `vm.mockCall` invents a return value |
| [RG08](docs/rules/RG08.md) `no-assertion` | error | the test asserts nothing |
| [RG09](docs/rules/RG09.md) `tautological-assertion` | error | an assertion cannot fail |
| [RG10](docs/rules/RG10.md) `no-baseline` | warn | a gain is asserted with no before-value |
| [RG11](docs/rules/RG11.md) `unpinned-fork` | warn | a fork has no block number |
| [RG12](docs/rules/RG12.md)–[RG14](docs/rules/RG14.md) | error / warn / note | `realism.yaml` boundaries and dependent preconditions |
| [RG15](docs/rules/RG15.md) `profit-ignores-costs` | warn | asserted profit ignores a cost the test measures |
| [RG16](docs/rules/RG16.md) `implausible-amount` | warn | a deal or mint is above the fixed bound |
| [RG17](docs/rules/RG17.md) `unevidenced-setup` | warn | counterparty setup has no deployment evidence |
| [RG18](docs/rules/RG18.md) `revert-only` | warn | the only assertion is a revert |

`realism-gate explain RG15` prints why a rule exists and how it can be wrong. Each page under `docs/rules/` is generated from the catalog.

## Usage

```text
realism-gate check [PATHS]...
    --format text|json|sarif|markdown
    --fail-on error|warn|note|never
    --declaration FILE          default: ./realism.yaml
    --rules RG15,RG17
    --skip RG08
realism-gate verify-fix --poc test/Poc.t.sol --patch fix.diff
realism-gate verify-fix --poc test/Poc.t.sol --patched poc-patched/
realism-gate rules
realism-gate explain RULE
realism-gate init
```

Exit status for `check`: `0` nothing at or above `--fail-on`, `1` there is, `2` the inputs could not be read. `verify-fix` exits `0` when the PoC fails on the patch, `1` when it still passes, `2` when forge did not run the test.

## Limits

It reads the test, not the protocol and not the chain. It does not know a function is gated when the name does not say so. It does not know the balance at a pinned block. It does not know that a README forbids the call; RG17 is silent only when `deployment.evidence` cites a source. It flattens control flow. A Move file is not type-checked. `check` never runs forge and has no network code; `verify-fix` is the command that does run forge, locally.

A clean result is not a payable finding. The full list is [docs/limitations.md](docs/limitations.md). How often a flag was right, on the corpora that were labelled, is [docs/precision.md](docs/precision.md).

## Related

| Repository | Question |
|---|---|
| [rejection-taxonomy](https://github.com/KhomDev/rejection-taxonomy) | Why did findings die? |
| [bounty-economics](https://github.com/KhomDev/bounty-economics) | Can this finding pay on this program? |
| [viability-gate](https://github.com/KhomDev/viability-gate) | Will the write-up survive triage? |
| **realism-gate** | **Does the PoC show what an unprivileged attacker can do?** |

## Contributing

A false positive or a false negative, reduced to a synthetic test. See [CONTRIBUTING.md](CONTRIBUTING.md). Do not attach a PoC for a live, unfixed finding.

## License

MIT. See [LICENSE](LICENSE).
