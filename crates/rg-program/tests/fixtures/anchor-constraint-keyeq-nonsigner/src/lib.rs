use anchor_lang::prelude::*;

#[program]
pub mod vault {
    use super::*;

    pub fn close(ctx: Context<Close>) -> Result<()> {
        let _ = ctx;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Close<'info> {
    #[account(constraint = config.owner == treasury.key())]
    pub config: Account<'info, Config>,
    pub treasury: AccountInfo<'info>,
}

#[account]
pub struct Config {
    pub owner: Pubkey,
}
