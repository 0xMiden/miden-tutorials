use miden_client::auth::AuthSecretKey;
use miden_lib::account::auth::AuthRpoFalcon512;
use rand::{rngs::StdRng, RngCore};
use std::sync::Arc;

use miden_client::{
    account::component::BasicWallet,
    builder::ClientBuilder,
    keystore::FilesystemKeyStore,
    rpc::{Endpoint, GrpcClient},
    transaction::{TransactionProver, TransactionRequestBuilder},
    ClientError, RemoteTransactionProver,
};
use miden_client_sqlite_store::ClientBuilderSqliteExt;
use miden_objects::account::{AccountBuilder, AccountStorageMode, AccountType};

#[tokio::main]
async fn main() -> Result<(), ClientError> {
    // Initialize client
    let endpoint = Endpoint::testnet();
    let timeout_ms = 10_000;
    let rpc_client = Arc::new(GrpcClient::new(&endpoint, timeout_ms));

    // Initialize keystore
    let keystore_path = std::path::PathBuf::from("./keystore");
    let keystore = Arc::new(FilesystemKeyStore::<StdRng>::new(keystore_path).unwrap());

    let store_path = std::path::PathBuf::from("./store.sqlite3");

    let mut client = ClientBuilder::new()
        .rpc(rpc_client)
        .sqlite_store(store_path)
        .authenticator(keystore.clone())
        .in_debug_mode(true.into())
        .build()
        .await?;

    let sync_summary = client.sync_state().await.unwrap();
    println!("Latest block: {}", sync_summary.block_num);

    // Create Alice's account
    let mut init_seed = [0_u8; 32];
    client.rng().fill_bytes(&mut init_seed);

    let key_pair = AuthSecretKey::new_rpo_falcon512();

    let alice_account = AccountBuilder::new(init_seed)
        .account_type(AccountType::RegularAccountImmutableCode)
        .storage_mode(AccountStorageMode::Private)
        .with_auth_component(AuthRpoFalcon512::new(key_pair.public_key().to_commitment()))
        .with_component(BasicWallet)
        .build()
        .unwrap();

    client.add_account(&alice_account, false).await?;
    keystore.add_key(&key_pair).unwrap();

    // -------------------------------------------------------------------------
    // Setup the remote tx prover
    // -------------------------------------------------------------------------
    let remote_tx_prover: RemoteTransactionProver =
        RemoteTransactionProver::new("https://tx-prover.testnet.miden.io");
    let _tx_prover: Arc<dyn TransactionProver> = Arc::new(remote_tx_prover);

    // We use a dummy transaction request to showcase delegated proving.
    // The only effect of this tx should be increasing Alice's nonce.
    println!("Alice nonce initial: {:?}", alice_account.nonce());
    let script_code = "begin push.1 drop end";
    let tx_script = client
        .script_builder()
        .compile_tx_script(script_code)
        .unwrap();

    let transaction_request = TransactionRequestBuilder::new()
        .custom_script(tx_script)
        .build()
        .unwrap();

    // Note: The delegated prover API has changed in v0.12
    // The new API would be:
    // 1. Execute transaction locally to get execution result
    // 2. Use prove_transaction_with() to generate proof with remote prover
    // 3. Submit the proven transaction
    // However, since the delegated prover is not live yet, we'll use the standard flow

    let _tx_id = client
        .submit_new_transaction(alice_account.id(), transaction_request)
        .await?;

    println!("Transaction submitted (delegated proving not available yet)");

    client.sync_state().await.unwrap();

    let account = client
        .get_account(alice_account.id())
        .await
        .unwrap()
        .unwrap();

    println!("Alice nonce has increased: {:?}", account.account().nonce());

    Ok(())
}
