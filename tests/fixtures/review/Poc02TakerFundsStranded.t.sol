// SPDX-License-Identifier: LicenseRef-Degensoft-SwapVM-1.1
pragma solidity ^0.8.27;

/// @custom:license-url https://github.com/1inch/swap-vm/blob/main/LICENSES/SwapVM-1.1.txt
/// @custom:copyright (c) 2025 Degensoft Ltd

import { TokenMock } from "@1inch/solidity-utils/contracts/mocks/TokenMock.sol";
import { IAqua } from "@1inch/aqua/src/interfaces/IAqua.sol";

import { ISwapVM } from "../../../contracts/interfaces/ISwapVM.sol";
import { AquaSwapVMTest, SwapProgram } from "./base/AquaSwapVMTest.sol";
import { TraitsHelper } from "./helpers/SwapVMTestSetup.sol";
import { XYCSwap } from "../../../contracts/instructions/XYCSwap.sol";
import { Salt } from "../../../contracts/instructions/Controls.sol";
import { dynamic } from "./utils/Dynamic.sol";

/**
 * @title POC-02: taker pre-push stranded by maker dock
 * @notice Taker pre-pushes tokenIn via a SEPARATE Aqua.push tx (permissionless).
 *         Maker docks the full token list. The paired swap reverts in
 *         SwapVM.swap via AQUA.safeBalances (DOCKED), while the pushed
 *         tokens already moved to the maker wallet. Every recovery attempt
 *         available to the taker fails: no taker pull path, ledger reads
 *         DOCKED, no clawback.
 */
contract Poc02TakerFundsStranded is AquaSwapVMTest {
    uint256 constant PRE_PUSH = 100e18;
    uint256 constant SWAP_AMOUNT = 100e18;

    function buildPlainXycProgram() internal view returns (bytes memory) {
        return bytes.concat(XYCSwap.build(), Salt.build(abi.encodePacked(vm.randomUint())));
    }

    function buildOrder(bytes memory program) internal view returns (ISwapVM.Order memory) {
        return orders.MakerTraitsLibBuild(TraitsHelper.MakerTraitsLibArgs({
            maker: maker,
            tokenA: address(tokenA),
            tokenB: address(tokenB),
            shouldUnwrapWeth: false,
            useAquaInsteadOfSignature: true,
            usePermit2: false,
            allowZeroAmountIn: false,
            receiver: address(0),
            program: program
        }));
    }

    function test_Poc02_TakerPrePushStrandedByMakerDock() public {
        ISwapVM.Order memory order = buildOrder(buildPlainXycProgram());
        bytes32 orderHash = swapVM.hash(order);
        (TokenMock tokenIn, TokenMock tokenOut) = (tokenA, tokenB);

        // 1. Maker ships a live 2-token strategy to the real router.
        shipStrategy(order, tokenIn, tokenOut, INITIAL_BALANCE_A, INITIAL_BALANCE_B);

        uint256 makerWalletBefore = tokenIn.balanceOf(maker);
        uint256 takerWalletBefore = tokenIn.balanceOf(address(taker));

        // 2. Taker pre-pushes X in a SEPARATE permissionless tx.
        tokenIn.mint(address(taker), PRE_PUSH);
        vm.prank(address(taker));
        tokenIn.approve(address(aqua), PRE_PUSH);
        vm.prank(address(taker));
        aqua.push(maker, address(swapVM), orderHash, address(tokenIn), PRE_PUSH);

        // Real fund flow: taker wallet -> maker wallet; virtual credit recorded.
        assertEq(tokenIn.balanceOf(maker), makerWalletBefore + PRE_PUSH, "maker wallet gains pre-push");
        (uint256 balAfterPush, ) = aqua.rawBalances(maker, address(swapVM), orderHash, address(tokenIn));
        assertEq(balAfterPush, INITIAL_BALANCE_A + PRE_PUSH, "ledger credits pre-push");

        // 3. Maker docks the full token list.
        vm.prank(maker);
        aqua.dock(address(swapVM), orderHash, dynamic([address(tokenIn), address(tokenOut)]));

        (uint256 balIn, uint8 countIn) = aqua.rawBalances(maker, address(swapVM), orderHash, address(tokenIn));
        (uint256 balOut, uint8 countOut) = aqua.rawBalances(maker, address(swapVM), orderHash, address(tokenOut));
        assertEq(balIn, 0, "tokenIn ledger zeroed");
        assertEq(countIn, 0xff, "tokenIn DOCKED");
        assertEq(balOut, 0, "tokenOut ledger zeroed");
        assertEq(countOut, 0xff, "tokenOut DOCKED");

        // 4. The paired swap reverts (safeBalances guard, SwapVM.swap L219).
        SwapProgram memory sp = SwapProgram({
            amount: SWAP_AMOUNT,
            taker: taker,
            tokenA: tokenA,
            tokenB: tokenB,
            zeroForOne: true,
            isExactIn: true
        });
        mintTokenInToTaker(sp);
        vm.expectRevert(
            abi.encodeWithSelector(
                IAqua.SafeBalancesForTokenNotInActiveStrategy.selector,
                maker,
                address(swapVM),
                orderHash,
                address(tokenIn)
            )
        );
        swap(sp, order);

        // 5a. Taker pull under its own app namespace underflows.
        vm.prank(address(taker));
        vm.expectRevert();
        aqua.pull(maker, orderHash, address(tokenIn), PRE_PUSH, address(taker));

        // 5b. Even the app-namespace pull underflows: the ledger was zeroed.
        vm.prank(address(swapVM));
        vm.expectRevert();
        aqua.pull(maker, orderHash, address(tokenIn), PRE_PUSH, address(taker));

        // 5c. safeBalances still reverts after the failed recovery attempts.
        vm.expectRevert(
            abi.encodeWithSelector(
                IAqua.SafeBalancesForTokenNotInActiveStrategy.selector,
                maker,
                address(swapVM),
                orderHash,
                address(tokenIn)
            )
        );
        aqua.safeBalances(maker, address(swapVM), orderHash, address(tokenIn), address(tokenOut));

        // 6. Terminal balances: taker is down X tokenIn, received 0 tokenOut;
        //    maker wallet keeps X (the ERC20 balance never moved after the push).
        assertEq(tokenIn.balanceOf(maker), makerWalletBefore + PRE_PUSH, "maker keeps pushed tokens");
        assertEq(tokenOut.balanceOf(address(taker)), 0, "taker receives no tokenOut");
        assertEq(
            tokenIn.balanceOf(address(taker)),
            takerWalletBefore + SWAP_AMOUNT,
            "taker keeps only the unspent swap mint; pre-push sits with maker"
        );
        (uint256 balFinal, uint8 countFinal) =
            aqua.rawBalances(maker, address(swapVM), orderHash, address(tokenIn));
        assertEq(balFinal, 0, "ledger still zero");
        assertEq(countFinal, 0xff, "strategy still DOCKED");
    }

    function test_Poc02_Control_PrePushSwapSucceeds() public {
        ISwapVM.Order memory order = buildOrder(buildPlainXycProgram());
        shipStrategy(order, tokenA, tokenB, INITIAL_BALANCE_A, INITIAL_BALANCE_B);

        tokenA.mint(address(taker), PRE_PUSH);
        vm.prank(address(taker));
        tokenA.approve(address(aqua), PRE_PUSH);
        vm.prank(address(taker));
        aqua.push(maker, address(swapVM), swapVM.hash(order), address(tokenA), PRE_PUSH);

        SwapProgram memory sp = SwapProgram({
            amount: SWAP_AMOUNT,
            taker: taker,
            tokenA: tokenA,
            tokenB: tokenB,
            zeroForOne: true,
            isExactIn: true
        });
        mintTokenInToTaker(sp);
        (uint256 amountIn, uint256 amountOut) = swap(sp, order);
        assertEq(amountIn, SWAP_AMOUNT, "control swap amountIn");
        assertGt(amountOut, 0, "control swap pays tokenOut");
        assertGt(tokenB.balanceOf(address(taker)), 0, "control taker receives tokenOut");
    }
}
