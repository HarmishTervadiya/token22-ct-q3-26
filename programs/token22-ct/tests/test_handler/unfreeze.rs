use {
    anchor_lang::{solana_program::instruction::Instruction, InstructionData, ToAccountMetas},
    anchor_spl::token_2022::spl_token_2022::{
        self,
        extension::ExtensionType,
    },
    litesvm::LiteSVM,
    solana_keypair::Keypair,
    solana_signer::Signer,
};

pub fn create_frozen_account(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: &Keypair,
    owner: &Keypair,
) -> Keypair {
    let acc = Keypair::new();
    let len = ExtensionType::try_calculate_account_len::<
        anchor_spl::token_2022::spl_token_2022::state::Account,
    >(&[
        ExtensionType::ImmutableOwner,
        ExtensionType::TransferFeeAmount,
    ])
    .unwrap();
    let lamports = svm.minimum_balance_for_rent_exemption(len);
    crate::send(
        svm,
        payer,
        &[&acc],
        vec![
            anchor_lang::solana_program::system_instruction::create_account(
                &payer.pubkey(),
                &acc.pubkey(),
                lamports,
                len as u64,
                &spl_token_2022::ID,
            ),
            spl_token_2022::instruction::initialize_account3(
                &spl_token_2022::ID,
                &acc.pubkey(),
                &mint.pubkey(),
                &owner.pubkey(),
            )
            .unwrap(),
        ],
    );
    acc
}

pub fn unfreeze_via_program(
    svm: &mut LiteSVM,
    payer: &Keypair,
    freeze_authority: &Keypair,
    token_account: &Keypair,
    mint: &Keypair,
) {
    let ix = Instruction {
        program_id: token22_ct::ID,
        accounts: token22_ct::accounts::UnfreezeKycAccount {
            freeze_authority: freeze_authority.pubkey(),
            token_account: token_account.pubkey(),
            mint: mint.pubkey(),
            token_program: spl_token_2022::ID,
        }
        .to_account_metas(None),
        data: token22_ct::instruction::UnfreezeKycAccount {}.data(),
    };
    crate::send(svm, payer, &[freeze_authority], vec![ix]);
}
