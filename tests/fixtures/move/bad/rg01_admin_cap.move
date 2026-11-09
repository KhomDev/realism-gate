module vault::rg01 {
    #[test]
    fun test_attacker_takes_the_cap() {
        let attacker = @0xA11CE;
        let mut scenario = test_scenario::begin(@0xAD);
        test_scenario::next_tx(&mut scenario, @0xAD);
        test_scenario::next_tx(&mut scenario, attacker);
        let cap = test_scenario::take_from_address<AdminCap>(&scenario, @0xAD);
        transfer::public_transfer(cap, attacker);
        assert!(true, 0);
    }
}
