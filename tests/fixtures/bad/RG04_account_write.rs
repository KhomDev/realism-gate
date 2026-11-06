// RG04: the PoC zeroes its own debt by writing the obligation account.
use litesvm::LiteSVM;
use solana_sdk::instruction::Instruction;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk::transaction::Transaction;

fn send(svm: &mut LiteSVM, ix: Instruction, signers: &[&Keypair]) {
    let payer = signers[0].pubkey();
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer), signers, svm.latest_blockhash());
    svm.send_transaction(tx).unwrap();
}

#[test]
fn test_withdraw_with_cleared_debt() {
    let mut svm = LiteSVM::new();
    svm.add_program(lending::ID, include_bytes!("lending.so"));
    let attacker = Keypair::new();
    svm.airdrop(&attacker.pubkey(), 10_000_000_000).unwrap();
    let (obligation, _bump) =
        Pubkey::find_program_address(&[b"obligation", attacker.pubkey().as_ref()], &lending::ID);
    send(&mut svm, borrow_ix(&attacker.pubkey(), 1_000), &[&attacker]);

    let mut account = svm.get_account(&obligation).unwrap();
    account.data[8..16].copy_from_slice(&0u64.to_le_bytes());
    svm.set_account(obligation, account).unwrap();

    send(&mut svm, withdraw_collateral_ix(&attacker.pubkey()), &[&attacker]);
    let after = svm.get_account(&obligation).unwrap();
    assert_eq!(after.data[8..16], [0u8; 8]);
}
