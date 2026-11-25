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
    #[account(owner = token_program.key())]
    pub vault: AccountInfo<'info>,
    pub token_program: Program<'info, Token>,
}
