use anchor_lang::prelude::*;

#[program]
pub mod vault {
    use super::*;

    pub fn settle(ctx: Context<Settle>) -> Result<()> {
        let _ = ctx;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Settle<'info> {
    #[account(mut, has_one = admin @ VaultError::Unauthorized)]
    pub config: Account<'info, Config>,
    pub admin: Signer<'info>,
}

#[account]
pub struct Config {
    pub admin: Pubkey,
}

#[error_code]
pub enum VaultError {
    #[msg("unauthorized")]
    Unauthorized,
}
