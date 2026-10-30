// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test} from "forge-std/Test.sol";
import {Vault} from "src/Vault.sol";
import {IOracle} from "src/interfaces/IOracle.sol";
import {IERC20} from "src/interfaces/IERC20.sol";

contract PriceManipulationTest is Test {
    Vault vault;
    IOracle oracle;
    IERC20 token;
    address admin = makeAddr("admin");
    address attacker = makeAddr("attacker");

    function setUp() public {
        vault = new Vault(admin, address(token));
    }

    function test_borrowAtManipulatedPrice() public {
        vm.mockCall(address(oracle), abi.encodeWithSelector(IOracle.price.selector), abi.encode(1));
        uint256 before = token.balanceOf(attacker);
        vm.prank(attacker);
        vault.borrow(1_000e18);
        assertGt(token.balanceOf(attacker), before);
    }
}
