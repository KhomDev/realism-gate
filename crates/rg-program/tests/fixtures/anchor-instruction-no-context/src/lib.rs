use anchor_lang::prelude::*;

#[program]
pub mod vault {
    use super::*;

    pub fn bad(amount: u64) -> Result<()> {
        let _ = amount;
        Ok(())
    }
}
