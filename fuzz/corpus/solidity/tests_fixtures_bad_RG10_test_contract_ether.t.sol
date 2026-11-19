// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test} from "forge-std/Test.sol";
import {IPool} from "src/interfaces/IPool.sol";

/// Foundry starts the test contract with a large ether balance, so an
/// absolute ether read of it after the attack proves nothing on its own. The
/// token-balance exemption for the test contract must not cover ether.
contract TestContractEtherTest is Test {
    IPool constant POOL = IPool(0x00000000000000000000000000000000000000A1);

    function setUp() public {
        vm.createSelectFork("mainnet", 19_000_000);
    }

    function test_drainsEther() public {
        POOL.swap(address(this), true, 1e18);
        assertGt(address(this).balance, 2 ether);
    }
}
