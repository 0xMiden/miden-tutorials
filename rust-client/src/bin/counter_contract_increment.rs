use std::{fs, path::Path, sync::Arc};

use miden_client::{
    account::{AccountId, StorageSlotName},
    assembly::{
        Assembler, DefaultSourceManager, Library, Module, ModuleKind, Path as AssemblyPath,
    },
    builder::ClientBuilder,
    keystore::FilesystemKeyStore,
    rpc::{Endpoint, GrpcClient},
    store::AccountRecordData,
    transaction::{TransactionKernel, TransactionRequestBuilder},
    ClientError,
};
use miden_client_sqlite_store::ClientBuilderSqliteExt;

fn create_library(
    assembler: Assembler,
    library_path: &str,
    source_code: &str,
) -> Result<Library, Box<dyn std::error::Error>> {
    let source_manager = Arc::new(DefaultSourceManager::default());
    let module = Module::parser(ModuleKind::Library).parse_str(
        AssemblyPath::new(library_path),
        source_code,
        source_manager.clone(),
    )?;
    let library = assembler.clone().assemble_library([module])?;
    Ok(library)
}

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

    // -------------------------------------------------------------------------
    // STEP 1: Read the Public State of the Counter Contract
    // -------------------------------------------------------------------------
    println!("\n[STEP 1] Reading data from public state");

    // Define the Counter Contract account id from counter contract deploy
    let (_, counter_contract_id) =
        AccountId::from_bech32("mtst1apfclszryn8a5qqae6sa6hscfgn4mnqp").unwrap();

    client
        .import_account_by_id(counter_contract_id)
        .await
        .unwrap();

    let counter_contract_details = client
        .get_account(counter_contract_id)
        .await
        .unwrap()
        .expect("counter contract not found");
    let counter_contract = match counter_contract_details.account_data() {
        AccountRecordData::Full(account) => account,
        AccountRecordData::Partial(_) => panic!("counter contract is missing full account data"),
    };
    println!(
        "Account details: {:?}",
        counter_contract.storage().slots().first().unwrap()
    );

    // -------------------------------------------------------------------------
    // STEP 2: Call the Counter Contract with a script
    // -------------------------------------------------------------------------
    println!("\n[STEP 2] Call the increment_count procedure in the counter contract");

    // Load the MASM script referencing the increment procedure
    let script_path = Path::new("../masm/scripts/counter_script.masm");
    let script_code = fs::read_to_string(script_path).unwrap();

    let counter_path = Path::new("../masm/accounts/counter.masm");
    let counter_code = fs::read_to_string(counter_path).unwrap();

    let assembler = TransactionKernel::assembler();
    let account_component_lib = create_library(
        assembler.clone(),
        "external_contract::counter_contract",
        &counter_code,
    )
    .unwrap();

    let tx_script = client
        .code_builder()
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
        .submit_new_transaction(counter_contract_id, tx_increment_request)
        .await
        .unwrap();

    println!(
        "View transaction on MidenScan: https://testnet.midenscan.com/tx/{:?}",
        tx_id
    );

    client.sync_state().await.unwrap();

    // Retrieve updated contract data to see the incremented counter
    let account_record = client
        .get_account(counter_contract_id)
        .await
        .unwrap()
        .expect("counter contract not found");
    let account = match account_record.account_data() {
        AccountRecordData::Full(account) => account,
        AccountRecordData::Partial(_) => panic!("counter contract is missing full account data"),
    };
    let counter_slot_name =
        StorageSlotName::new("miden::tutorials::counter").expect("valid slot name");
    println!(
        "counter contract storage: {:?}",
        account.storage().get_item(&counter_slot_name)
    );
    Ok(())
}
