use rand::RngCore;
use std::sync::Arc;

use miden_client::{
    account::{component::BasicWallet, AccountBuilder, AccountStorageMode, AccountType},
    auth::{AuthFalcon512Rpo, AuthSecretKey},
    builder::ClientBuilder,
    keystore::FilesystemKeyStore,
    rpc::{Endpoint, GrpcClient},
    store::AccountRecordData,
    transaction::{
        LocalTransactionProver, ProvingOptions, TransactionProver, TransactionRequestBuilder,
    },
    ClientError,
};
use miden_client_sqlite_store::ClientBuilderSqliteExt;

#[tokio::main]
async fn main() -> Result<(), ClientError> {
    // Initialize client
    let endpoint = Endpoint::devnet();
    let timeout_ms = 10_000;
    let rpc_client = Arc::new(GrpcClient::new(&endpoint, timeout_ms));

    // Initialize keystore
    let keystore_path = std::path::PathBuf::from("./keystore");
    let keystore = Arc::new(FilesystemKeyStore::new(keystore_path).unwrap());

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

    let key_pair = AuthSecretKey::new_falcon512_rpo();

    let alice_account = AccountBuilder::new(init_seed)
        .account_type(AccountType::RegularAccountImmutableCode)
        .storage_mode(AccountStorageMode::Private)
        .with_auth_component(AuthFalcon512Rpo::new(key_pair.public_key().to_commitment()))
        .with_component(BasicWallet)
        .build()
        .unwrap();

    client.add_account(&alice_account, false).await?;
    keystore.add_key(&key_pair).unwrap();

    // -------------------------------------------------------------------------
    // Setup the local tx prover
    // -------------------------------------------------------------------------
    let local_tx_prover = LocalTransactionProver::new(ProvingOptions::default());
    let tx_prover: Arc<dyn TransactionProver> = Arc::new(local_tx_prover);

    // We use a dummy transaction request to showcase delegated proving.
    // The only effect of this tx should be increasing Alice's nonce.
    println!("Alice nonce initial: {:?}", alice_account.nonce());
    let script_code = "begin push.1 drop end";
    let tx_script = client
        .code_builder()
        .compile_tx_script(script_code)
        .unwrap();

    let transaction_request = TransactionRequestBuilder::new()
        .custom_script(tx_script)
        .build()
        .unwrap();

    // Step 1: Execute the transaction locally
    println!("Executing transaction...");
    let tx_result = client
        .execute_transaction(alice_account.id(), transaction_request)
        .await?;

    // Step 2: Prove the transaction using the local prover
    println!("Proving transaction with local prover...");
    let proven_transaction = client.prove_transaction_with(&tx_result, tx_prover).await?;

    // Step 3: Submit the proven transaction
    println!("Submitting proven transaction...");
    let submission_height = client
        .submit_proven_transaction(proven_transaction, &tx_result)
        .await?;

    // Step 4: Apply the transaction to local store
    client
        .apply_transaction(&tx_result, submission_height)
        .await?;

    println!("Transaction submitted successfully using local prover!");

    client.sync_state().await.unwrap();

    let account_record = client
        .get_account(alice_account.id())
        .await
        .unwrap()
        .expect("alice account not found");
    let account = match account_record.account_data() {
        AccountRecordData::Full(account) => account,
        AccountRecordData::Partial(_) => panic!("alice account is missing full account data"),
    };

    println!("Alice nonce has increased: {:?}", account.nonce());

    Ok(())
}
