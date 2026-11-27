use anchor_lang::prelude::*;

#[program]
pub mod beta {
    use super::*;

    pub fn open(ctx: Context<Open>) -> Result<()> {
        let _ = ctx;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Open<'info> {
    pub payer: Signer<'info>,
}
