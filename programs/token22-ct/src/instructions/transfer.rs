use anchor_lang::prelude::*;
use anchor_spl::{
    token_2022::spl_token_2022::{
        extension::{
            transfer_fee::TransferFeeConfig, BaseStateWithExtensions, StateWithExtensions,
        },
        state::Mint as MintState,
    },
    token_interface::{transfer_checked_with_fee, TokenInterface, TransferCheckedWithFee},
};

#[derive(Accounts)]
pub struct TransferWithFee<'info> {
    pub authority: Signer<'info>,

    #[account(mut)]
    /// CHECK: token account, validated by the Token-2022 program.
    pub source: UncheckedAccount<'info>,

    #[account(mut)]
    /// CHECK: token account, validated by the Token-2022 program.
    pub destination: UncheckedAccount<'info>,

    #[account(mut)]
    /// CHECK: mint, read via StateWithExtensions and validated by the Token-2022 program.
    pub mint: UncheckedAccount<'info>,

    pub token_program: Interface<'info, TokenInterface>,
}

impl<'info> TransferWithFee<'info> {
    pub fn transfer(&self, amount: u64, decimals: u8) -> Result<()> {
        let data = self.mint.try_borrow_data()?;
        let mint = StateWithExtensions::<MintState>::unpack(&data)?;
        let fee = mint
            .get_extension::<TransferFeeConfig>()?
            .calculate_epoch_fee(Clock::get()?.epoch, amount)
            .ok_or(ProgramError::InvalidArgument)?;
        transfer_checked_with_fee(
            CpiContext::new(
                self.token_program.key(),
                TransferCheckedWithFee {
                    token_program_id: self.token_program.to_account_info(),
                    source: self.source.to_account_info(),
                    mint: self.mint.to_account_info(),
                    destination: self.destination.to_account_info(),
                    authority: self.authority.to_account_info(),
                },
            ),
            amount,
            decimals,
            fee,
        )
    }
}
