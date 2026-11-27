use anchor_lang::prelude::*;

#[program]
pub mod vault {
    use super::*;

    pub fn open(ctx: Context<Open>) -> Result<()> {
        let _ = ctx;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Open<'info> {
    pub authority: Option<Signer<'info>>,
    pub vault: Account<'info, Vault>,
}

#[account]
pub struct Vault {
    pub authority: Pubkey,
}
