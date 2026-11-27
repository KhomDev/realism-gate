use solana_program::account_info::AccountInfo;
use solana_program::entrypoint::ProgramResult;
use solana_program::program_error::ProgramError;
use solana_program::pubkey::Pubkey;

pub fn process_close(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let vault = &accounts[0];
    if vault.owner != program_id {
        return Err(ProgramError::IncorrectProgramId);
    }
    Ok(())
}
