module vault::rg06 {
    #[test]
    fun test_mint_for_testing_to_attacker() {
        let attacker = @0xA11CE;
        let c = coin::mint_for_testing<SUI>(1000);
        transfer::public_transfer(c, attacker);
        let before = coin::value(&attacker_coin);
        assert!(coin::value(&attacker_coin) == before, 0);
    }
}
