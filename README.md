# realism-gate

**A writing and PoC-structure filter for Foundry, Solana and Move tests, plus an optional two-sided forge check. 1.0 also asks the other half of the question: does the proof prove what the report claims? It is not a validity oracle.**

For bounty hunters, contest auditors, and people hunting with an AI agent. A test can pass because the harness granted a role, wrote storage with `stdstore` or `vm.store`, moved that write into `setUp()`, or asserted `balance + 1 > before`. realism-gate looks for those shapes. With `--report` it also reads the finding's claims and binds each one to the evidence the PoC does or does not contain. A clean result means none of the patterns fired and no claim contradicted the test. It does not mean the bug is real, the citation exists, or a program pays for it.

```bash
cargo binstall realism-gate
```

From this commit, before a release exists: `cargo install --git https://github.com/KhomDev/realism-gate --locked`.

## 1.0: the claim table

One tool, two questions: **is the PoC built on something only the harness
could do** (the RG01-RG20 rules), and **does the report claim only what the PoC
shows** (the claim table). 0.x answered the first; 1.0 answers both.

A passing test and a correct report are two claims, and 0.x only checked the
first. 1.0 reads the finding too: `realism-gate check test --report finding.md`
prints the rule findings, then a claim table with one row per claim the report
makes. Each row is bound to evidence in the PoC and gets one of four statuses:
`SHOWN`, `NOT SHOWN`, `CONTRADICTED`, `NOT CHECKABLE`. There is no score.

<!-- BEGIN:example-claims -->

```text
realism-gate 1.0.0  ·  1 file  ·  1 suite  ·  4 tests

test/ShadowVault.t.sol
  note   RG03 deployer-is-attacker  test/ShadowVault.t.sol:57:26  ShadowVaultTest::test_healthyDistribution_shipsSurplus
         3 call(s) in the exploit run as the test contract, which deployed `sheet`
         (BalanceSheet). None of them is named like an access-controlled function, so this is
         the ordinary local-deployment pattern, not evidence the attacker used a privilege. It
         is a note. Attack from a separate address if one of those calls is in fact gated.
         ↳ sheet.reserve()
         → Deploy and configure as one address, attack as another: `vm.startPrank(attacker)`
         before the exploit steps, or transfer ownership to a dedicated owner in `setUp()`.

  note   RG03 deployer-is-attacker  test/ShadowVault.t.sol:68:25  ShadowVaultTest::test_negativeControl_distributesWithoutShadowVault
         3 call(s) in the exploit run as the test contract, which deployed `sheet`
         (BalanceSheet). None of them is named like an access-controlled function, so this is
         the ordinary local-deployment pattern, not evidence the attacker used a privilege. It
         is a note. Attack from a separate address if one of those calls is in fact gated.
         ↳ sheet.distributeSurplus()
         → Deploy and configure as one address, attack as another: `vm.startPrank(attacker)`
         before the exploit steps, or transfer ownership to a dedicated owner in `setUp()`.

  note   RG03 deployer-is-attacker  test/ShadowVault.t.sol:81:9  ShadowVaultTest::test_shadowDrawBlocksDistribution
         2 call(s) in the exploit run as the test contract, which deployed `sheet`
         (BalanceSheet), `vault` (VaultEngine). None of them is named like an access-controlled
         function, so this is the ordinary local-deployment pattern, not evidence the attacker
         used a privilege. It is a note. Attack from a separate address if one of those calls is
         in fact gated.
         ↳ sheet.distributeSurplus()
         → Deploy and configure as one address, attack as another: `vm.startPrank(attacker)`
         before the exploit steps, or transfer ownership to a dedicated owner in `setUp()`.

  note   RG03 deployer-is-attacker  test/ShadowVault.t.sol:111:26  ShadowVaultTest::test_psmDepositBlocked
         1 call(s) in the exploit run as the test contract, which deployed `vault`
         (VaultEngine). None of them is named like an access-controlled function, so this is the
         ordinary local-deployment pattern, not evidence the attacker used a privilege. It is a
         note. Attack from a separate address if one of those calls is in fact gated.
         ↳ vault.locked(alice, USDT_A)
         → Deploy and configure as one address, attack as another: `vm.startPrank(attacker)`
         before the exploit steps, or transfer ownership to a dedicated owner in `setUp()`.

CLAIMS  claims.yaml  ·  structured
CLAIM                           EVIDENCE                                      STATUS
any user, no role               `alice` (makeAddr("alice")) sends the         SHOWN
                                exploit in test_psmDepositBlocked; no role,
                                no prank as protocol (RG01-RG03 clean)
other:distributeSurplus         vm.expectRevert(BalanceSheet.ReserveBackingShortfall.selector)  SHOWN
reverts with                    in test_shadowDrawBlocksDistribution;
ReserveBackingShortfall         vm.expectRevert(PegStabilityModule.CeilingExceeded.selector)
(revenue halted) and            in test_psmDepositBlocked; negative control
PSM.sellStable reverts with     test_healthyDistribution_shipsSurplus acts
CeilingExceeded (on-ramp        without the setup and does not revert
denial)
USDR                            `assertEq(drawn, locked)` in                  SHOWN
                                test_shadowDrawBlocksDistribution compares
                                two quantities of `USDR`
the drawn USDR equals the       `assertEq(drawn, locked)` in                  SHOWN
locked stablecoins, so the      test_shadowDrawBlocksDistribution compares
position is capital-neutral     the two quantities the claim is about
no privileged role; no oracle   `no privileged role`: RG01-RG03 clean; `no    SHOWN
or price movement; no keeper    oracle or price movement`: no fork, RPC or
and no off-chain trigger        chain read in the PoC; `no keeper and no
                                off-chain trigger`: every exploit step is
                                the same sender's own call
src/VaultEngine.sol:open,       all 4 cited names are in the target tree      SHOWN
src/VaultEngine.sol:frob,       (RG20 clean)
src/BalanceSheet.sol:distributeSurplus,
src/PSM.sol:sellStable
restrict vaults on fee-exempt   not run: `check` never applies a patch. Run   NOT SHOWN
stable collateral to the PSM    `realism-gate verify-fix` on the patch and
                                quote its verdict
CLAIMS  COMPLETE  every claim the PoC can show is shown; the fix was not run

SUMMARY  0 errors  ·  0 warnings  ·  4 notes  ·  0 suppressed
RESULT   PASS  (fails on error and above)

realism-gate reads the test; it does not run it. A clean result means no known
defect pattern was found, not that the PoC is valid. `realism-gate explain <RULE>`
shows why a rule exists and how it can be wrong.
```

<!-- END:example-claims -->

That block is generated by `tests/docs.rs` from
[`examples/rain-usdr`](examples/rain-usdr), the RAIN-USDR case study reduced
to a runnable fixture; the cited contracts are made into a pinned git tree by
[`examples/rain-usdr/make-target-repo.sh`](examples/rain-usdr/make-target-repo.sh).

The claims come from a structured block when the report has one: a
`claims.yaml` next to the finding, `--claims FILE`, or a YAML front-matter
block. Without one, the tool extracts only what parses reliably, labels the
binding **inferred**, and never lets an inferred run pass. The format, the
predicate behind each field, and the exit codes are in
[docs/claims.md](docs/claims.md).

### The evidence card

```bash
realism-gate check test --report finding.md --card card.md --card-json card.json
```

The card is the submission-sized version: tool version, rule-set hash, PoC and
report sha256, target commit, the claim table, and the exact rerun command. It
is **signed by nothing; reproducible by command** — every field is a hash of a
file it names or a value the command prints again. See
[docs/evidence-card.md](docs/evidence-card.md) for the field list and a golden
card.

### What 1.0 is not

- **Not a validity oracle.** A `SHOWN` claim row means the evidence the
  predicate looks for is in the PoC. It does not mean the bug is real, that the
  severity is right, or that a program pays for it.
- **An inferred claim is not a pass.** If no structured claims block exists,
  the whole run exits non-zero however good the rows look.
- **No score.** Four statuses and a table, not a number.
- **Not an execution tool.** `check` never runs the PoC; the `fix` row stays
  open until you run `verify-fix` yourself.
- **Not a duplicate or payout check.** That is a different question.

### Migrating

| If you used | Now use | What changed |
|---|---|---|
| `vg` claim notes P23--P26, P19 | `realism-gate check --report` | Claims bound to PoC evidence, one row per field |
| `vg` citation anchor | `realism-gate check --target --target-commit` (RG20) | One implementation |
| realism-gate 0.x | realism-gate 1.0 | Same rule ids and severities; adds `--report`, `init-claims` and the evidence card |

The full four-tools-to-two migration table is in the release announcement; the
per-release detail is in [CHANGELOG.md](CHANGELOG.md#100---2026-11-13).

## Before and after

The test deploys a vault and then drains it as admin, with `deal` writing the vault's balance and no before-value:

<!-- BEGIN:example -->

```text
realism-gate 1.0.0  ·  1 file  ·  1 suite  ·  1 test

test/VaultDrain.t.sol
  warn   RG11 unpinned-fork  test/VaultDrain.t.sol:17:9  VaultDrainTest::test_attackerDrainsVault
         The fork has no block number, so it runs against the chain as it is today. Balances,
         prices and the bug itself can change before a triager runs it.
         ↳ vm.createSelectFork(vm.envString("RPC_URL"))
         → Pass the block: `vm.createSelectFork(url, BLOCK)`, and say in the report why that
         block is representative.

  warn   RG02 privileged-step-in-exploit  test/VaultDrain.t.sol:24:9  VaultDrainTest::test_attackerDrainsVault
         The exploit runs a step as `admin`: the name `admin` reads as the `admin` role. An
         attacker cannot sign as this address. Moving the prank into setUp() does not make it
         the attacker's action; a prank there next to a direct storage write is RG19. Declare
         the role in an operator realism.yaml outside the repo under review if the claim relies
         on that role acting.
         ↳ vm.prank(admin)
         → Do not hide the prank in `setUp()`.

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

- A bounty hunter about to submit a PoC: run the rules, then bind the report's claims to what the test actually shows.
- A contest auditor whose test deploys the protocol locally.
- A triager who wants the claim table and the [evidence card](docs/evidence-card.md) next to the report.
- An agent that drafts findings. The [skill](skills/realism-gate/SKILL.md) and [`realism-gate-mcp`](docs/agents.md) run the PoC check and the claim check before the report is written.

## What it does

- Reads Foundry (Solidity, via solar), Solana tests (Rust via syn, Anchor TypeScript via oxc) and Move unit tests (Sui `test_scenario`, Aptos `#[test]`). `check` does not run them.
- With `--report`, reads the finding's claims and binds them to the PoC: attacker, impact, asset, amount, preconditions, citations and fix. Structured claims are exact; heuristic claims are labelled inferred and never count as a pass. There is no score. See [docs/claims.md](docs/claims.md).
- Writes an [evidence card](docs/evidence-card.md) (`--card`, `--card-json`): the claim table plus the tool version, rule-set hash, PoC and report sha256, target commit and rerun command. Signed by nothing; reproducible by command.
- Rebuilds the order the test runs in, including `setUp`, helpers and callbacks, and who sends each call.
- Twenty rules. RG01-RG14 are the privilege, injection and evidence rules. RG15-RG18 are the economic ones. RG19 flags an owner prank in `setUp()` next to a direct storage write. RG20, with `--target` and `--target-commit`, checks that cited contracts, functions and import paths exist at that commit.
- RG03 warns only when the deployer calls something named like an access-controlled function. An ordinary local `deposit` is a note.
- JSON, SARIF and Markdown. An operator `realism.yaml` outside the repo under review states the attacker model and `deployment.evidence`. A file inside that repo is not applied. Inline `allow` comments are ignored unless `--trust-inline-allows`.
- `verify-fix` is the opt-in that runs forge twice: the PoC must pass before the patch and fail after it. See [docs/verify-fix.md](docs/verify-fix.md).
- The GitHub Action (`@v1`) can check a report too: the `report` and `claims` inputs print the claim table in the job log and enable the evidence card output, and the JSON report carries both. See [docs/ci.md](docs/ci.md).

## Solana

Same rule ids, same output. Rust harnesses (solana-program-test, LiteSVM, Mollusk, anchor-client, anchor-litesvm) and Anchor's mocha or jest tests.

<!-- BEGIN:example-rust -->

```text
realism-gate 1.0.0  ·  1 file  ·  1 suite  ·  1 test

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
         `admin` role. An attacker cannot produce that signature. Declare the role in an
         operator realism.yaml outside the repo under review if the claim relies on that role
         acting.
         ↳ send(&mut svm, set_withdraw_fee_ix(&admin.pubkey(), 0), &[&admin])
         → Do not treat a privileged signature as configuration by moving it earlier.

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
realism-gate 1.0.0  ·  1 file  ·  1 suite  ·  1 test

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
         `admin` role. An attacker cannot produce that signature. Declare the role in an
         operator realism.yaml outside the repo under review if the claim relies on that role
         acting.
         ↳ program.methods.setWithdrawFee(new BN(0)).accounts({ admin: admin.publicKey, vaultState }).signers([admin]).rpc()
         → Do not treat a privileged signature as configuration by moving it earlier.

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
| [RG19](docs/rules/RG19.md) `setup-hides-storage-write` | error | `setUp()` pranks as a role and writes storage |
| [RG20](docs/rules/RG20.md) `missing-citation` | error | a cited path, contract or function is missing at `--target-commit` |

`realism-gate explain RG15` prints why a rule exists and how it can be wrong. Each page under `docs/rules/` is generated from the catalog.

## Usage

```text
realism-gate check [PATHS]...
    --format text|json|sarif|markdown      --output FILE
    --fail-on error|warn|note|never
    --report FILE               the finding; prints the claim table after the findings
    --claims FILE               a claims.yaml (default: claims.yaml next to --report)
    --card FILE --card-json FILE  write the evidence card
    --no-fail-on-claims         do not exit 1 for a contradicted claim or an inferred binding
    --declaration FILE          operator file outside the repo under review
    --no-declaration            ignore ./realism.yaml
    --config FILE               same, for realism-gate.toml
    --no-config                 ignore ./realism-gate.toml
    --target DIR --target-commit REV
    --rules RG15,RG17           --skip RG08
    --match-test REGEX          --match-contract REGEX
    --severity RULE=LEVEL       --exclude GLOB
    --show-suppressed           --trust-inline-allows
    --changed-since REF
    --color auto|always|never
realism-gate init-claims finding.md [--out FILE] [--force]
realism-gate verify-fix --sandbox --poc test/Poc.t.sol --patch fix.diff
realism-gate verify-fix --sandbox --poc test/Poc.t.sol --patched poc-patched/
realism-gate rules
realism-gate explain RULE
realism-gate init
```

Exit status for `check`: `0` nothing at or above `--fail-on`, no claim is `CONTRADICTED`, and the binding is structured; `1` a finding is at or above `--fail-on`, **or** a claim is `CONTRADICTED`, **or** the claims were inferred; `2` the inputs could not be read. `--fail-on` does not suppress the claim failures, and `--no-fail-on-claims` is the documented escape hatch. `init-claims` exits `0` on a write and `2` on an error. `verify-fix` exits `0` when the PoC passed before the patch and failed after it, `1` when the bug did not reproduce or the PoC still passes, `2` when forge did not run the test or the project is unsafe to run. The full table is in [docs/claims.md](docs/claims.md#exit-codes).

## Limits

It reads the test. It does not run it, except `verify-fix`, which runs forge twice under the limits in [docs/verify-fix.md](docs/verify-fix.md). It does not know a function is gated when the name does not say so. It does not know the balance at a pinned block. It does not know that a README forbids the call; RG17 is silent only when an operator `deployment.evidence` outside the repo cites a source. RG20 checks cited names only when you pass a target and a commit. It flattens control flow. A Move file is not type-checked. `check` has no network code.

A claim table is narrower than the bug. Binding cannot confirm a role the test does not grant, an asset the PoC never names, an amount stated only in prose, a precondition it has no words for, or a citation without a target commit. The `fix` row is always open until you run `verify-fix`. And the inferred fallback reads only backticked identifiers, the headline's impact verb and the words "any user"/"unprivileged"/role names; everything else is `NOT CHECKABLE`, and an inferred run is never a pass.

A clean result is not a payable finding. The full list is [docs/limitations.md](docs/limitations.md#claims-and-binding). How often a flag was right, on the corpora that were labelled, is [docs/precision.md](docs/precision.md); the claim-binding numbers (impact: 91.0% on the first build, n=100, Wilson 95% [83.8-95.2]; 98.9% after fixing the misses that the same set exposed, so in-sample) and their caveats are [there](docs/precision.md#claim-binding).

## Related

| Tool | Question |
|---|---|
| [priorart](https://github.com/KhomDev/priorart) (formerly viability-gate) | Is it new, in scope, and payable? |
| **realism-gate** | **Does the proof prove the claim?** |

## Contributing

A false positive or a false negative, reduced to a synthetic test. See [CONTRIBUTING.md](CONTRIBUTING.md). Do not attach a PoC for a live, unfixed finding.

Releases follow [RELEASING.md](RELEASING.md): patch releases for fixes, at most one minor per week, and a mandatory dry run of the release workflow before a tag.

## License

MIT. See [LICENSE](LICENSE).
