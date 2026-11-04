// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test} from "forge-std/Test.sol";
import {IERC20} from "src/interfaces/IERC20.sol";
import {IPool} from "src/interfaces/IPool.sol";

/// A shared base that zeroes the attacker's ether before each test, the way
/// PoC collections log a balance before and after the exploit.
abstract contract BalanceLogBase is Test {
    address attacker;

    function _attacker() private view returns (address) {
        return attacker == address(0) ? address(this) : attacker;
    }

    modifier balanceLog() {
        vm.deal(_attacker(), 0);
        _;
    }
}

contract Drainer {
    function drain(IPool pool) external {
        pool.skim(address(this));
    }
}

/// Absolute reads after the attack that are differences all the same: the
/// starting balance is one the test fixed itself.
contract FundedBaselineTest is BalanceLogBase {
    IERC20 constant USDC = IERC20(0x00000000000000000000000000000000000000C3);
    IERC20 constant DAI = IERC20(0x00000000000000000000000000000000000000D5);
    IPool constant POOL = IPool(0x00000000000000000000000000000000000000A1);
    address constant EXPLOITER = 0x00000000000000000000000000000000000000E1;

    function setUp() public {
        vm.createSelectFork("mainnet", 19_000_000);
        attacker = EXPLOITER;
    }

    /// The modifier zeroed EXPLOITER's ether through `_attacker()`.
    function test_etherFromZero() public balanceLog {
        vm.prank(EXPLOITER);
        POOL.swap(EXPLOITER, true, 1e18);
        assertGt(EXPLOITER.balance, 1 ether);
    }

    /// deal() fixed the USDC balance at 4 units; the claim is far above it.
    function test_tokenFromDealtAmount() public {
        deal(address(USDC), address(this), 4);
        POOL.swap(address(this), true, 1e18);
        assertGt(USDC.balanceOf(address(this)), 1_000_000e6);
    }

    /// A contract the test deploys holds no DAI, even after it is dealt USDC.
    function test_freshContractOtherToken() public {
        Drainer d = new Drainer();
        deal(address(USDC), address(d), 100e6);
        d.drain(POOL);
        assertGt(DAI.balanceOf(address(d)), 1e18);
    }

    /// The test contract holds no tokens before the attack.
    function test_testContractTokens() public {
        POOL.swap(address(this), false, 1e18);
        assertGe(DAI.balanceOf(address(this)), 5e18);
    }
}
