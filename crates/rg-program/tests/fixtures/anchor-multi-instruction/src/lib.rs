use anchor_lang::prelude::*;

#[program]
pub mod vault {
    use super::*;

    pub fn withdraw(ctx: Context<Withdraw>) -> Result<()> {
        let _ = ctx;
        Ok(())
    }

    pub fn poke(ctx: Context<Poke>) -> Result<()> {
        let _ = ctx;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Withdraw<'info> {
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct Poke<'info> {
    pub vault: Account<'info, Vault>,
}

#[account]
pub struct Vault {
    pub total: u64,
}
