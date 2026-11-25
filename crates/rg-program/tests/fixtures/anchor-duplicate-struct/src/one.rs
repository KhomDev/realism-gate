use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Vault<'info> {
    pub authority: Signer<'info>,
}
