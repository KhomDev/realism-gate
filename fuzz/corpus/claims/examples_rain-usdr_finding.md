# Self-backed vaults switch off a stablecoin's surplus

**Report RAINUSDR-1622.** A vault opened on a fee-exempt stable collateral type draws USDR 1:1
against stablecoins the PSM was the only intended route for. Its debt has no reserve counterpart,
so `BalanceSheet.distributeSurplus` reverts for everyone, and the same draws consume the debt
ceiling PSM deposits depend on.

## Summary

`VaultEngine.open` let anyone open a vault on the fee-exempt stablecoin collateral types
(USDT-A, USDC-A) that were meant to be used only through the Peg Stability Module. A fully
collateralised, 1:1 draw from such a vault made `BalanceSheet.distributeSurplus` revert with
`ReserveBackingShortfall` on every call, and the same draws used up the collateral type's debt
ceiling that PSM deposits depend on.

## Root cause

`VaultEngine.open` accepted any initialised collateral type; its only checks were that the system
was live, the owner non-zero and the rate set. `frob` applied its solvency gate only to volatile
collateral, so a vault on a stable type sat outside both that gate and the PSM's own checks. Stable
collateral is priced at par, so any user could join their own stablecoins and draw USDR 1:1.

`distributeSurplus` summed the debt of every fee-exempt collateral type and reverted when the
reserve was smaller. Its premise, written in its own comment, is that all such debt is the
bookkeeping counterpart of stablecoins the PSM put in the reserve. A self-backed vault's debt has
no reserve counterpart, and the check could not tell the two apart. The check also ran before the
"nothing to distribute" early return, so it reverted even when no surplus would ship.

## Impact

- **Revenue halted.** One wei of shadow debt made every `distributeSurplus` call revert, stopping
  the buyback-and-burn channel until the vault owner repays or governance reconfigures the system.
  Fee exemption is one-way, so the collateral type cannot be un-exempted.
- **On-ramp denial.** Draws up to the debt ceiling made `PSM.sellStable` revert
  `CeilingExceeded` for every later depositor.
- **Zero cost to the attacker.** The position is capital-neutral: drawn USDR equals locked
  stablecoins, so there is no reason to ever repay.

## Proof of concept

A Foundry suite against the program's contracts, using the repository's own mocks and no RPC. Four
tests:

- a healthy distribution ships surplus, and distribution repeats with the same setup and no shadow
  vault (the negative control);
- after a one wei shadow draw, `distributeSurplus` reverts `ReserveBackingShortfall`;
- with the ceiling at current usage, a PSM deposit reverts `CeilingExceeded` while the attacker
  holds the drawn USDR;
- the drawn USDR equals the locked stablecoins exactly, so the attack is capital-neutral.

The attack path needs no oracle, price movement, keeper or privileged role: `open`, `join`,
`frob`.

## Fix

Restrict vaults on fee-exempt stable collateral to the PSM, in `VaultEngine.open` or `frob`,
mirroring the rate guards already in place; or make the backing check sum only PSM-owned debt.
Either way the check then means what its comment says.
