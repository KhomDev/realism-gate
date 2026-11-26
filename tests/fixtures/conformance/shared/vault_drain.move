// Conformance scenario "vault drain", Sui Move. Synthetic.
// The admin's transaction initialises the vault; a test-only mint funds the
// attacker (the mutation); the attacker's transaction withdraws (the
// exploit); the test asserts the attacker's balance grew. A Move test has no
// separate fee payer.
#[test_only]
module vault::vault_drain_tests {
    use sui::coin;
    use sui::sui::SUI;
    use sui::test_scenario;
    use vault::vault;

    #[test(admin = @0xAD, attacker = @0xA11CE)]
    fun attacker_drains_the_vault(admin: address, attacker: address) {
        let mut scenario = test_scenario::begin(admin);
        vault::initialize(test_scenario::ctx(&mut scenario));
        test_scenario::next_tx(&mut scenario, attacker);
        let seed = coin::mint_for_testing<SUI>(1_000, test_scenario::ctx(&mut scenario));
        let before = coin::value(&seed);
        let loot = vault::withdraw(seed, test_scenario::ctx(&mut scenario));
        assert!(coin::value(&loot) > before, 0);
        transfer::public_transfer(loot, attacker);
        test_scenario::end(scenario);
    }
}
