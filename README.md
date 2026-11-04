# realism-gate

[![ci](https://github.com/KhomDev/realism-gate/actions/workflows/ci.yml/badge.svg)](https://github.com/KhomDev/realism-gate/actions/workflows/ci.yml)
[![license: MIT](https://img.shields.io/badge/license-MIT-informational.svg)](LICENSE)

**A static checker for Foundry proof-of-concept tests. It asks one question:
does this PoC show what an unprivileged attacker can do, or only what the
harness made true?**

A PoC that passes `forge test` can still prove nothing. The exploit step runs
under `vm.prank(admin)`. The "donation" is a `deal()` that overwrites the
vault's balance. The broken state is written with `vm.store`. The profit check
is `assertGt(balance, 0)` on an address that already held tokens. Each of these
passes locally and fails triage, and each is visible in the test's source.

realism-gate reads the source, rebuilds the order Foundry runs it in, works out
who sends every call, and reports those patterns with the line, the reason and
the fix.

- **Reads, never runs.** No compilation, no `forge`, no RPC. Your PoC never
  leaves the machine; the binary has no network code (CI checks the dependency
  graph and the source).
- **A real parser.** It uses [solar](https://github.com/paradigmxyz/solar), the
  Solidity parser built for the Foundry toolchain, not regular expressions. It
  follows inheritance, `super.setUp()`, modifiers, internal helpers and
  flash-loan callbacks.
- **Every finding can be argued with.** Each rule documents why it exists and
  how it can be wrong. A finding is silenced with a comment that must give a
  reason.

```bash
realism-gate check test/
```

## What it looks like

This is real output. `tests/docs.rs` regenerates it from
[`examples/vault-drain`](examples/vault-drain/test/VaultDrain.t.sol) and fails
if the README has drifted.

<!-- BEGIN:example -->

```text
realism-gate 0.1.0  ·  1 file  ·  1 suite  ·  1 test

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

## Install

From a release (Linux, macOS, Windows; checksums and build provenance
attached):

```bash
gh release download --repo KhomDev/realism-gate --pattern '*x86_64-unknown-linux-gnu*' --pattern SHA256SUMS
sha256sum -c SHA256SUMS --ignore-missing
```

From source (Rust 1.95 or newer):

```bash
cargo install --locked --git https://github.com/KhomDev/realism-gate
```

Two binaries are installed, `realism-gate` and the short `rgate`. They are the
same program.

In CI, see [docs/ci.md](docs/ci.md). The short version:

```yaml
- uses: KhomDev/realism-gate@v0.1.0
  with:
    paths: test
    upload-sarif: true
```

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
realism-gate check [PATHS]...      default: ./test if it exists, else .
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
    --match-contract REGEX
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
`foundry.toml`. It flattens control flow. A clean result means no known defect
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
