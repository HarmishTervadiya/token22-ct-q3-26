
use {
    anchor_lang::{
        prelude::Pubkey,
        solana_program::{instruction::Instruction, system_program},
        AccountDeserialize, InstructionData, ToAccountMetas,
    },
    litesvm::LiteSVM,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

#[test]
fn test_initialize() {
    let program_id = token22_ct::id();
    let payer = Keypair::new();
    let counter = Pubkey::find_program_address(
        &[token22_ct::constants::COUNTER_SEED],
        &program_id,
    )
    .0;
    let mut svm = LiteSVM::new();
    let bytes = include_bytes!(concat!(
        env!("CARGO_TARGET_TMPDIR"),
        "/../deploy/token22_ct.so"
    ));
    svm.add_program(program_id, bytes).unwrap();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();

  
    // let blockhash = svm.latest_blockhash();
    // let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    // let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();

    // let res = svm.send_transaction(tx);
    // assert!(res.is_ok());

}
