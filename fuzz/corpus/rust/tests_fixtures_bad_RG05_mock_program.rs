// RG05: a mock price feed is loaded where the real one lives.
use litesvm::LiteSVM;
use solana_sdk::instruction::Instruction;
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk::transaction::Transaction;

fn send(svm: &mut LiteSVM, ix: Instruction, signers: &[&Keypair]) {
    let payer = signers[0].pubkey();
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer), signers, svm.latest_blockhash());
    svm.send_transaction(tx).unwrap();
}

#[test]
fn test_liquidate_on_stale_price() {
    let mut svm = LiteSVM::new();
    svm.add_program(lending::ID, include_bytes!("lending.so"));
    svm.add_program(price_feed::ID, include_bytes!("mock_price_feed.so"));
    let attacker = Keypair::new();
    svm.airdrop(&attacker.pubkey(), 10_000_000_000).unwrap();

    let before = svm.get_balance(&attacker.pubkey()).unwrap();
    send(&mut svm, liquidate_ix(&attacker.pubkey()), &[&attacker]);
    assert!(svm.get_balance(&attacker.pubkey()).unwrap() > before);
}
