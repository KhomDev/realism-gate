// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test} from "forge-std/Test.sol";
import {Vault} from "src/Vault.sol";

/// The only assertion is that the call reverts. No balance is checked.
contract RevertOnlyTest is Test {
    Vault vault;
    address attacker = makeAddr("attacker");

    function setUp() public {
        vault = new Vault();
    }

    function test_withdrawReverts() public {
        vm.prank(attacker);
        vm.expectRevert();
        vault.withdraw(1);
    }
}
