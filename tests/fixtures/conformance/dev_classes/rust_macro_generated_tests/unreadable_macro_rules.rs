// DEV class "tests generated inside macros", macro_rules variant. Synthetic:
// the tests are stamped out by a local macro_rules! definition.
use litesvm::LiteSVM;
use solana_sdk::signature::{Keypair, Signer};

macro_rules! drain_test {
    ($name:ident, $amount:expr) => {
        #[test]
        fn $name() {
            let mut svm = LiteSVM::new();
            let attacker = Keypair::new();
            svm.airdrop(&attacker.pubkey(), $amount).unwrap();
            assert_eq!(svm.get_balance(&attacker.pubkey()), Some($amount));
        }
    };
}

drain_test!(drains_one, 1);
drain_test!(drains_two, 2);
