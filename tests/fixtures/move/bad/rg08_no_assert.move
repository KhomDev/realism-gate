module vault::rg08 {
    #[test]
    fun test_withdraws_and_prints() {
        let attacker = @0xA11CE;
        test_scenario::next_tx(&mut scenario, attacker);
        vault::withdraw();
    }
}
