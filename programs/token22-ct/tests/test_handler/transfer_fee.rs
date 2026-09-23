use {
    anchor_lang::{solana_program::instruction::Instruction, InstructionData, ToAccountMetas},
    anchor_spl::token_2022::spl_token_2022::{
        self,
        extension::{
            transfer_fee::TransferFeeAmount, BaseStateWithExtensions, ExtensionType,
            StateWithExtensions,
        },
        state::{Account as TokenAccount, AccountState},
    },
    litesvm::LiteSVM,
    solana_keypair::Keypair,
    solana_signer::Signer,
};

pub fn create_token_account(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: &Keypair,
    owner: &Keypair,
) -> Keypair {
    let acc = Keypair::new();
    let len = ExtensionType::try_calculate_account_len::<TokenAccount>(&[
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
            spl_token_2022::instruction::thaw_account(
                &spl_token_2022::ID,
                &acc.pubkey(),
                &mint.pubkey(),
                &payer.pubkey(),
                &[],
            )
            .unwrap(),
        ],
    );
    acc
}

pub fn mint_to(svm: &mut LiteSVM, payer: &Keypair, mint: &Keypair, dst: &Keypair, amount: u64) {
    crate::send(
        svm,
        payer,
        &[],
        vec![
            spl_token_2022::instruction::mint_to(
                &spl_token_2022::ID,
                &mint.pubkey(),
                &dst.pubkey(),
                &payer.pubkey(),
                &[],
                amount,
            )
            .unwrap(),
        ],
    );
}

pub fn transfer_via_program(
    svm: &mut LiteSVM,
    payer: &Keypair,
    authority: &Keypair,
    source: &Keypair,
    destination: &Keypair,
    mint: &Keypair,
    amount: u64,
) {
    let ix = Instruction {
        program_id: token22_ct::ID,
        accounts: token22_ct::accounts::TransferWithFee {
            authority: authority.pubkey(),
            source: source.pubkey(),
            destination: destination.pubkey(),
            mint: mint.pubkey(),
            token_program: spl_token_2022::ID,
        }
        .to_account_metas(None),
        data: token22_ct::instruction::TransferWithFee {
            amount,
            decimals: token22_ct::DECIMALS,
        }
        .data(),
    };
    crate::send(svm, payer, &[authority], vec![ix]);
}

pub fn balances(svm: &LiteSVM, account: &Keypair) -> (u64, u64) {
    let data = svm.get_account(&account.pubkey()).unwrap().data.clone();
    let state = StateWithExtensions::<TokenAccount>::unpack(&data).unwrap();
    let withheld = u64::from(
        state
            .get_extension::<TransferFeeAmount>()
            .unwrap()
            .withheld_amount,
    );
    (state.base.amount, withheld)
}

pub fn is_unfrozen(svm: &LiteSVM, account: &Keypair) -> bool {
    let data = svm.get_account(&account.pubkey()).unwrap().data.clone();
    StateWithExtensions::<TokenAccount>::unpack(&data)
        .unwrap()
        .base
        .state
        == AccountState::Initialized
}
