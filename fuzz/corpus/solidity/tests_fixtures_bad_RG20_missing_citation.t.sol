// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test} from "forge-std/Test.sol";
import {MissingVault} from "src/MissingVault.sol";

/// Cites a file, a contract and a function that are not in the pinned target.
contract MissingCitationTest is Test {
    MissingVault vault;

    function test_citesNothingReal() public {
        vault = new MissingVault();
        uint256 before = vault.balanceOf(address(this));
        vault.inventedDrain();
        assertEq(vault.balanceOf(address(this)), before);
    }
}
