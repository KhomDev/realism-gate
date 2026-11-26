// DEV class "tests generated inside macros". Synthetic: the test exists only
// after `for_each_token!` expands. A syntax tree has no expansion, so the file
// must be reported as unreadable, with the macro named as the reason.
use litesvm::LiteSVM;
use solana_sdk::signature::{Keypair, Signer};

for_each_token!(
    (Classic, classic_tests),
    mod drain {
        use super::*;

        #[tokio::test]
        async fn attacker_drains(state: &mut State) {
            let mut svm = LiteSVM::new();
            let attacker = Keypair::new();
            svm.airdrop(&attacker.pubkey(), 1).unwrap();
            assert_eq!(svm.get_balance(&attacker.pubkey()), Some(1));
        }
    }
);
