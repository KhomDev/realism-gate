use anchor_lang::prelude::*;

#[program]
pub mod vault {
    use super::*;

    pub fn init(ctx: Context<Init>) -> Result<()> {
        let _ = ctx;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Init<'info> {
    #[account(
        init_if_needed,
        payer = payer,
        seeds = [b"config", config.key().as_ref()],
        bump
    )]
    pub vault: Account<'info, Vault>,
    pub config: Account<'info, Config>,
    #[account(mut)]
    pub payer: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
}

#[account]
pub struct Vault {
    pub config: Pubkey,
}

#[account]
pub struct Config {
    pub paused: bool,
}
