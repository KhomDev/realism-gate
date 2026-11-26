// Twin: the same test written out, outside any macro. It must be read.
use litesvm::LiteSVM;
use solana_sdk::signature::{Keypair, Signer};

#[test]
fn drains_one() {
    let mut svm = LiteSVM::new();
    let attacker = Keypair::new();
    svm.airdrop(&attacker.pubkey(), 1).unwrap();
    assert_eq!(svm.get_balance(&attacker.pubkey()), Some(1));
}
