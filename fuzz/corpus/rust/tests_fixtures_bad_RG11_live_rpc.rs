// RG11: the pool is read from a live cluster when the test runs.
use litesvm::LiteSVM;
use solana_client::rpc_client::RpcClient;
use solana_sdk::instruction::Instruction;
use solana_sdk::pubkey;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk::transaction::Transaction;

const POOL: Pubkey = pubkey!("PooL111111111111111111111111111111111111111");

fn send(svm: &mut LiteSVM, ix: Instruction, signers: &[&Keypair]) {
    let payer = signers[0].pubkey();
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer), signers, svm.latest_blockhash());
    svm.send_transaction(tx).unwrap();
}

#[test]
fn test_swap_against_the_live_pool() {
    let rpc = RpcClient::new(std::env::var("RPC_URL").unwrap());
    let mut svm = LiteSVM::new();
    svm.add_program(amm::ID, include_bytes!("amm.so"));
    let pool = rpc.get_account(&POOL).unwrap();
    svm.set_account(POOL, pool).unwrap();
    let attacker = Keypair::new();
    svm.airdrop(&attacker.pubkey(), 10_000_000_000).unwrap();

    let before = svm.get_balance(&attacker.pubkey()).unwrap();
    send(&mut svm, swap_ix(&attacker.pubkey(), &POOL), &[&attacker]);
    assert!(svm.get_balance(&attacker.pubkey()).unwrap() > before);
}
