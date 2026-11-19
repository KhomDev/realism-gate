// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test} from "forge-std/Test.sol";
import {Vault} from "src/Vault.sol";
import {IERC20} from "src/interfaces/IERC20.sol";

contract AbsoluteProfitTest is Test {
    Vault vault;
    IERC20 token;
    address admin = makeAddr("admin");
    address attacker = makeAddr("attacker");

    function setUp() public {
        vault = new Vault(admin, address(token));
        deal(address(token), attacker, 10e18);
    }

    function test_attackerProfits() public {
        vm.prank(attacker);
        vault.exploit();
        assertGt(token.balanceOf(attacker), 0);
    }
}
