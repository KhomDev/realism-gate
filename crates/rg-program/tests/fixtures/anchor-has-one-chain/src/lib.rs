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
    #[account(
        has_one = authority,
        has_one = operator
    )]
    pub vault: Account<'info, Vault>,
    pub authority: Signer<'info>,
    pub operator: Signer<'info>,
}

#[account]
pub struct Vault {
    pub authority: Pubkey,
    pub operator: Pubkey,
}
