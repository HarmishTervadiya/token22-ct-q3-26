pub mod constants;
pub mod instructions;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;

declare_id!("7MhqudaCvXdPr5LKvvEuZt5UJ9k1g1kStkoXt3CxDEXM");

#[program]
pub mod token22_ct {
    use super::*;

    pub fn initialize(ctx: Context<InitializeMint>) -> Result<()> {
        ctx.accounts.initialize()
    }
}
