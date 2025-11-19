use miden_lib::transaction::TransactionKernel;
use rand::rngs::StdRng;
use std::{fs, path::Path, sync::Arc};

use miden_client::{
    account::AccountId,
    assembly::{Assembler, DefaultSourceManager, LibraryPath, Module, ModuleKind},
    builder::ClientBuilder,
    keystore::FilesystemKeyStore,
    rpc::{Endpoint, GrpcClient},
    transaction::TransactionRequestBuilder,
    ClientError,
};
use miden_client_sqlite_store::ClientBuilderSqliteExt;

fn create_library(
    assembler: Assembler,
    library_path: &str,
    source_code: &str,
) -> Result<miden_objects::assembly::Library, Box<dyn std::error::Error>> {
    let source_manager = Arc::new(DefaultSourceManager::default());
    let module = Module::parser(ModuleKind::Library).parse_str(
        LibraryPath::new(library_path)?,
        source_code,
        &source_manager,
    )?;
    let library = assembler.clone().assemble_library([module])?;
    Ok(library)
}

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

    // -------------------------------------------------------------------------
    // STEP 1: Read the Public State of the Counter Contract
    // -------------------------------------------------------------------------
    println!("\n[STEP 1] Reading data from public state");

    // Define the Counter Contract account id from counter contract deploy
    let (_, counter_contract_id) =
        AccountId::from_bech32("mtst1arjemrxne8lj5qz4mg9c8mtyxg954483").unwrap();

    client
        .import_account_by_id(counter_contract_id)
        .await
        .unwrap();

    let counter_contract_details = client.get_account(counter_contract_id).await.unwrap();

    let counter_contract = if let Some(account_record) = counter_contract_details {
        // Clone the account to get an owned instance
        let account = account_record.account().clone();
        println!(
            "Account details: {:?}",
            account.storage().slots().first().unwrap()
        );
        account // Now returns an owned account
    } else {
        panic!("Counter contract not found!");
    };

    // -------------------------------------------------------------------------
    // STEP 2: Call the Counter Contract with a script
    // -------------------------------------------------------------------------
    println!("\n[STEP 2] Call the increment_count procedure in the counter contract");

    // Load the MASM script referencing the increment procedure
    let script_path = Path::new("../masm/scripts/counter_script.masm");
    let script_code = fs::read_to_string(script_path).unwrap();

    let counter_path = Path::new("../masm/accounts/counter.masm");
    let counter_code = fs::read_to_string(counter_path).unwrap();

    let assembler = TransactionKernel::assembler().with_debug_mode(true);
    let account_component_lib = create_library(
        assembler.clone(),
        "external_contract::counter_contract",
        &counter_code,
    )
    .unwrap();

    let tx_script = client
        .script_builder()
        .with_dynamically_linked_library(&account_component_lib)
        .unwrap()
        .compile_tx_script(&script_code)
        .unwrap();

    // Build a transaction request with the custom script
    let tx_increment_request = TransactionRequestBuilder::new()
        .custom_script(tx_script)
        .build()
        .unwrap();

    // Execute and submit the transaction
    let tx_id = client
        .submit_new_transaction(counter_contract.id(), tx_increment_request)
        .await
        .unwrap();

    println!(
        "View transaction on MidenScan: https://testnet.midenscan.com/tx/{:?}",
        tx_id
    );

    client.sync_state().await.unwrap();

    // Retrieve updated contract data to see the incremented counter
    let account = client.get_account(counter_contract.id()).await.unwrap();
    println!(
        "counter contract storage: {:?}",
        account.unwrap().account().storage().get_item(0)
    );
    Ok(())
}
