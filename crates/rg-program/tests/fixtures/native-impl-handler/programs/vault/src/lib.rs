use solana_program::account_info::AccountInfo;
use solana_program::entrypoint::ProgramResult;
use solana_program::program_error::ProgramError;
use solana_program::pubkey::Pubkey;

pub struct Processor;

impl Processor {
    pub fn process(_program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
        let payer = &accounts[0];
        if !payer.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }
        Ok(())
    }
}
