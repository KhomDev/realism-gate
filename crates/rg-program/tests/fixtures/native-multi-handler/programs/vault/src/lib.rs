use solana_program::account_info::AccountInfo;
use solana_program::entrypoint::ProgramResult;
use solana_program::program_error::ProgramError;
use solana_program::pubkey::Pubkey;

pub fn process_deposit(_program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let payer = &accounts[0];
    if !payer.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    Ok(())
}

pub fn process_close(_program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let vault = &accounts[0];
    let _ = vault;
    Ok(())
}
