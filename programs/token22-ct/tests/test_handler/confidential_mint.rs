use {
    anchor_lang::{solana_program::instruction::Instruction, InstructionData, ToAccountMetas},
    anchor_spl::token_interface::spl_token_2022::ID as TOKEN_2022_PROGRAM_ID,
    litesvm::LiteSVM,
    solana_keypair::Keypair,
    solana_signer::Signer,
    zk::encryption::elgamal::ElGamalKeypair,
};

pub fn init_confidential_mint(
    svm: &mut LiteSVM,
    payer: &Keypair,
    fee_authority: &ElGamalKeypair,
) -> Keypair {
    let mint = Keypair::new();
    let fee_pubkey: [u8; 32] = fee_authority.pubkey().into();
    let ix = Instruction {
        program_id: token22_ct::ID,
        accounts: token22_ct::accounts::InitializeConfidentialMint {
            payer: payer.pubkey(),
            mint: mint.pubkey(),
            system_program: anchor_lang::solana_program::system_program::ID,
            token_program: TOKEN_2022_PROGRAM_ID,
        }
        .to_account_metas(None),
        data: token22_ct::instruction::InitializeConfidentialMint {
            withdraw_withheld_authority_elgamal_pubkey: fee_pubkey,
        }
        .data(),
    };
    crate::send(svm, payer, &[&mint], vec![ix]);
    mint
}
