// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test} from "forge-std/Test.sol";
import {IERC20} from "src/interfaces/IERC20.sol";
import {IPool} from "src/interfaces/IPool.sol";

/// The profit assertion subtracts the fee the test computed.
contract ProfitNetsFeeTest is Test {
    IERC20 token;
    IPool pool;
    address attacker = makeAddr("attacker");

    function test_profitIsNet() public {
        uint256 amount = 1_000e18;
        uint256 fee = amount * 30 / 10_000;
        uint256 before = token.balanceOf(attacker);
        vm.prank(attacker);
        pool.swap(attacker, true, amount);
        uint256 profit = token.balanceOf(attacker) - before - fee;
        assertGt(profit, 0);
    }
}
