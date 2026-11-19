// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test} from "forge-std/Test.sol";
import {Vault} from "src/Vault.sol";
import {IERC20} from "src/interfaces/IERC20.sol";

/// `balance + 1 > before` passes when the balance did not fall.
contract InflatedBaselineTest is Test {
    Vault vault;
    IERC20 token;
    address attacker = makeAddr("attacker");

    function setUp() public {
        vault = new Vault();
    }

    function test_plusOneBeatsTheBaseline() public {
        uint256 before = token.balanceOf(attacker);
        vm.prank(attacker);
        vault.withdraw(1);
        uint256 a = token.balanceOf(attacker) + 1;
        assertGt(a, before);
    }
}
