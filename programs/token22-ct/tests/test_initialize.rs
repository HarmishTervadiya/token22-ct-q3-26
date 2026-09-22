// Tests for `initialize`. The SVM tests load `target/deploy/token22_ct.so`
// at runtime, so run `anchor build` before `cargo test`.
use {
    anchor_lang::{
        solana_program::{instruction::Instruction, system_program},
        InstructionData, ToAccountMetas,
    },
    anchor_spl::token_2022::spl_token_2022::{
        self,
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
};

fn fixed_extensions() -> [ExtensionType; 4] {
    [
        ExtensionType::TransferFeeConfig,
        ExtensionType::MintCloseAuthority,
        ExtensionType::DefaultAccountState,
        ExtensionType::MetadataPointer,
    ]
}

fn metadata_probe() -> TokenMetadata {
    // Packed length depends only on the string lengths, so this matches the
    // program's metadata exactly.
    TokenMetadata {
        update_authority: spl_pod::optional_keys::OptionalNonZeroPubkey::default(),
        mint: spl_token_2022::ID,
        name: token22_ct::TOKEN_NAME.to_string(),
        symbol: token22_ct::TOKEN_SYMBOL.to_string(),
        uri: token22_ct::TOKEN_URI.to_string(),
        additional_metadata: vec![],
    }
}

fn setup() -> (LiteSVM, Keypair, Keypair) {
    let payer = Keypair::new();
    let mint = Keypair::new();

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

    (svm, payer, mint)
}

/// Compare raw bytes to stay independent of Pubkey crate versions.
fn opt_bytes(k: spl_pod::optional_keys::OptionalNonZeroPubkey) -> [u8; 32] {
    k.0.to_bytes()
}

fn copt_bytes(k: COption<Pubkey>) -> Option<[u8; 32]> {
    match k {
        COption::Some(p) => Some(p.to_bytes()),
        COption::None => None,
    }
}

fn send_initialize(svm: &mut LiteSVM, payer: &Keypair, mint: &Keypair) {
    let instruction = Instruction {
        program_id: token22_ct::ID,
        accounts: token22_ct::accounts::InitializeMint {
            mint: mint.pubkey(),
            payer: payer.pubkey(),
            system_program: system_program::ID,
            token_program: spl_token_2022::ID,
        }
        .to_account_metas(None),
        data: token22_ct::instruction::Initialize {}.data(),
    };

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx =
        VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[payer, mint]).unwrap();

    if let Err(e) = svm.send_transaction(tx) {
        panic!("initialize transaction failed: {:?}", e);
    }
}

/// Initializes a mint and returns the raw on-chain account data. All reads
/// below go through StateWithExtensions, never raw unpack.
fn initialized_mint_data() -> (LiteSVM, Keypair, Keypair, Vec<u8>) {
    let (mut svm, payer, mint) = setup();
    send_initialize(&mut svm, &payer, &mint);
    let account = svm
        .get_account(&mint.pubkey())
        .expect("mint account must exist after initialize");
    assert_eq!(
        account.owner.to_bytes(),
        spl_token_2022::ID.to_bytes(),
        "mint owner must be the Token-2022 program"
    );
    let data = account.data.clone();
    (svm, payer, mint, data)
}

#[test]
fn test_initialize() {
    let (mut svm, payer, mint) = setup();
    send_initialize(&mut svm, &payer, &mint);
    assert!(svm.get_account(&mint.pubkey()).is_some());
}

#[test]
fn test_mint_base_state() {
    let (_svm, payer, _mint, data) = initialized_mint_data();
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
    let (_svm, payer, mint, data) = initialized_mint_data();
    let state = StateWithExtensions::<MintState>::unpack(&data).unwrap();

    // Transfer fee: 100 bps into newer_transfer_fee, live epoch math.
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

    // New accounts default to Frozen.
    let default_state = state.get_extension::<DefaultAccountState>().unwrap();
    assert_eq!(default_state.state, AccountState::Frozen as u8);

    // Metadata pointer aims at the mint itself.
    let pointer = state.get_extension::<MetadataPointer>().unwrap();
    assert_eq!(opt_bytes(pointer.authority), payer.pubkey().to_bytes());
    assert_eq!(
        opt_bytes(pointer.metadata_address),
        mint.pubkey().to_bytes()
    );

    // Mint close authority is set.
    let close = state.get_extension::<MintCloseAuthority>().unwrap();
    assert_eq!(
        opt_bytes(close.close_authority),
        payer.pubkey().to_bytes()
    );

    // Exactly the expected set, nothing more.
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
    let (_svm, payer, mint, data) = initialized_mint_data();
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
    let (svm, _payer, mint, data) = initialized_mint_data();
    // Exact fixed size at mint-init, then metadata init grows the account by
    // the 4-byte TLV header plus the packed metadata.
    let base = ExtensionType::try_calculate_account_len::<MintState>(&fixed_extensions()).unwrap();
    let expected = base + 4 + metadata_probe().get_packed_len().unwrap();
    assert_eq!(data.len(), expected);
    let lamports = svm.get_account(&mint.pubkey()).unwrap().lamports;
    assert!(lamports >= svm.minimum_balance_for_rent_exemption(expected));
}

/// Host-native regression test: TokenMetadata is variable-length, so it must
/// never go through try_calculate_account_len (that was the original
/// InvalidArgument bug). No .so needed.
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
