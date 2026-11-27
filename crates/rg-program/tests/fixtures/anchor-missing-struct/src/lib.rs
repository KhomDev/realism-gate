use anchor_lang::prelude::*;

#[program]
pub mod vault {
    use super::*;

    pub fn open(ctx: Context<Ghost>) -> Result<()> {
        let _ = ctx;
        Ok(())
    }
}
