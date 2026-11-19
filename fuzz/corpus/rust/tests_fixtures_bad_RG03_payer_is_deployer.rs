// RG03: the payer sets the vault up and then sweeps it as the same key.
use solana_program_test::{ProgramTest, processor};
use solana_sdk::signature::Signer;
use solana_sdk::transaction::Transaction;

#[tokio::test]
async fn test_sweep_fees() {
    let program_test = ProgramTest::new("fee_vault", fee_vault::ID, processor!(fee_vault::process));
    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;
    let init = Transaction::new_signed_with_payer(
        &[init_vault_ix(&payer.pubkey())],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    );
    banks_client.process_transaction(init).await.unwrap();

    let before = banks_client.get_balance(payer.pubkey()).await.unwrap();
    let sweep = Transaction::new_signed_with_payer(
        &[sweep_ix(&payer.pubkey())],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    );
    banks_client.process_transaction(sweep).await.unwrap();
    assert!(banks_client.get_balance(payer.pubkey()).await.unwrap() > before);
}
