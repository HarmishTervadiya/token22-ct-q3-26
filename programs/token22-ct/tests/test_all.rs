mod test_handler;

use {
    anchor_lang::solana_program::instruction::Instruction,
    litesvm::LiteSVM,
    solana_keypair::Keypair,
    solana_message::Message,
    solana_signer::Signer,
    solana_transaction::Transaction,
    token_metadata_1::state::TokenMetadata,
    spl_type_length_value::variable_len_pack::VariableLenPack,
    t22new::{
        extension::{
            confidential_transfer::ConfidentialTransferMint,
            default_account_state::DefaultAccountState,
            metadata_pointer::MetadataPointer,
            mint_close_authority::MintCloseAuthority,
            permanent_delegate::PermanentDelegate,
            transfer_fee::TransferFeeConfig,
            BaseStateWithExtensions, ExtensionType, StateWithExtensions,
        },
        state::{AccountState, Mint as MintState},
    },
    test_handler::{confidential, confidential_mint, initialize, transfer_fee, unfreeze},
    zk::encryption::elgamal::ElGamalKeypair,
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
    svm.airdrop(&payer.pubkey(), 100_000_000_000).unwrap();

    (svm, payer)
}

fn send(svm: &mut LiteSVM, payer: &Keypair, signers: &[&Keypair], ixs: Vec<Instruction>) {
    let mut all = vec![payer];
    for s in signers {
        if !all.iter().any(|k| k.pubkey() == s.pubkey()) {
            all.push(*s);
        }
    }
    let blockhash = svm.latest_blockhash();
    let mut tx = Transaction::new_unsigned(Message::new(&ixs, Some(&payer.pubkey())));
    tx.try_sign(&all, blockhash).unwrap();
    if let Err(e) = svm.send_transaction(tx) {
        panic!("transaction failed: {:?}", e);
    }
}

fn authority_is(opt: impl std::fmt::Debug, key: &Keypair) -> bool {
    format!("{:?}", opt).contains(&key.pubkey().to_string())
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
    assert!(authority_is(state.base.mint_authority, &payer));
    assert!(authority_is(state.base.freeze_authority, &payer));
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
    assert!(authority_is(fee.withdraw_withheld_authority, &payer));

    let default_state = state.get_extension::<DefaultAccountState>().unwrap();
    assert_eq!(default_state.state, AccountState::Frozen as u8);

    let pointer = state.get_extension::<MetadataPointer>().unwrap();
    assert!(authority_is(pointer.authority, &payer));
    assert!(authority_is(pointer.metadata_address, &mint));

    let close = state.get_extension::<MintCloseAuthority>().unwrap();
    assert!(authority_is(close.close_authority, &payer));

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
    assert!(authority_is(metadata.update_authority, &payer));
}

#[test]
fn test_mint_size_and_rent() {
    let (mut svm, _payer) = setup();
    let mint = initialize::init_mint(&mut svm, &_payer);
    let data = initialize::mint_data(&svm, &mint);
    let base = ExtensionType::try_calculate_account_len::<MintState>(&[
        ExtensionType::TransferFeeConfig,
        ExtensionType::MintCloseAuthority,
        ExtensionType::DefaultAccountState,
        ExtensionType::MetadataPointer,
    ])
    .unwrap();
    let probe = TokenMetadata {
        update_authority: Default::default(),
        mint: Default::default(),
        name: token22_ct::TOKEN_NAME.to_string(),
        symbol: token22_ct::TOKEN_SYMBOL.to_string(),
        uri: token22_ct::TOKEN_URI.to_string(),
        additional_metadata: vec![],
    };
    let expected = base + 4 + probe.get_packed_len().unwrap();
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

    transfer_fee::transfer_via_program(&mut svm, &payer, &user, &src, &dst, &mint, 10_000);

    assert_eq!(transfer_fee::balances(&svm, &src), (40_000, 0));
    assert_eq!(transfer_fee::balances(&svm, &dst), (9_900, 100));
    assert!(transfer_fee::is_unfrozen(&svm, &src));
    assert!(transfer_fee::is_unfrozen(&svm, &dst));
}

#[test]
fn test_unfreeze_after_kyc() {
    let (mut svm, payer) = setup();
    let mint = initialize::init_mint(&mut svm, &payer);
    let user = Keypair::new();
    let acc = unfreeze::create_frozen_account(&mut svm, &payer, &mint, &user);
    assert!(!transfer_fee::is_unfrozen(&svm, &acc));

    unfreeze::unfreeze_via_program(&mut svm, &payer, &payer, &acc, &mint);
    assert!(transfer_fee::is_unfrozen(&svm, &acc));

    let data = initialize::mint_data(&svm, &mint);
    let state = StateWithExtensions::<MintState>::unpack(&data).unwrap();
    let default_state = state.get_extension::<DefaultAccountState>().unwrap();
    assert_eq!(default_state.state, AccountState::Frozen as u8);
}

#[test]
fn test_confidential_mint() {
    let (mut svm, payer) = setup();
    let fee_authority = ElGamalKeypair::new_rand();
    let mint = confidential_mint::init_confidential_mint(&mut svm, &payer, &fee_authority);
    let data = initialize::mint_data(&svm, &mint);
    let state = StateWithExtensions::<MintState>::unpack(&data).unwrap();

    let mut types = state.get_extension_types().unwrap();
    let mut expected = vec![
        ExtensionType::TransferFeeConfig,
        ExtensionType::MintCloseAuthority,
        ExtensionType::DefaultAccountState,
        ExtensionType::MetadataPointer,
        ExtensionType::PermanentDelegate,
        ExtensionType::ConfidentialTransferMint,
        ExtensionType::ConfidentialTransferFeeConfig,
        ExtensionType::TokenMetadata,
    ];
    types.sort_by_key(|t| *t as u16);
    expected.sort_by_key(|t| *t as u16);
    assert_eq!(types, expected);

    let delegate = state.get_extension::<PermanentDelegate>().unwrap();
    assert!(authority_is(delegate.delegate, &payer));

    let ct = state.get_extension::<ConfidentialTransferMint>().unwrap();
    assert!(authority_is(ct.authority, &payer));
    assert!(!bool::from(ct.auto_approve_new_accounts));

    let fee_config = state
        .get_extension::<
            t22new::extension::confidential_transfer_fee::ConfidentialTransferFeeConfig,
        >()
        .unwrap();
    let fee_key: [u8; 32] = fee_authority.pubkey().into();
    assert_eq!(
        fee_config.withdraw_withheld_authority_elgamal_pubkey.0,
        fee_key
    );
}

#[test]
fn test_confidential_lifecycle() {
    let (mut svm, payer) = setup();
    let fee_authority = ElGamalKeypair::new_rand();
    let mint = confidential_mint::init_confidential_mint(&mut svm, &payer, &fee_authority);
    let alice_owner = Keypair::new();
    let bob_owner = Keypair::new();
    let alice = confidential::create_and_configure(&mut svm, &payer, &mint, &alice_owner);
    let bob = confidential::create_and_configure(&mut svm, &payer, &mint, &bob_owner);

    confidential::fund(&mut svm, &payer, &mint, &alice.account, 10_000);
    confidential::deposit_via_program(&mut svm, &payer, &alice, &mint, &alice_owner, 10_000);

    let ct = confidential::read_ct(&svm, &alice.account);
    assert_eq!(confidential::pending_balance(&ct, &alice.elgamal), 10_000);
    assert_eq!(confidential::available_balance(&ct, &alice.elgamal), 0);

    assert_eq!(
        confidential::apply_via_program(&mut svm, &payer, &alice, &alice_owner),
        10_000
    );

    confidential::confidential_transfer_with_fee(
        &mut svm,
        &payer,
        &mint,
        &alice,
        &alice_owner,
        &bob,
        &fee_authority,
        2_500,
    );

    let bob_ct = confidential::read_ct(&svm, &bob.account);
    assert_eq!(confidential::pending_balance(&bob_ct, &bob.elgamal), 2_475);
    assert_eq!(confidential::available_balance(&bob_ct, &bob.elgamal), 0);

    assert_eq!(
        confidential::apply_via_program(&mut svm, &payer, &bob, &bob_owner),
        2_475
    );

    confidential::confidential_withdraw(&mut svm, &payer, &mint, &bob, &bob_owner, 1_000);
    assert_eq!(confidential::public_balance(&svm, &bob.account), 1_000);
    let bob_final = confidential::read_ct(&svm, &bob.account);
    assert_eq!(
        confidential::available_balance(&bob_final, &bob.elgamal),
        1_475
    );
}
