// A LiteSVM proof of concept with four problems realism-gate reports. It is
// an example, not a working exploit: the program and its instruction
// builders are not part of this repository.
use litesvm::LiteSVM;
use solana_client::rpc_client::RpcClient;
use solana_sdk::account::Account;
use solana_sdk::instruction::Instruction;
use solana_sdk::native_token::LAMPORTS_PER_SOL;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk::transaction::Transaction;

fn send(svm: &mut LiteSVM, ix: Instruction, signers: &[&Keypair]) {
    let payer = signers[0].pubkey();
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer), signers, svm.latest_blockhash());
    svm.send_transaction(tx).unwrap();
}

#[test]
fn test_attacker_drains_vault() {
    let rpc = RpcClient::new(std::env::var("RPC_URL").unwrap());
    let mut svm = LiteSVM::new();
    svm.add_program_from_file(vault::ID, "fixtures/vault.so").unwrap();
    let (vault_state, _bump) = Pubkey::find_program_address(&[b"state"], &vault::ID);
    svm.set_account(vault_state, rpc.get_account(&vault_state).unwrap()).unwrap();
    let admin = Keypair::new();
    let attacker = Keypair::new();
    svm.airdrop(&attacker.pubkey(), LAMPORTS_PER_SOL).unwrap();

    send(&mut svm, set_withdraw_fee_ix(&admin.pubkey(), 0), &[&admin]);
    let state = svm.get_account(&vault_state).unwrap();
    svm.set_account(vault_state, Account { lamports: 1_000_000 * LAMPORTS_PER_SOL, ..state }).unwrap();
    send(&mut svm, withdraw_ix(&attacker.pubkey()), &[&attacker]);
    assert!(svm.get_balance(&attacker.pubkey()).unwrap() > 0);
}
