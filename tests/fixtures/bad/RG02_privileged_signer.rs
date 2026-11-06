// RG02: the admin moves the price in the middle of the exploit.
use litesvm::LiteSVM;
use solana_sdk::instruction::Instruction;
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk::transaction::Transaction;

fn send(svm: &mut LiteSVM, ix: Instruction, signers: &[&Keypair]) {
    let payer = signers[0].pubkey();
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer), signers, svm.latest_blockhash());
    svm.send_transaction(tx).unwrap();
}

fn setup() -> (LiteSVM, Keypair, Keypair) {
    let mut svm = LiteSVM::new();
    svm.add_program(lending::ID, include_bytes!("lending.so"));
    let admin = Keypair::new();
    let attacker = Keypair::new();
    svm.airdrop(&admin.pubkey(), 10_000_000_000).unwrap();
    svm.airdrop(&attacker.pubkey(), 10_000_000_000).unwrap();
    send(&mut svm, init_market_ix(&admin.pubkey()), &[&admin]);
    (svm, admin, attacker)
}

#[test]
fn test_borrow_after_price_drop() {
    let (mut svm, admin, attacker) = setup();
    let before = svm.get_balance(&attacker.pubkey()).unwrap();
    send(&mut svm, set_price_ix(&admin.pubkey(), 1), &[&admin]);
    send(&mut svm, borrow_ix(&attacker.pubkey(), 1_000), &[&attacker]);
    assert!(svm.get_balance(&attacker.pubkey()).unwrap() > before);
}
