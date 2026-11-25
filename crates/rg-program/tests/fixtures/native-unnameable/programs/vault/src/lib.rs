use solana_program::account_info::AccountInfo;
use solana_program::entrypoint::ProgramResult;
use solana_program::program_error::ProgramError;
use solana_program::pubkey::Pubkey;

pub fn process_close(_program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    if !get_account(accounts, 0).is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    Ok(())
}

fn get_account<'a>(_accounts: &'a [AccountInfo], _index: usize) -> &'a AccountInfo<'a> {
    unimplemented!()
}
