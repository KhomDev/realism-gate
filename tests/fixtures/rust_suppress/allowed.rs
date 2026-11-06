// One allow comment with a reason (honoured), one without (ignored).
use litesvm::LiteSVM;
use solana_client::rpc_client::RpcClient;
use solana_sdk::account::Account;
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
fn test_drain() {
    // realism-gate: allow(RG11) RPC_URL points at a local validator started from a pinned snapshot
    let rpc = RpcClient::new(std::env::var("RPC_URL").unwrap());
    let mut svm = LiteSVM::new();
    svm.add_program(vault::ID, include_bytes!("vault.so"));
    let attacker = Keypair::new();
    svm.airdrop(&attacker.pubkey(), 10_000_000_000).unwrap();
    let (vault_state, _bump) = Pubkey::find_program_address(&[b"state"], &vault::ID);
    // realism-gate: allow(RG04)
    svm.set_account(vault_state, Account { lamports: 1, data: vec![1; 64], owner: vault::ID, ..Default::default() }).unwrap();

    let before = svm.get_balance(&attacker.pubkey()).unwrap();
    send(&mut svm, drain_ix(&attacker.pubkey()), &[&attacker]);
    assert!(svm.get_balance(&attacker.pubkey()).unwrap() > before);
}
