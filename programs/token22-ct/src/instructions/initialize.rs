use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::invoke;
use anchor_spl::token_interface::{
    approve, initialize_mint2, mint_close_authority_initialize, spl_token_2022, transfer_checked,
    transfer_fee_initialize, Approve, InitializeMint2, Mint, MintCloseAuthorityInitialize,
    TokenInterface, TransferChecked, TransferFeeInitialize,
};
use spl_token_2022::{
    extension::{
        confidential_transfer::{instruction as confidential_instruction, DecryptableBalance},
        confidential_transfer_fee::instruction as confidential_fee_instruction,
        transfer_fee::TransferFeeConfig,
        BaseStateWithExtensions, ExtensionType, StateWithExtensions,
    },
    state::Mint as MintState,
};

#[derive(Accounts)]
pub struct InitializeMint<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(mut)]
    pub mint: Signer<'info>,

    pub system_program: Program<'info, System>,
    pub token_program: Interface<'info, TokenInterface>,
}

impl<'info> InitializeMint<'info> {
    pub fn intialize(&mut self) -> Result<()> {
        let extensions: &[ExtensionType] = &[
            ExtensionType::TransferFeeConfig,
            ExtensionType::DefaultAccountState,
            ExtensionType::MetadataPointer,
            ExtensionType::TokenMetadata,
            ExtensionType::MintCloseAuthority,
        ];

        let space = ExtensionType::try_calculate_account_len::<MintState>(&extensions)?;
        let lamports = Rent::get()?.minimum_balance(space);

        anchor_lang::system_program::create_account(
            CpiContext::new(
                self.system_program.key(),
                anchor_lang::system_program::CreateAccount {
                    from: self.payer.to_account_info(),
                    to: self.mint.to_account_info(),
                },
            ),
            lamports,
            space as u64,
            &self.token_program.key(),
        )?;

        Ok(())
    }
}
