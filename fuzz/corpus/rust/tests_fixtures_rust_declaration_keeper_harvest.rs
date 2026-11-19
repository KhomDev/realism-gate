// The attack needs the keeper's scheduled harvest. Without a declaration the
// keeper's signature is flagged; tests/fixtures/declaration/realism.yaml
// declares mallet the attacker and the keeper's role.
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
    svm.add_program_from_file(yield_vault::ID, "fixtures/yield_vault.so").unwrap();
    let admin = Keypair::new();
    let keeper = Keypair::new();
    let mallet = Keypair::new();
    for who in [&admin, &keeper, &mallet] {
        svm.airdrop(&who.pubkey(), 10_000_000_000).unwrap();
    }
    send(&mut svm, init_vault_ix(&admin.pubkey(), &keeper.pubkey()), &[&admin]);
    (svm, keeper, mallet)
}

#[test]
fn test_deposit_right_before_the_harvest() {
    let (mut svm, keeper, mallet) = setup();
    let before = svm.get_balance(&mallet.pubkey()).unwrap();
    send(&mut svm, deposit_ix(&mallet.pubkey(), 1_000_000), &[&mallet]);
    send(&mut svm, harvest_ix(&keeper.pubkey()), &[&keeper]);
    send(&mut svm, withdraw_ix(&mallet.pubkey()), &[&mallet]);
    assert!(svm.get_balance(&mallet.pubkey()).unwrap() > before);
}
