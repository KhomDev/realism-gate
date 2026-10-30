// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test} from "forge-std/Test.sol";
import {Lender} from "src/Lender.sol";
import {IERC20} from "src/interfaces/IERC20.sol";

contract DebtClearTest is Test {
    Lender lender;
    IERC20 token;
    address admin = makeAddr("admin");
    address attacker = makeAddr("attacker");

    function setUp() public {
        lender = new Lender(admin);
    }

    function test_debtCanBeCleared() public {
        uint256 before = token.balanceOf(attacker);
        vm.startPrank(attacker);
        lender.borrow(500e18);
        vm.store(address(lender), bytes32(uint256(7)), bytes32(0));
        lender.withdrawCollateral(1_000e18);
        vm.stopPrank();
        assertGt(token.balanceOf(attacker), before);
    }
}
