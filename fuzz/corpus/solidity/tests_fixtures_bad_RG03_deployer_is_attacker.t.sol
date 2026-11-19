// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test} from "forge-std/Test.sol";
import {Vault} from "src/Vault.sol";
import {IERC20} from "src/interfaces/IERC20.sol";

contract SweepTest is Test {
    Vault vault;
    IERC20 token;

    function setUp() public {
        vault = new Vault();
    }

    function test_anyoneCanSweep() public {
        uint256 before = token.balanceOf(address(this));
        vault.sweep(address(token));
        assertGt(token.balanceOf(address(this)), before);
    }
}
