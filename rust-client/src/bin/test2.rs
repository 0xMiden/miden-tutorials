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
    let faucet_id = AccountId::from_hex("0xde0e03cdc76a7720000eb598fbc0a3").unwrap();
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



    Ok(())
}