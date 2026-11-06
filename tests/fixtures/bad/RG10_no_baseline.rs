// RG10: "more than zero" over a balance the test funded itself.
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
fn test_drain_pays_the_attacker() {
    let mut svm = LiteSVM::new();
    svm.add_program(vault::ID, include_bytes!("vault.so"));
    let attacker = Keypair::new();
    svm.airdrop(&attacker.pubkey(), 10_000_000_000).unwrap();
    send(&mut svm, drain_ix(&attacker.pubkey()), &[&attacker]);
    assert!(svm.get_balance(&attacker.pubkey()).unwrap() > 0);
}
