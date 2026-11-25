use anchor_lang::prelude::*;

#[account]
pub struct Vault {
    pub authority: Pubkey,
}
