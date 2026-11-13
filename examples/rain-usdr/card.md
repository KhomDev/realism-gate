# Evidence card: realism-gate 1.0.0

What this PoC shows, claim by claim, with the hashes a reader needs to check that
nothing changed. **Signed by nothing.** Every field is a hash of a file, or a value
the rerun command reproduces.

| Field | Value |
|---|---|
| tool | `realism-gate 1.0.0` |
| rule set | RG01-RG20, sha256 `ae1794ca39ec7159` |
| PoC | sha256 `646772314298b370` (`test/ShadowVault.t.sol`) |
| report | sha256 `0b5fba2ac3b7fa82` (`finding.md`) |
| claims | sha256 `8b5bc392845c08e8` (claims.yaml) |
| claim binding | structured |
| target commit | `5172ee079348b6f56469b148c3808857e1d4d892` |
| outcome | COMPLETE  every claim the PoC can show is shown; the fix was not run |

## Claim table

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

## Reproduce

```bash
realism-gate check test --report finding.md --target .target-repo --target-commit 5172ee079348b6f56469b148c3808857e1d4d892
```

A reader who runs that command on this tree gets this table. The JSON twin carries
the full digests, and docs/evidence-card.md gives the recipe for each one
(`poc_sha256` hashes every checked file's path and source, not one file).
Nothing here is signed: there is no key, no timestamp and no authority behind
it, only a command.
