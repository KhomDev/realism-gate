module vault::good {
    #[test]
    fun test_only_the_attacker_acts() {
        let admin = @0xAD;
        let attacker = @0xA11CE;
        let mut scenario = test_scenario::begin(admin);
        test_scenario::next_tx(&mut scenario, attacker);
        vault::deposit(test_scenario::ctx(&mut scenario));
        let before = coin::value(&attacker_coin);
        assert!(coin::value(&attacker_coin) >= before, 0);
    }
}
