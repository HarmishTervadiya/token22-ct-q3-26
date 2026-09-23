use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::invoke;
use anchor_spl::{
    token_2022::spl_token_2022::extension::confidential_transfer::instruction as ct_ix,
    token_interface::TokenInterface,
};

#[derive(Accounts)]
pub struct DepositConfidential<'info> {
    #[account(mut, owner = token_program.key())]
    /// CHECK
    pub token_account: UncheckedAccount<'info>,

    #[account(owner = token_program.key())]
    /// CHECK
    pub mint: UncheckedAccount<'info>,

    pub authority: Signer<'info>,
    pub token_program: Interface<'info, TokenInterface>,
}

impl<'info> DepositConfidential<'info> {
    pub fn deposit(&self, amount: u64, decimals: u8) -> Result<()> {
        let ix = ct_ix::deposit(
            &self.token_program.key(),
            &self.token_account.key(),
            &self.mint.key(),
            amount,
            decimals,
            &self.authority.key(),
            &[],
        )?;
        invoke(
            &ix,
            &[
                self.token_account.to_account_info(),
                self.mint.to_account_info(),
                self.authority.to_account_info(),
                self.token_program.to_account_info(),
            ],
        )?;
        Ok(())
    }
}
