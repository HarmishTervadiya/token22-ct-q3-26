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

    pub fn transfer_with_fee(
        ctx: Context<TransferWithFee>,
        amount: u64,
        decimals: u8,
    ) -> Result<()> {
        ctx.accounts.transfer(amount, decimals)
    }

    pub fn unfreeze_kyc_account(ctx: Context<UnfreezeKycAccount>) -> Result<()> {
        ctx.accounts.unfreeze()
    }

    pub fn initialize_confidential_mint(
        ctx: Context<InitializeConfidentialMint>,
        withdraw_withheld_authority_elgamal_pubkey: [u8; 32],
    ) -> Result<()> {
        ctx.accounts
            .initialize(withdraw_withheld_authority_elgamal_pubkey)
    }

    pub fn deposit_confidential(
        ctx: Context<DepositConfidential>,
        amount: u64,
        decimals: u8,
    ) -> Result<()> {
        ctx.accounts.deposit(amount, decimals)
    }

    pub fn apply_pending_balance(
        ctx: Context<ApplyPendingBalance>,
        expected_pending_balance_credit_counter: u64,
        new_decryptable_available_balance: [u8; 36],
    ) -> Result<()> {
        ctx.accounts.apply(
            expected_pending_balance_credit_counter,
            new_decryptable_available_balance,
        )
    }
}
