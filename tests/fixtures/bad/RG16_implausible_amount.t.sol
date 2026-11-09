// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test} from "forge-std/Test.sol";
import {IERC20} from "src/interfaces/IERC20.sol";

/// `deal` of 10^30 raw units, and a pinned fork overwritten with 10^27.
contract ImplausibleAmountTest is Test {
    IERC20 token;
    address attacker = makeAddr("attacker");

    function test_absurdDeal() public {
        deal(address(token), attacker, 1_000_000_000_000e18);
        assertEq(token.balanceOf(attacker), 1_000_000_000_000e18);
    }

    function test_forkOverwrite() public {
        vm.createSelectFork("mainnet", 19_000_000);
        deal(address(token), attacker, 1_000_000_000e18);
        assertEq(token.balanceOf(attacker), 1_000_000_000e18);
    }

    function test_maxMint() public {
        token.mint(attacker, type(uint256).max);
        assertEq(token.balanceOf(attacker), type(uint256).max);
    }
}
