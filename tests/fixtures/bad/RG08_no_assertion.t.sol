// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test, console} from "forge-std/Test.sol";
import {IPool} from "src/interfaces/IPool.sol";
import {IERC20} from "src/interfaces/IERC20.sol";

contract ProfitLogTest is Test {
    IPool pool = IPool(0x00000000000000000000000000000000000000A1);
    IERC20 token = IERC20(0x00000000000000000000000000000000000000C3);

    function setUp() public {
        vm.createSelectFork("mainnet", 19_000_000);
    }

    function test_silent() public {
        pool.swap(address(this), true, 1e18);
    }

    function test_logsOnly() public {
        pool.swap(address(this), true, 1e18);
        emit log_named_decimal_uint("profit", token.balanceOf(address(this)), 18);
        console.log("done");
    }
}
