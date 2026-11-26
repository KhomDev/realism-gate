// SPDX-License-Identifier: MIT
// Conformance scenario "vault drain", Solidity (Foundry). Synthetic.
// The admin initialises the vault in setUp; the attacker is funded with a
// cheatcode (the mutation), sends the withdrawal (the exploit), and the test
// asserts its balance grew. Foundry has no separate fee payer.
pragma solidity ^0.8.20;

import "forge-std/Test.sol";

interface IVault {
    function initialize() external;
    function withdraw() external;
}

contract VaultDrainTest is Test {
    IVault vault;
    address admin = makeAddr("admin");
    address attacker = makeAddr("attacker");

    function setUp() public {
        vault = IVault(deployCode("Vault.sol"));
        vm.prank(admin);
        vault.initialize();
        vm.deal(attacker, 1 ether);
    }

    function testAttackerDrainsTheVault() public {
        uint256 before = attacker.balance;
        vm.prank(attacker);
        vault.withdraw();
        assertGt(attacker.balance, before);
    }
}
