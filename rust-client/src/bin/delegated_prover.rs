use std::sync::Arc;

use miden_client::account::AccountId;
use miden_client::crypto::FeltRng;
use miden_client::{
    asset::FungibleAsset,
    keystore::FilesystemKeyStore,
    note::NoteType,
    rpc::Endpoint,
    transaction::{OutputNote, TransactionProver, TransactionRequestBuilder},
    ClientError, Felt, RemoteTransactionProver,
};
use miden_client_tools::{
    create_basic_account, create_exact_p2id_note, instantiate_client, mint_from_faucet_for_account,
};

#[tokio::main]
async fn main() -> Result<(), ClientError> {
    // Initialize client, keystore, & delegated prover endpoint
    let endpoint = Endpoint::testnet();
    let mut client = instantiate_client(endpoint, None).await.unwrap();

    let keystore: FilesystemKeyStore<rand::prelude::StdRng> =
        FilesystemKeyStore::new("./keystore".into()).unwrap();

    let remote_tx_prover: RemoteTransactionProver =
        RemoteTransactionProver::new("https://tx-prover.testnet.miden.io");
    let tx_prover: Arc<dyn TransactionProver + 'static> = Arc::new(remote_tx_prover);

    let sync_summary = client.sync_state().await.unwrap();
    println!("Latest block: {}", sync_summary.block_num);

    let (alice_account, _) = create_basic_account(&mut client, keystore.clone())
        .await
        .unwrap();

    let (bob_account, _) = create_basic_account(&mut client, keystore.clone())
        .await
        .unwrap();

    // import public faucet id
    let (_, faucet_id) = AccountId::from_bech32("mtst1qq932n3zkt5rxgpw5tgd9szxp58tllml").unwrap();
    client.import_account_by_id(faucet_id).await.unwrap();
    let binding = client.get_account(faucet_id).await.unwrap().unwrap();
    let faucet = binding.account();

    let _ = mint_from_faucet_for_account(&mut client, &alice_account, &faucet, 1000, None)
        .await
        .unwrap();

    let account = client
        .get_account(alice_account.id())
        .await
        .unwrap()
        .unwrap();

    println!(
        "Alice initial account balance: {:?}",
        account.account().vault().get_balance(faucet.id())
    );

    // Creating 10 separate P2ID notes with 10 tokens each to send to Bob
    let send_amount = 10;
    let fungible_asset = FungibleAsset::new(faucet.id(), send_amount).unwrap();
    let mut p2id_notes = vec![];
    for _ in 0..=9 {
        let p2id_note = create_exact_p2id_note(
            alice_account.id(),
            bob_account.id(),
            vec![fungible_asset.into()],
            NoteType::Public,
            Felt::new(0),
            client.rng().draw_word(),
        )?;
        p2id_notes.push(p2id_note);
    }

    // Specifying output notes and creating a tx request to create them
    let output_notes: Vec<OutputNote> = p2id_notes.into_iter().map(OutputNote::Full).collect();
    let transaction_request = TransactionRequestBuilder::new()
        .own_output_notes(output_notes)
        .build()
        .unwrap();
    let tx_execution_result = client
        .new_transaction(alice_account.id(), transaction_request)
        .await?;

    // Using the `submit_transaction_with_prover` function
    // to offload proof generation to the delegated prover
    client
        .submit_transaction_with_prover(tx_execution_result, tx_prover.clone())
        .await
        .unwrap();

    client.sync_state().await.unwrap();

    let account = client
        .get_account(alice_account.id())
        .await
        .unwrap()
        .unwrap();

    println!(
        "Alice final account balance: {:?}",
        account.account().vault().get_balance(faucet.id())
    );

    Ok(())
}
