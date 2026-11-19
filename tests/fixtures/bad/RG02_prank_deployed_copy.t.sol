// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test} from "forge-std/Test.sol";

// A copy of the protocol, defined in the PoC file and deployed by the test.
contract Vault {
    uint256 public balance = 100;

    function sweep(address) external {
        balance = 0;
    }
}

contract DeployedCopyTest is Test {
    Vault vault;

    function setUp() public {
        vault = new Vault();
    }

    function test_sweep_as_the_vault() public {
        uint256 before = vault.balance();
        vm.prank(address(vault));
        vault.sweep(address(this));
        assertLt(vault.balance(), before);
    }
}
