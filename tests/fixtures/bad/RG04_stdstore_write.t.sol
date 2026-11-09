// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test} from "forge-std/Test.sol";
import {Vault} from "src/Vault.sol";
import {IERC20} from "src/interfaces/IERC20.sol";

/// Storage writes that are not spelled `vm.store`. Each one used to pass.
contract StdstoreWriteTest is Test {
    Vault vault;
    IERC20 token;
    address attacker = makeAddr("attacker");
    address other = makeAddr("other");

    function setUp() public {
        vault = new Vault();
    }

    function test_stdstoreWritesShares() public {
        uint256 before = token.balanceOf(attacker);
        stdstore.target(address(vault)).sig("shares(address)").with_key(attacker).checked_write(1);
        vm.prank(attacker);
        vault.withdraw(1);
        assertEq(token.balanceOf(attacker), before);
    }

    function test_setArbitraryStorage() public {
        uint256 before = token.balanceOf(attacker);
        setArbitraryStorage(address(vault), bytes32(uint256(3)), bytes32(uint256(1)));
        vm.prank(attacker);
        vault.withdraw(1);
        assertEq(token.balanceOf(attacker), before);
    }

    function test_copyStorage() public {
        uint256 before = token.balanceOf(attacker);
        copyStorage(address(vault), address(other));
        vm.prank(attacker);
        vault.withdraw(1);
        assertEq(token.balanceOf(attacker), before);
    }
}
