// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test} from "forge-std/Test.sol";
import {Vault} from "src/Vault.sol";
import {IERC20} from "src/interfaces/IERC20.sol";

contract KeeperDrainTest is Test {
    Vault vault;
    IERC20 token;
    address admin = makeAddr("admin");
    address attacker = makeAddr("attacker");

    function setUp() public {
        vault = new Vault(admin, address(token));
        vm.prank(admin);
        vault.grantRole(vault.KEEPER_ROLE(), attacker);
    }

    function test_keeperDrainsVault() public {
        uint256 before = token.balanceOf(attacker);
        vm.prank(attacker);
        vault.harvest(attacker);
        assertGt(token.balanceOf(attacker), before);
    }
}
