use {
    anchor_lang::{
        prelude::Pubkey,
        solana_program::instruction::Instruction,
        InstructionData, ToAccountMetas,
    },
    anchor_spl::token_interface::spl_token_2022::ID as TOKEN_2022_PROGRAM_ID,
    bytemuck::Pod,
    litesvm::LiteSVM,
    solana_compute_budget_interface::ComputeBudgetInstruction,
    solana_keypair::Keypair,
    solana_signer::Signer,
    solana_system_interface::instruction as system_ix,
    t22new::{
        extension::{
            confidential_transfer::{
                instruction as ct_ix, ConfidentialTransferAccount, DecryptableBalance,
            },
            BaseStateWithExtensions, ExtensionType, StateWithExtensions,
        },
        state::Account as TokenAccount,
    },
    zk::{
        encryption::{
            auth_encryption::AeKey,
            derivation::derive_confidential_keys,
            elgamal::{ElGamalCiphertext, ElGamalKeypair, ElGamalPubkey},
        },
        zk_elgamal_proof_program::pubkey_validity::build_pubkey_validity_proof_data,
    },
    zkif::{
        instruction::{close_context_state, ContextStateInfo, ProofInstruction},
        proof_data::ZkProofData,
        state::ProofContextState,
        ID as ZK_PROGRAM_ID,
    },
    proofext::instruction::ProofLocation,
};

pub struct Holder {
    pub account: Pubkey,
    pub elgamal: ElGamalKeypair,
    pub aes: AeKey,
}

pub fn create_and_configure(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: &Keypair,
    owner: &Keypair,
) -> Holder {
    let acc = Keypair::new();
    let space = ExtensionType::try_calculate_account_len::<TokenAccount>(&[
        ExtensionType::ImmutableOwner,
        ExtensionType::TransferFeeAmount,
        ExtensionType::ConfidentialTransferAccount,
        ExtensionType::ConfidentialTransferFeeAmount,
    ])
    .unwrap();
    let lamports = svm.minimum_balance_for_rent_exemption(space);
    crate::send(
        svm,
        payer,
        &[&acc],
        vec![
            system_ix::create_account(
                &payer.pubkey(),
                &acc.pubkey(),
                lamports,
                space as u64,
                &TOKEN_2022_PROGRAM_ID,
            ),
            t22new::instruction::initialize_account3(
                &TOKEN_2022_PROGRAM_ID,
                &acc.pubkey(),
                &mint.pubkey(),
                &owner.pubkey(),
            )
            .unwrap(),
        ],
    );
    crate::send(
        svm,
        payer,
        &[],
        vec![
            t22new::instruction::thaw_account(
                &TOKEN_2022_PROGRAM_ID,
                &acc.pubkey(),
                &mint.pubkey(),
                &payer.pubkey(),
                &[],
            )
            .unwrap(),
        ],
    );

    let (elgamal, aes) = derive_confidential_keys(owner, b"").unwrap();
    let proof = build_pubkey_validity_proof_data(&elgamal).unwrap();
    let ixs = ct_ix::configure_account(
        &TOKEN_2022_PROGRAM_ID,
        &acc.pubkey(),
        &mint.pubkey(),
        &aes.encrypt(0).into(),
        65536,
        &owner.pubkey(),
        &[],
        proofext::instruction::ProofLocation::InstructionOffset(
            std::num::NonZeroI8::new(1).unwrap(),
            &proof,
        ),
    )
    .unwrap();
    crate::send(svm, payer, &[owner], ixs);
    crate::send(
        svm,
        payer,
        &[],
        vec![
            ct_ix::approve_account(
                &TOKEN_2022_PROGRAM_ID,
                &acc.pubkey(),
                &mint.pubkey(),
                &payer.pubkey(),
                &[],
            )
            .unwrap(),
        ],
    );

    Holder {
        account: acc.pubkey(),
        elgamal,
        aes,
    }
}

pub fn read_ct(svm: &LiteSVM, account: &Pubkey) -> ConfidentialTransferAccount {
    let data = svm.get_account(account).unwrap().data.clone();
    *StateWithExtensions::<TokenAccount>::unpack(&data)
        .unwrap()
        .get_extension::<ConfidentialTransferAccount>()
        .unwrap()
}

pub fn available_balance(ct: &ConfidentialTransferAccount, elgamal: &ElGamalKeypair) -> u64 {
    let ciphertext: ElGamalCiphertext = ct.available_balance.try_into().unwrap();
    elgamal.secret().decrypt_u32(&ciphertext).unwrap()
}

pub fn pending_balance(ct: &ConfidentialTransferAccount, elgamal: &ElGamalKeypair) -> u64 {
    let lo: ElGamalCiphertext = ct.pending_balance_lo.try_into().unwrap();
    let hi: ElGamalCiphertext = ct.pending_balance_hi.try_into().unwrap();
    let lo = elgamal.secret().decrypt_u32(&lo).unwrap();
    let hi = elgamal.secret().decrypt_u32(&hi).unwrap();
    lo + (hi << 16)
}

pub fn public_balance(svm: &LiteSVM, account: &Pubkey) -> u64 {
    let data = svm.get_account(account).unwrap().data.clone();
    StateWithExtensions::<TokenAccount>::unpack(&data)
        .unwrap()
        .base
        .amount
}

pub fn fund(svm: &mut LiteSVM, payer: &Keypair, mint: &Keypair, dst: &Pubkey, amount: u64) {
    crate::send(
        svm,
        payer,
        &[],
        vec![
            t22new::instruction::mint_to(
                &TOKEN_2022_PROGRAM_ID,
                &mint.pubkey(),
                dst,
                &payer.pubkey(),
                &[],
                amount,
            )
            .unwrap(),
        ],
    );
}

pub fn deposit_via_program(
    svm: &mut LiteSVM,
    payer: &Keypair,
    holder: &Holder,
    mint: &Keypair,
    owner: &Keypair,
    amount: u64,
) {
    let ix = Instruction {
        program_id: token22_ct::ID,
        accounts: token22_ct::accounts::DepositConfidential {
            token_account: holder.account,
            mint: mint.pubkey(),
            authority: owner.pubkey(),
            token_program: TOKEN_2022_PROGRAM_ID,
        }
        .to_account_metas(None),
        data: token22_ct::instruction::DepositConfidential {
            amount,
            decimals: token22_ct::DECIMALS,
        }
        .data(),
    };
    crate::send(svm, payer, &[owner], vec![ix]);
}

pub fn apply_via_program(
    svm: &mut LiteSVM,
    payer: &Keypair,
    holder: &Holder,
    owner: &Keypair,
) -> u64 {
    let ct = read_ct(svm, &holder.account);
    let counter: u64 = ct.pending_balance_credit_counter.into();
    let new_available =
        available_balance(&ct, &holder.elgamal) + pending_balance(&ct, &holder.elgamal);
    let bytes: [u8; 36] = holder.aes.encrypt(new_available).to_bytes();
    let ix = Instruction {
        program_id: token22_ct::ID,
        accounts: token22_ct::accounts::ApplyPendingBalance {
            token_account: holder.account,
            authority: owner.pubkey(),
            token_program: TOKEN_2022_PROGRAM_ID,
        }
        .to_account_metas(None),
        data: token22_ct::instruction::ApplyPendingBalance {
            expected_pending_balance_credit_counter: counter,
            new_decryptable_available_balance: bytes,
        }
        .data(),
    };
    crate::send(svm, payer, &[owner], vec![ix]);
    new_available
}

pub fn stage_proof<T, U>(
    svm: &mut LiteSVM,
    payer: &Keypair,
    kind: ProofInstruction,
    proof: &T,
) -> Pubkey
where
    T: Pod + ZkProofData<U>,
    U: Pod,
{
    let len = std::mem::size_of::<ProofContextState<U>>();
    let ctx = Keypair::new();
    let lamports = svm.minimum_balance_for_rent_exemption(len);
    crate::send(
        svm,
        payer,
        &[&ctx],
        vec![system_ix::create_account(
            &payer.pubkey(),
            &ctx.pubkey(),
                lamports,
                len as u64,
                &ZK_PROGRAM_ID,
            ),
        ],
    );
    let ix = kind.encode_verify_proof(
        Some(ContextStateInfo {
            context_state_account: &ctx.pubkey(),
            context_state_authority: &payer.pubkey(),
        }),
        proof,
    );
    crate::send(
        svm,
        payer,
        &[],
        vec![
            ComputeBudgetInstruction::set_compute_unit_limit(400_000),
            ix,
        ],
    );
    ctx.pubkey()
}

pub fn close_contexts(svm: &mut LiteSVM, payer: &Keypair, contexts: &[Pubkey]) {
    let ixs: Vec<Instruction> = contexts
        .iter()
        .map(|c| {
            close_context_state(
                ContextStateInfo {
                    context_state_account: c,
                    context_state_authority: &payer.pubkey(),
                },
                &payer.pubkey(),
            )
        })
        .collect();
    crate::send(svm, payer, &[], ixs);
}

pub fn confidential_transfer_with_fee(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: &Keypair,
    alice: &Holder,
    alice_owner: &Keypair,
    bob: &Holder,
    fee_authority: &ElGamalKeypair,
    amount: u64,
) {
    let ct = read_ct(svm, &alice.account);
    let available = available_balance(&ct, &alice.elgamal);
    let current_available: ElGamalCiphertext = ct.available_balance.try_into().unwrap();
    let current_decryptable = ct.decryptable_available_balance.try_into().unwrap();
    let bob_pubkey: ElGamalPubkey = read_ct(svm, &bob.account).elgamal_pubkey.try_into().unwrap();

    let proofs = proofgen::transfer_with_fee::transfer_with_fee_split_proof_data(
        &current_available,
        &current_decryptable,
        amount,
        &alice.elgamal,
        &alice.aes,
        &bob_pubkey,
        None,
        fee_authority.pubkey(),
        token22_ct::TRANSFER_FEE_BPS,
        token22_ct::MAXIMUM_FEE,
    )
    .unwrap();

    let eq_ctx = stage_proof(
        svm,
        payer,
        ProofInstruction::VerifyCiphertextCommitmentEquality,
        &proofs.equality_proof_data,
    );
    let val_ctx = stage_proof(
        svm,
        payer,
        ProofInstruction::VerifyBatchedGroupedCiphertext3HandlesValidity,
        &proofs
            .transfer_amount_ciphertext_validity_proof_data_with_ciphertext
            .proof_data,
    );
    let pct_ctx = stage_proof(
        svm,
        payer,
        ProofInstruction::VerifyPercentageWithCap,
        &proofs.percentage_with_cap_proof_data,
    );
    let fee_val_ctx = stage_proof(
        svm,
        payer,
        ProofInstruction::VerifyBatchedGroupedCiphertext2HandlesValidity,
        &proofs.fee_ciphertext_validity_proof_data,
    );
    let range_ctx = stage_proof(
        svm,
        payer,
        ProofInstruction::VerifyBatchedRangeProofU256,
        &proofs.range_proof_data,
    );

    let new_decryptable: DecryptableBalance = alice.aes.encrypt(available - amount).into();
    let ixs = ct_ix::transfer_with_fee(
        &TOKEN_2022_PROGRAM_ID,
        &alice.account,
        &mint.pubkey(),
        &bob.account,
        &new_decryptable,
        &proofs
            .transfer_amount_ciphertext_validity_proof_data_with_ciphertext
            .ciphertext_lo,
        &proofs
            .transfer_amount_ciphertext_validity_proof_data_with_ciphertext
            .ciphertext_hi,
        &alice_owner.pubkey(),
        &[],
        ProofLocation::ContextStateAccount(&eq_ctx),
        ProofLocation::ContextStateAccount(&val_ctx),
        ProofLocation::ContextStateAccount(&pct_ctx),
        ProofLocation::ContextStateAccount(&fee_val_ctx),
        ProofLocation::ContextStateAccount(&range_ctx),
    )
    .unwrap();
    crate::send(svm, payer, &[alice_owner], ixs);
    close_contexts(
        svm,
        payer,
        &[eq_ctx, val_ctx, pct_ctx, fee_val_ctx, range_ctx],
    );
}

pub fn confidential_withdraw(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: &Keypair,
    holder: &Holder,
    owner: &Keypair,
    amount: u64,
) {
    let ct = read_ct(svm, &holder.account);
    let available = available_balance(&ct, &holder.elgamal);
    let current: ElGamalCiphertext = ct.available_balance.try_into().unwrap();

    let proofs =
        proofgen::withdraw::withdraw_proof_data(&current, available, amount, &holder.elgamal)
            .unwrap();

    let eq_ctx = stage_proof(
        svm,
        payer,
        ProofInstruction::VerifyCiphertextCommitmentEquality,
        &proofs.equality_proof_data,
    );
    let range_ctx = stage_proof(
        svm,
        payer,
        ProofInstruction::VerifyBatchedRangeProofU64,
        &proofs.range_proof_data,
    );

    let new_decryptable: DecryptableBalance = holder.aes.encrypt(available - amount).into();
    let ixs = ct_ix::withdraw(
        &TOKEN_2022_PROGRAM_ID,
        &holder.account,
        &mint.pubkey(),
        amount,
        token22_ct::DECIMALS,
        &new_decryptable,
        &owner.pubkey(),
        &[],
        ProofLocation::ContextStateAccount(&eq_ctx),
        ProofLocation::ContextStateAccount(&range_ctx),
    )
    .unwrap();
    crate::send(svm, payer, &[owner], ixs);
    close_contexts(svm, payer, &[eq_ctx, range_ctx]);
}
