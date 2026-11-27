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
    pub vault: VaultAccounts<'info>,
}

#[derive(Accounts)]
pub struct VaultAccounts<'info> {
    pub authority: Signer<'info>,
}
