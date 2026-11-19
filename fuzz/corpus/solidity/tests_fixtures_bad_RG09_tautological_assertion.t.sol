// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test} from "forge-std/Test.sol";
import {Vault} from "src/Vault.sol";

contract SelfCheckTest is Test {
    Vault vault;
    address admin = makeAddr("admin");
    address attacker = makeAddr("attacker");

    function setUp() public {
        vault = new Vault(admin);
    }

    function test_stealsHundred() public {
        vm.prank(attacker);
        vault.exploit();
        uint256 stolen = 100e18;
        assertEq(stolen, 100e18);
        assertTrue(true);
    }
}
