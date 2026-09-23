use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::invoke;
use anchor_spl::{
    token_2022::spl_token_2022::extension::confidential_transfer::{
        instruction as ct_ix, DecryptableBalance,
    },
    token_interface::TokenInterface,
};

#[derive(Accounts)]
pub struct ApplyPendingBalance<'info> {
    #[account(mut, owner = token_program.key())]
    /// CHECK
    pub token_account: UncheckedAccount<'info>,

    pub authority: Signer<'info>,
    pub token_program: Interface<'info, TokenInterface>,
}

impl<'info> ApplyPendingBalance<'info> {
    pub fn apply(
        &self,
        expected_pending_balance_credit_counter: u64,
        new_decryptable_available_balance: [u8; 36],
    ) -> Result<()> {
        let balance = DecryptableBalance::from(new_decryptable_available_balance);
        let ix = ct_ix::apply_pending_balance(
            &self.token_program.key(),
            &self.token_account.key(),
            expected_pending_balance_credit_counter,
            &balance,
            &self.authority.key(),
            &[],
        )?;
        invoke(
            &ix,
            &[
                self.token_account.to_account_info(),
                self.authority.to_account_info(),
                self.token_program.to_account_info(),
            ],
        )?;
        Ok(())
    }
}
