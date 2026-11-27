use anchor_lang::prelude::*;

#[program]
pub mod vault {
    use super::*;

    pub fn peek(ctx: Context<Peek>) -> Result<()> {
        let _ = ctx;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Peek<'info> {
    pub vault: Account<'info, Vault>,
    pub system_program: Program<'info, System>,
}

#[account]
pub struct Vault {
    pub total: u64,
}
