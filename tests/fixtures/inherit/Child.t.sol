// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {BaseFixture} from "./BaseFixture.sol";

contract ChildTest is BaseFixture {
    function setUp() public override {
        super.setUp();
    }

    function test_first() public {
        uint256 before = _measure();
        vm.prank(attacker);
        vault.exploit();
        assertGt(_measure(), before);
    }

    function test_second() public {
        uint256 before = token.balanceOf(attacker);
        vm.prank(attacker);
        vault.exploitAgain();
        assertGt(token.balanceOf(attacker), before);
    }
}
