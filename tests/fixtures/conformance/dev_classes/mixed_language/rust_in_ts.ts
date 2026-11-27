// Mixed-language, other direction: a Rust test saved under a .ts name.
use solana_sdk::signature::Keypair;

#[test]
fn attacker_withdraws() {
    let attacker = Keypair::new();
    assert!(attacker.pubkey() != Pubkey::default());
}
