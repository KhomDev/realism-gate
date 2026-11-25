use anchor_lang::prelude::*;

declare_id!("Vau1t1111111111111111111111111111111111111");

#[program]
pub mod vault {
    use super::*;

    pub fn withdraw(ctx: Context<Withdraw>) -> Result<()> {
        let _ = ctx;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Withdraw<'info> {
    pub authority: Signer<'info>,
    pub vault: Account<'info, Vault>,
}

#[account]
pub struct Vault {
    pub authority: Pubkey,
}
