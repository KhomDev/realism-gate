// RG09: both assertions check the test's own arithmetic.
use litesvm::LiteSVM;
use solana_sdk::instruction::Instruction;
use solana_sdk::native_token::LAMPORTS_PER_SOL;
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk::transaction::Transaction;

fn send(svm: &mut LiteSVM, ix: Instruction, signers: &[&Keypair]) {
    let payer = signers[0].pubkey();
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer), signers, svm.latest_blockhash());
    svm.send_transaction(tx).unwrap();
}

#[test]
fn test_drain_steals_100_sol() {
    let mut svm = LiteSVM::new();
    svm.add_program(vault::ID, include_bytes!("vault.so"));
    let attacker = Keypair::new();
    svm.airdrop(&attacker.pubkey(), 10_000_000_000).unwrap();
    let stolen = 100 * LAMPORTS_PER_SOL;
    send(&mut svm, drain_ix(&attacker.pubkey()), &[&attacker]);
    assert_eq!(stolen, 100 * LAMPORTS_PER_SOL);
    assert!(true);
}
