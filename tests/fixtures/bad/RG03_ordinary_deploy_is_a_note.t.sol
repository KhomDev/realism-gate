// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test} from "forge-std/Test.sol";
import {Vault} from "src/Vault.sol";
import {IERC20} from "src/interfaces/IERC20.sol";

/// The test contract deploys the vault and calls an ordinary function as
/// itself. That is the local-deployment pattern, a note, not a warning:
/// `deposit` is not named like an access-controlled function.
contract OrdinaryDeployTest is Test {
    Vault vault;
    IERC20 token;
    address attacker = makeAddr("attacker");

    function setUp() public {
        vault = new Vault();
    }

    function test_depositAsDeployer() public {
        uint256 before = token.balanceOf(attacker);
        vault.deposit(1 ether, attacker);
        assertEq(token.balanceOf(attacker), before);
    }
}
