use anchor_lang::prelude::*;

#[program]
pub mod vault {
    use super::*;

    pub fn set_admin(ctx: &mut Context<SetAdmin>, new_admin: Pubkey) -> Result<()> {
        let _ = (ctx, new_admin);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct SetAdmin<'info> {
    pub admin: Signer<'info>,
}
