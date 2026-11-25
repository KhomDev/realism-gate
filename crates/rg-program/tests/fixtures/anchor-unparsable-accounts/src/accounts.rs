use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Withdraw<'info> {
    pub authority: Signer<'info>,
