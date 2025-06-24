use miden_client::{keystore::FilesystemKeyStore, rpc::Endpoint, ClientError};
use miden_client_tools::{
    create_basic_account, create_basic_faucet, delete_keystore_and_store, instantiate_client,
    mint_from_faucet_for_account,
};
use miden_objects::account::NetworkId;
#[tokio::main]
async fn main() -> Result<(), ClientError> {
    delete_keystore_and_store(None).await;

    // Initialize client, keystore, & delegated prover endpoint
    let endpoint = Endpoint::testnet();
    let mut client = instantiate_client(endpoint, None).await.unwrap();

    let keystore: FilesystemKeyStore<rand::prelude::StdRng> =
        FilesystemKeyStore::new("./keystore".into()).unwrap();

    let sync_summary = client.sync_state().await.unwrap();
    println!("Latest block: {}", sync_summary.block_num);

    let faucet = create_basic_faucet(&mut client, keystore.clone())
        .await
        .unwrap();
    println!("faucetId: {:?}", faucet.id().to_bech32(NetworkId::Testnet));

    // mint to publish faucet on chain
    let (alice_account, _) = create_basic_account(&mut client, keystore.clone())
        .await
        .unwrap();

    let _ = mint_from_faucet_for_account(&mut client, &alice_account, &faucet, 1, None)
        .await
        .unwrap();

    println!("deployed");

    Ok(())
}
