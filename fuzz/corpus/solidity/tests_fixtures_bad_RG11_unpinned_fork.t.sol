// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test} from "forge-std/Test.sol";
import {IPool} from "src/interfaces/IPool.sol";
import {IERC20} from "src/interfaces/IERC20.sol";

contract LiveForkTest is Test {
    IPool pool = IPool(0x00000000000000000000000000000000000000A1);
    IERC20 token = IERC20(0x00000000000000000000000000000000000000C3);
    address attacker = makeAddr("attacker");

    function setUp() public {
        vm.createSelectFork(vm.envString("RPC_URL"));
    }

    function test_swapProfit() public {
        uint256 before = token.balanceOf(attacker);
        vm.prank(attacker);
        pool.swap(attacker, true, 1e18);
        assertGt(token.balanceOf(attacker), before);
    }
}
