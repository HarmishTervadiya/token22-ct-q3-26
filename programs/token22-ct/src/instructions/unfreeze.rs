use anchor_lang::prelude::*;
use anchor_spl::token_interface::{thaw_account, ThawAccount, TokenInterface};

#[derive(Accounts)]
pub struct UnfreezeKycAccount<'info> {
    pub freeze_authority: Signer<'info>,

    #[account(mut)]
    /// CHECK: token account, validated by the Token-2022 program.
    pub token_account: UncheckedAccount<'info>,

    /// CHECK: mint, validated by the Token-2022 program.
    pub mint: UncheckedAccount<'info>,

    pub token_program: Interface<'info, TokenInterface>,
}

impl<'info> UnfreezeKycAccount<'info> {
    pub fn unfreeze(&self) -> Result<()> {
        thaw_account(CpiContext::new(
            self.token_program.key(),
            ThawAccount {
                account: self.token_account.to_account_info(),
                mint: self.mint.to_account_info(),
                authority: self.freeze_authority.to_account_info(),
            },
        ))
    }
}
