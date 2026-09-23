use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::invoke;
use anchor_spl::{
    token_2022::spl_token_2022::{
        extension::{
            confidential_transfer::instruction as ct_ix,
            confidential_transfer_fee::instruction as ct_fee_ix,
            ExtensionType,
        },
        state::{AccountState, Mint as MintState},
    },
    token_interface::{
        default_account_state_initialize, initialize_mint2, metadata_pointer_initialize,
        mint_close_authority_initialize, permanent_delegate_initialize,
        token_metadata_initialize, transfer_fee_initialize,
        DefaultAccountStateInitialize, InitializeMint2, MetadataPointerInitialize,
        MintCloseAuthorityInitialize, PermanentDelegateInitialize, TokenInterface,
        TokenMetadataInitialize, TransferFeeInitialize,
    },
};
use spl_token_metadata_interface::state::TokenMetadata;
use spl_type_length_value::variable_len_pack::VariableLenPack;

use crate::{DECIMALS, MAXIMUM_FEE, TOKEN_NAME, TOKEN_SYMBOL, TOKEN_URI, TRANSFER_FEE_BPS};

const TLV_HEADER_LEN: usize = 4;

#[derive(Accounts)]
pub struct InitializeConfidentialMint<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(mut)]
    pub mint: Signer<'info>,

    pub system_program: Program<'info, System>,
    pub token_program: Interface<'info, TokenInterface>,
}

impl<'info> InitializeConfidentialMint<'info> {
    pub fn initialize(
        &mut self,
        withdraw_withheld_authority_elgamal_pubkey: [u8; 32],
    ) -> Result<()> {
        let fixed_extensions: &[ExtensionType] = &[
            ExtensionType::TransferFeeConfig,
            ExtensionType::MintCloseAuthority,
            ExtensionType::DefaultAccountState,
            ExtensionType::MetadataPointer,
            ExtensionType::PermanentDelegate,
            ExtensionType::ConfidentialTransferMint,
            ExtensionType::ConfidentialTransferFeeConfig,
        ];

        let metadata = TokenMetadata {
            update_authority: spl_pod::optional_keys::OptionalNonZeroPubkey::try_from(Some(
                self.payer.key(),
            ))
            .unwrap(),
            mint: self.mint.key(),
            name: TOKEN_NAME.to_string(),
            symbol: TOKEN_SYMBOL.to_string(),
            uri: TOKEN_URI.to_string(),
            additional_metadata: vec![],
        };

        let space =
            ExtensionType::try_calculate_account_len::<MintState>(fixed_extensions)?;
        let lamports =
            Rent::get()?.minimum_balance(space + TLV_HEADER_LEN + metadata.get_packed_len()?);

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

        self.transfer_fee_config()?;
        self.mint_close_config()?;
        self.default_state_config()?;
        self.metadata_pointer_config()?;
        self.permanent_delegate_config()?;
        self.confidential_config()?;
        self.confidential_fee_config(withdraw_withheld_authority_elgamal_pubkey)?;

        initialize_mint2(
            CpiContext::new(
                self.token_program.key(),
                InitializeMint2 {
                    mint: self.mint.to_account_info(),
                },
            ),
            DECIMALS,
            &self.payer.key(),
            Some(&self.payer.key()),
        )?;

        self.token_metadata_config()?;
        Ok(())
    }

    pub fn transfer_fee_config(&self) -> Result<()> {
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
            TRANSFER_FEE_BPS,
            MAXIMUM_FEE,
        )
    }

    pub fn mint_close_config(&self) -> Result<()> {
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

    pub fn default_state_config(&self) -> Result<()> {
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

    pub fn metadata_pointer_config(&self) -> Result<()> {
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

    pub fn permanent_delegate_config(&self) -> Result<()> {
        permanent_delegate_initialize(
            CpiContext::new(
                self.token_program.key(),
                PermanentDelegateInitialize {
                    mint: self.mint.to_account_info(),
                    token_program_id: self.token_program.to_account_info(),
                },
            ),
            &self.payer.key(),
        )
    }

    pub fn confidential_config(&self) -> Result<()> {
        let ix = ct_ix::initialize_mint(
            &self.token_program.key(),
            &self.mint.key(),
            Some(self.payer.key()),
            false,
            None,
        )?;
        invoke(
            &ix,
            &[
                self.mint.to_account_info(),
                self.token_program.to_account_info(),
            ],
        )?;
        Ok(())
    }

    pub fn confidential_fee_config(
        &self,
        withdraw_withheld_authority_elgamal_pubkey: [u8; 32],
    ) -> Result<()> {
        let ix = ct_fee_ix::initialize_confidential_transfer_fee_config(
            &self.token_program.key(),
            &self.mint.key(),
            Some(self.payer.key()),
            &withdraw_withheld_authority_elgamal_pubkey.into(),
        )?;
        invoke(
            &ix,
            &[
                self.mint.to_account_info(),
                self.token_program.to_account_info(),
            ],
        )?;
        Ok(())
    }

    pub fn token_metadata_config(&self) -> Result<()> {
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
            TOKEN_NAME.to_string(),
            TOKEN_SYMBOL.to_string(),
            TOKEN_URI.to_string(),
        )
    }
}
