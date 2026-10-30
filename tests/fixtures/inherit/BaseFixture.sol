// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test} from "forge-std/Test.sol";
import {Vault} from "src/Vault.sol";
import {IERC20} from "src/interfaces/IERC20.sol";

abstract contract BaseFixture is Test {
    Vault vault;
    IERC20 token;
    address admin = makeAddr("admin");
    address attacker = makeAddr("attacker");

    function setUp() public virtual {
        vault = new Vault(admin, address(token));
        // Seeds the vault's balance without its accounting seeing it.
        deal(address(token), address(vault), 1_000e18);
    }

    function _measure() internal view returns (uint256) {
        return token.balanceOf(attacker);
    }
}
