// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test} from "forge-std/Test.sol";
import {Vault} from "src/Vault.sol";
import {IERC20} from "src/interfaces/IERC20.sol";
import {MockOracle} from "test/mocks/MockOracle.sol";

contract OracleSwapTest is Test {
    Vault vault;
    IERC20 token;
    MockOracle oracle;
    address admin = makeAddr("admin");
    address attacker = makeAddr("attacker");

    function setUp() public {
        oracle = new MockOracle();
        vault = new Vault(admin, address(token));
    }

    function test_borrowAgainstInflatedPrice() public {
        vm.prank(admin);
        vault.setOracle(address(oracle));

        uint256 before = token.balanceOf(attacker);
        vm.prank(attacker);
        vault.borrow(1_000e18);
        assertGt(token.balanceOf(attacker), before);
    }
}
