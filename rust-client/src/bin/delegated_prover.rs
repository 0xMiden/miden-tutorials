use miden_client::auth::AuthSecretKey;
use std::sync::Arc;

use miden_client::account::{AccountStorageMode, AccountType};
use miden_client::builder::ClientBuilder;
use miden_client::crypto::SecretKey;
use miden_client::rpc::TonicRpcClient;
use miden_client::{
    keystore::FilesystemKeyStore,
    rpc::Endpoint,
    transaction::{TransactionProver, TransactionRequestBuilder},
    ClientError, RemoteTransactionProver, ScriptBuilder,
};
use miden_lib::account::wallets::create_basic_wallet;
use miden_lib::AuthScheme;

#[tokio::main]
async fn main() -> Result<(), ClientError> {
    // Initialize client & keystore
    let endpoint = Endpoint::testnet();
    let timeout_ms = 10_000;
    let rpc_api = Arc::new(TonicRpcClient::new(&endpoint, timeout_ms));
    let keystore = FilesystemKeyStore::new("./keystore".into()).unwrap().into();

    let mut client = ClientBuilder::new()
        .rpc(rpc_api)
        .authenticator(keystore)
        .in_debug_mode(true.into())
        .build()
        .await?;

    let keystore: FilesystemKeyStore<rand::prelude::StdRng> =
        FilesystemKeyStore::new("./keystore".into()).unwrap();

    let key_pair = SecretKey::with_rng(client.rng());

    let sync_summary = client.sync_state().await.unwrap();
    println!("Latest block: {}", sync_summary.block_num);

    let (alice_account, seed) = create_basic_wallet(
        [0; 32],
        AuthScheme::RpoFalcon512 {
            pub_key: key_pair.public_key(),
        },
        AccountType::RegularAccountImmutableCode,
        AccountStorageMode::Private,
    )
    .unwrap();

    client
        .add_account(&alice_account, Some(seed), false)
        .await?;
    keystore
        .add_key(&AuthSecretKey::RpoFalcon512(key_pair))
        .unwrap();

    // -------------------------------------------------------------------------
    // Setup the remote tx prover
    // -------------------------------------------------------------------------
    let remote_tx_prover: RemoteTransactionProver =
        RemoteTransactionProver::new("https://tx-prover.testnet.miden.io");
    let tx_prover: Arc<dyn TransactionProver + 'static> = Arc::new(remote_tx_prover);

    // We use a dummy transaction request to showcase delegated proving.
    // The only effect of this tx should be increasing Alice's nonce.
    println!("Alice nonce initial: {:?}", alice_account.nonce());
    let script_code = "begin push.1 drop end";
    let tx_script = ScriptBuilder::new(true)
        .compile_tx_script(script_code)
        .unwrap();

    let transaction_request = TransactionRequestBuilder::new()
        .custom_script(tx_script)
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

    println!("Alice nonce has increased: {:?}", account.account().nonce());

    Ok(())
}
