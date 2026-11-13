# RAIN-USDR: the acceptance fixture

This is the RAINUSDR-1622 case study reduced to a Foundry suite, and the
fixture `realism-gate check --report` is tested against. It is the one run the
1.0 release promises: **every claim the PoC can show is `SHOWN`, and the fix
row is `NOT SHOWN` because `check` never applies a patch.**

Source: `content/20-case-study-rain-usdr.md` in the launch workspace. The
contracts under `target/` are a minimal stand-in tree, not the audited
protocol: they exist so the cited names can be checked at a pinned commit and
so the four behaviours are readable.

## Files

| Path | What it is |
|---|---|
| `finding.md` | the report: summary, root cause, impact, PoC, fix |
| `claims.yaml` | the same claims as data, schema `realism-gate.claims/v1` |
| `test/ShadowVault.t.sol` | the PoC: four tests, no fork, no RPC, no state injection |
| `target/` | the minimal cited tree: `VaultEngine`, `BalanceSheet`, `PSM`, a stablecoin mock |
| `make-target-repo.sh` | builds `target/` into a throwaway git repo at `.target-repo/` |
| `card.md`, `card.json` | the golden evidence card this run writes |

## The four tests

1. `test_healthyDistribution_shipsSurplus` — a healthy distribution ships the
   100e18 surplus.
2. `test_negativeControl_distributesWithoutShadowVault` — the same setup with
   no shadow vault: distribution runs, then has nothing to ship. This is the
   negative control the revert needs.
3. `test_shadowDrawBlocksDistribution` — one wei of shadow debt makes
   `distributeSurplus` revert `ReserveBackingShortfall`; `assertEq(drawn,
   locked)` shows the position is capital-neutral.
4. `test_psmDepositBlocked` — drawn to the ceiling, a PSM deposit reverts
   `CeilingExceeded`; `assertEq(drawn, locked)` again.

## Run it

```bash
sh examples/rain-usdr/make-target-repo.sh
# TARGET examples/rain-usdr/.target-repo
# COMMIT 5172ee079348b6f56469b148c3808857e1d4d892
cd examples/rain-usdr
realism-gate check test --report finding.md --claims claims.yaml \
  --target .target-repo --target-commit 5172ee079348b6f56469b148c3808857e1d4d892
```

The repo is built with a fixed identity and fixed dates, so the commit is the
same object on every run. `.target-repo/` is gitignored.

## Expected output

Exit status `0`. After the rule findings, the claim table:

```text
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
```

Fields, in table order: `attacker`, `impact`, `asset`, `amount`,
`preconditions`, `cited`, `fix` — one row per claim field, six `SHOWN` and one
`NOT SHOWN`. The JSON reports the same as `claims.outcome: "shown"`,
`claims.shown: 6`, `claims.not_shown: 1`, and `claim_binding: "structured"`.

Four RG03 **notes** also print, one per test: the exploit's own reads and
distribution calls run as the test contract, which deployed the protocol. A
note is context, not a stop, and the claim table says `RG01-RG03 clean`
because no rule at `error` fired.

Without `--target`/`--target-commit` the `cited` row is `NOT CHECKABLE`: there
is no tree to check the names against. The run still exits `0`.

## The claims

`claims.yaml` states, field by field, what the report claims; `docs/claims.md`
has the predicate each field is bound to. The one row that is `NOT SHOWN` is
the fix: `check` reads the PoC, it never applies a patch, so the fix is
`verify-fix`'s question and the report has to quote that run.

## Tests

[tests/claims.rs](../../tests/claims.rs) asserts the table above: statuses,
field order, counts, `claim_binding`, exit status, the card's hashes and
reproduce command, `init-claims` round-trip, `CONTRADICTED` exit non-zero, and
that an inferred binding never passes. [tests/docs.rs](../../tests/docs.rs)
regenerates the README block and the golden card from this fixture, so neither
can drift.
