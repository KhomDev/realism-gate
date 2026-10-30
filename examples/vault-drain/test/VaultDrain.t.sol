// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test} from "forge-std/Test.sol";
import {Vault} from "src/Vault.sol";
import {IERC20} from "src/interfaces/IERC20.sol";

/// A PoC of the kind that passes `forge test` and still fails triage.
/// Every line realism-gate flags here is something a triager would ask about.
contract VaultDrainTest is Test {
    Vault vault;
    IERC20 token;
    address admin = makeAddr("admin");
    address attacker = makeAddr("attacker");

    function setUp() public {
        vm.createSelectFork(vm.envString("RPC_URL"));
        vault = new Vault(admin, address(token));
        deal(address(token), attacker, 10e18);
    }

    function test_attackerDrainsVault() public {
        // "Configure" the vault the way the exploit needs it.
        vm.prank(admin);
        vault.setWithdrawalFee(0);

        // Seed the vault so there is something to steal.
        deal(address(token), address(vault), 1_000_000e18);

        vm.prank(attacker);
        vault.withdrawAll(attacker);

        assertGt(token.balanceOf(attacker), 0);
    }
}
