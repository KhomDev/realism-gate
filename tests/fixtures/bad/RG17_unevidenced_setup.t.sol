// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test} from "forge-std/Test.sol";
import {Aqua} from "src/Aqua.sol";

/// A maker ships a strategy the test invented. Nothing in deployment.evidence
/// covers `aqua` or `maker`.
contract UnevidencedSetupTest is Test {
    Aqua aqua;
    address maker = makeAddr("maker");
    address router = makeAddr("router");

    function setUp() public {
        aqua = new Aqua();
    }

    function test_makerShipsStrategy() public {
        address[] memory tokens = new address[](0);
        uint256[] memory amounts = new uint256[](0);
        vm.prank(maker);
        aqua.ship(router, "", tokens, amounts);
        assertEq(address(aqua).code.length > 0 ? 1 : 0, 1);
    }
}
