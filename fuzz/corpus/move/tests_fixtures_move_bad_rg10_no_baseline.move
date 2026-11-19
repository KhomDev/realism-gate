module vault::rg10 {
    #[test]
    fun test_profit_without_a_before() {
        let attacker = @0xA11CE;
        test_scenario::next_tx(&mut scenario, attacker);
        vault::withdraw();
        assert!(coin::value(&attacker_coin) > 0, 0);
    }
}
