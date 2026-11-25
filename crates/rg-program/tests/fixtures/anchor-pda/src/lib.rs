use anchor_lang::prelude::*;

#[program]
pub mod vault {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        let _ = ctx;
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(seed: u64)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = user,
        seeds = [b"vault", user.key().as_ref(), &seed.to_le_bytes()],
        bump,
        space = 8 + 32
    )]
    pub vault: Account<'info, Vault>,
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[account]
pub struct Vault {
    pub seed: u64,
}
