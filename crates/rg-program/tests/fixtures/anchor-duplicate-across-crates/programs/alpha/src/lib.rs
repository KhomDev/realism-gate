use anchor_lang::prelude::*;

#[program]
pub mod alpha {
    use super::*;

    pub fn open(ctx: Context<Open>) -> Result<()> {
        let _ = ctx;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Open<'info> {
    pub authority: Signer<'info>,
}
