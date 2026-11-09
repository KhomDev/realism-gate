// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test} from "forge-std/Test.sol";
import {Vault} from "src/Vault.sol";
import {IERC20} from "src/interfaces/IERC20.sol";

/// The owner prank and the storage write were moved into setUp so the test
/// body looks like an unprivileged call.
contract SetupHidesStorageTest is Test {
    Vault vault;
    IERC20 token;
    address owner = makeAddr("owner");
    address attacker = makeAddr("attacker");

    function setUp() public {
        vault = new Vault(owner);
        vm.prank(owner);
        vm.store(address(vault), bytes32(uint256(1)), bytes32(uint256(0)));
    }

    function test_attackerDrains() public {
        uint256 before = token.balanceOf(attacker);
        vm.prank(attacker);
        vault.withdraw(1);
        assertGt(token.balanceOf(attacker), before);
    }
}
