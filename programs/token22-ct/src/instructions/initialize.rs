use std::u64;

use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::invoke;
use anchor_spl::{
    token_2022::spl_token_2022::state::AccountState,
    token_interface::{
        approve, default_account_state_initialize, initialize_mint2, metadata_pointer_initialize,
        mint_close_authority_initialize, spl_token_2022, token_metadata_initialize,
        transfer_checked, transfer_fee_initialize, Approve, DefaultAccountStateInitialize,
        InitializeMint2, MetadataPointerInitialize, Mint, MintCloseAuthorityInitialize,
        TokenInterface, TokenMetadataInitialize, TransferChecked, TransferFeeInitialize,
    },
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
            &self.payer.key(),
        )?;

        self.default_state_config()?;
        self.metadata_pointer_config()?;
        self.mint_close_config()?;
        self.token_metadatat_config()?;
        self.transfer_fee_config()?;

        initialize_mint2(
            CpiContext::new(
                self.token_program.key(),
                InitializeMint2 {
                    mint: self.mint.to_account_info(),
                },
            ),
            6,
            &self.payer.key(),
            None,
        )?;

        Ok(())
    }

    pub fn default_state_config(&mut self) -> Result<()> {
        default_account_state_initialize(
            CpiContext::new(
                self.token_program.key(),
                DefaultAccountStateInitialize {
                    mint: self.mint.to_account_info(),
                    token_program_id: self.token_program.to_account_info(),
                },
            ),
            &AccountState::Frozen,
        )
    }

    pub fn metadata_pointer_config(&mut self) -> Result<()> {
        metadata_pointer_initialize(
            CpiContext::new(
                self.token_program.key(),
                MetadataPointerInitialize {
                    mint: self.mint.to_account_info(),
                    token_program_id: self.token_program.to_account_info(),
                },
            ),
            Some(self.payer.key()),
            Some(self.mint.key()),
        )
    }

    pub fn mint_close_config(&mut self) -> Result<()> {
        mint_close_authority_initialize(
            CpiContext::new(
                self.token_program.key(),
                MintCloseAuthorityInitialize {
                    mint: self.mint.to_account_info(),
                    token_program_id: self.token_program.to_account_info(),
                },
            ),
            Some(&self.payer.key()),
        )
    }

    pub fn transfer_fee_config(&mut self) -> Result<()> {
        transfer_fee_initialize(
            CpiContext::new(
                self.token_program.key(),
                TransferFeeInitialize {
                    mint: self.mint.to_account_info(),
                    token_program_id: self.token_program.to_account_info(),
                },
            ),
            Some(&self.payer.key()),
            Some(&self.payer.key()),
            100,
            u64::MAX,
        )
    }

    pub fn token_metadatat_config(&mut self) -> Result<()> {
        token_metadata_initialize(
            CpiContext::new(
                self.token_program.key(),
                TokenMetadataInitialize {
                    mint: self.mint.to_account_info(),
                    mint_authority: self.payer.to_account_info(),
                    update_authority: self.payer.to_account_info(),
                    program_id: self.token_program.to_account_info(),
                    metadata: self.mint.to_account_info(),
                },
            ),
            String::from("Test name"),
            String::from("Test symbol"),
            String::from("Test uri"),
        )
    }
}
