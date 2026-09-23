mod test_handler;

use {
    anchor_lang::solana_program::instruction::Instruction,
    anchor_spl::token_2022::spl_token_2022::{
        extension::{
            default_account_state::DefaultAccountState,
            metadata_pointer::MetadataPointer,
            mint_close_authority::MintCloseAuthority,
            transfer_fee::TransferFeeConfig,
            BaseStateWithExtensions, ExtensionType, StateWithExtensions,
        },
        state::{AccountState, Mint as MintState},
    },
    litesvm::LiteSVM,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_program_option::COption,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
    spl_token_metadata_interface::state::TokenMetadata,
    spl_type_length_value::variable_len_pack::VariableLenPack,
    test_handler::{initialize, transfer_fee},
};

fn setup() -> (LiteSVM, Keypair) {
    let payer = Keypair::new();

    let mut svm = LiteSVM::new();
    let program_path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/deploy/token22_ct.so");
    assert!(
        program_path.exists(),
        "program binary not found at {}. Run `anchor build` first.",
        program_path.display()
    );
    svm.add_program_from_file(token22_ct::ID, program_path).unwrap();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();

    (svm, payer)
}

fn send(svm: &mut LiteSVM, payer: &Keypair, signers: &[&Keypair], ixs: Vec<Instruction>) {
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&ixs, Some(&payer.pubkey()), &blockhash);
    let mut all = vec![payer];
    all.extend_from_slice(signers);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &all).unwrap();
    if let Err(e) = svm.send_transaction(tx) {
        panic!("transaction failed: {:?}", e);
    }
}

fn opt_bytes(k: spl_pod::optional_keys::OptionalNonZeroPubkey) -> [u8; 32] {
    k.0.to_bytes()
}

fn copt_bytes(k: COption<Pubkey>) -> Option<[u8; 32]> {
    match k {
        COption::Some(p) => Some(p.to_bytes()),
        COption::None => None,
    }
}

#[test]
fn test_initialize() {
    let (mut svm, payer) = setup();
    let mint = initialize::init_mint(&mut svm, &payer);
    assert!(svm.get_account(&mint.pubkey()).is_some());
}

#[test]
fn test_mint_base_state() {
    let (mut svm, payer) = setup();
    let mint = initialize::init_mint(&mut svm, &payer);
    let data = initialize::mint_data(&svm, &mint);
    let state = StateWithExtensions::<MintState>::unpack(&data).unwrap();
    assert_eq!(state.base.decimals, token22_ct::DECIMALS);
    assert!(state.base.is_initialized);
    assert_eq!(state.base.supply, 0);
    assert_eq!(
        copt_bytes(state.base.mint_authority),
        Some(payer.pubkey().to_bytes())
    );
    assert_eq!(
        copt_bytes(state.base.freeze_authority),
        Some(payer.pubkey().to_bytes())
    );
}

#[test]
fn test_extensions() {
    let (mut svm, payer) = setup();
    let mint = initialize::init_mint(&mut svm, &payer);
    let data = initialize::mint_data(&svm, &mint);
    let state = StateWithExtensions::<MintState>::unpack(&data).unwrap();

    let fee = state.get_extension::<TransferFeeConfig>().unwrap();
    assert_eq!(
        u16::from(fee.newer_transfer_fee.transfer_fee_basis_points),
        token22_ct::TRANSFER_FEE_BPS
    );
    assert_eq!(
        u64::from(fee.newer_transfer_fee.maximum_fee),
        token22_ct::MAXIMUM_FEE
    );
    assert_eq!(
        opt_bytes(fee.withdraw_withheld_authority),
        payer.pubkey().to_bytes()
    );
    assert_eq!(fee.calculate_epoch_fee(0, 10_000).unwrap(), 100);

    let default_state = state.get_extension::<DefaultAccountState>().unwrap();
    assert_eq!(default_state.state, AccountState::Frozen as u8);

    let pointer = state.get_extension::<MetadataPointer>().unwrap();
    assert_eq!(opt_bytes(pointer.authority), payer.pubkey().to_bytes());
    assert_eq!(
        opt_bytes(pointer.metadata_address),
        mint.pubkey().to_bytes()
    );

    let close = state.get_extension::<MintCloseAuthority>().unwrap();
    assert_eq!(
        opt_bytes(close.close_authority),
        payer.pubkey().to_bytes()
    );

    let mut types = state.get_extension_types().unwrap();
    let mut expected = vec![
        ExtensionType::TransferFeeConfig,
        ExtensionType::MintCloseAuthority,
        ExtensionType::DefaultAccountState,
        ExtensionType::MetadataPointer,
        ExtensionType::TokenMetadata,
    ];
    types.sort_by_key(|t| *t as u16);
    expected.sort_by_key(|t| *t as u16);
    assert_eq!(types, expected);
}

#[test]
fn test_token_metadata_onchain() {
    let (mut svm, payer) = setup();
    let mint = initialize::init_mint(&mut svm, &payer);
    let data = initialize::mint_data(&svm, &mint);
    let state = StateWithExtensions::<MintState>::unpack(&data).unwrap();
    let metadata = state.get_variable_len_extension::<TokenMetadata>().unwrap();
    assert_eq!(metadata.name, token22_ct::TOKEN_NAME);
    assert_eq!(metadata.symbol, token22_ct::TOKEN_SYMBOL);
    assert_eq!(metadata.uri, token22_ct::TOKEN_URI);
    assert_eq!(metadata.mint.to_bytes(), mint.pubkey().to_bytes());
    assert_eq!(
        opt_bytes(metadata.update_authority),
        payer.pubkey().to_bytes()
    );
}

#[test]
fn test_mint_size_and_rent() {
    let (mut svm, _payer) = setup();
    let mint = initialize::init_mint(&mut svm, &_payer);
    let data = initialize::mint_data(&svm, &mint);
    let base =
        ExtensionType::try_calculate_account_len::<MintState>(&initialize::fixed_extensions())
            .unwrap();
    let expected = base + 4 + initialize::metadata_probe().get_packed_len().unwrap();
    assert_eq!(data.len(), expected);
    let lamports = svm.get_account(&mint.pubkey()).unwrap().lamports;
    assert!(lamports >= svm.minimum_balance_for_rent_exemption(expected));
}

#[test]
fn test_token_metadata_must_not_use_try_calculate() {
    let with_metadata = [
        ExtensionType::TransferFeeConfig,
        ExtensionType::MintCloseAuthority,
        ExtensionType::DefaultAccountState,
        ExtensionType::MetadataPointer,
        ExtensionType::TokenMetadata,
    ];
    assert!(ExtensionType::try_calculate_account_len::<MintState>(&with_metadata).is_err());
}

#[test]
fn test_transfer_with_fee() {
    let (mut svm, payer) = setup();
    let mint = initialize::init_mint(&mut svm, &payer);
    let user = Keypair::new();
    let src = transfer_fee::create_token_account(&mut svm, &payer, &mint, &user);
    let dst = transfer_fee::create_token_account(&mut svm, &payer, &mint, &user);
    transfer_fee::mint_to(&mut svm, &payer, &mint, &src, 50_000);

    let mint_data = initialize::mint_data(&svm, &mint);
    let mint_state = StateWithExtensions::<MintState>::unpack(&mint_data).unwrap();
    let expected_fee = mint_state
        .get_extension::<TransferFeeConfig>()
        .unwrap()
        .calculate_epoch_fee(0, 10_000)
        .unwrap();
    assert_eq!(expected_fee, 100);

    transfer_fee::transfer_via_program(&mut svm, &payer, &user, &src, &dst, &mint, 10_000);

    assert_eq!(transfer_fee::balances(&svm, &src), (40_000, 0));
    assert_eq!(transfer_fee::balances(&svm, &dst), (9_900, 100));
    assert!(transfer_fee::is_thawed(&svm, &src));
    assert!(transfer_fee::is_thawed(&svm, &dst));
}
