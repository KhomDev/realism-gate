use solana_program::account_info::AccountInfo;
use solana_program::entrypoint::ProgramResult;
use solana_program::pubkey::Pubkey;

pub fn process_peek(_program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let authority = &accounts[0];
    if authority.is_signer {
        log_privileged_view();
    }
    Ok(())
}

fn log_privileged_view() {}
