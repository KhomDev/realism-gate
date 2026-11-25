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
    #[account(address = authority.key())]
    pub config: AccountInfo<'info>,
    pub authority: Signer<'info>,
}
