use anchor_lang::prelude::*;

#[program]
pub mod vault {
    use super::*;

    pub fn fund(ctx: Context<Fund>) -> Result<()> {
        let _ = ctx;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Fund<'info> {
    pub payer: Box<Signer<'info>>,
    pub vault: Account<'info, Vault>,
}

#[account]
pub struct Vault {
    pub total: u64,
}
