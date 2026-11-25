use solana_program::account_info::AccountInfo;
use solana_program::entrypoint::ProgramResult;
use solana_program::program_error::ProgramError;
use solana_program::pubkey::Pubkey;

pub fn process_withdraw(_program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let admin = &accounts[0];
    let operator = &accounts[1];
    require!(
        admin.is_signer || operator.is_signer,
        ProgramError::MissingRequiredSignature
    );
    Ok(())
}
