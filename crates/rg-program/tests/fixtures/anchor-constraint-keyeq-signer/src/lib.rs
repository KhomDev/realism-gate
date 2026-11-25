use anchor_lang::prelude::*;

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
    #[account(constraint = config.authority == authority.key())]
    pub config: Account<'info, Config>,
    pub authority: Signer<'info>,
}

#[account]
pub struct Config {
    pub authority: Pubkey,
}
