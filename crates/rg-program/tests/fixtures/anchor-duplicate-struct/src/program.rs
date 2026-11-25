use anchor_lang::prelude::*;

#[program]
pub mod vault {
    use super::*;

    pub fn open(ctx: Context<Vault>) -> Result<()> {
        let _ = ctx;
        Ok(())
    }
}
