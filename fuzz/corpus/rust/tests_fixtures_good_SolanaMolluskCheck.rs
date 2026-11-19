// A Mollusk test: the validating run's checks are its assertions.
use mollusk_svm::Mollusk;
use mollusk_svm::result::Check;
use solana_sdk::account::Account;
use solana_sdk::pubkey::Pubkey;

#[test]
fn test_withdraw_more_than_deposited_fails() {
    let program_id = Pubkey::new_unique();
    let mollusk = Mollusk::new(&program_id, "vault");
    let user = Pubkey::new_unique();
    let accounts = vec![(user, Account::new(1_000_000, 0, &solana_sdk::system_program::id()))];
    let ix = withdraw_ix(&program_id, &user, 2_000_000);
    mollusk.process_and_validate_instruction(&ix, &accounts, &[Check::err(ProgramError::InsufficientFunds)]);
}
