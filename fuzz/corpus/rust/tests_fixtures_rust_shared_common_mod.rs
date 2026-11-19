// Shared test code, as in a crate's tests/common/mod.rs.
use litesvm::LiteSVM;
use solana_sdk::instruction::Instruction;
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk::transaction::Transaction;

pub struct World {
    pub svm: LiteSVM,
    pub admin: Keypair,
    pub attacker: Keypair,
}

pub fn send(svm: &mut LiteSVM, ix: Instruction, signers: &[&Keypair]) {
    let payer = signers[0].pubkey();
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer), signers, svm.latest_blockhash());
    svm.send_transaction(tx).unwrap();
}

pub fn setup() -> World {
    let mut svm = LiteSVM::new();
    svm.add_program_from_file(market::ID, "fixtures/market.so").unwrap();
    let admin = Keypair::new();
    let attacker = Keypair::new();
    svm.airdrop(&admin.pubkey(), 10_000_000_000).unwrap();
    svm.airdrop(&attacker.pubkey(), 10_000_000_000).unwrap();
    send(&mut svm, init_market_ix(&admin.pubkey()), &[&admin]);
    World { svm, admin, attacker }
}
