use anchor_lang::prelude::*;

#[program]
pub mod vault {
    use super::*;

    pub fn open(ctx: Context<Missing>) -> Result<()> {
        let _ = ctx;
        Ok(())
    }
}
