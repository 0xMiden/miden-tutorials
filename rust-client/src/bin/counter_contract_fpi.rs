use rand::RngCore;
use std::{fs, path::Path, sync::Arc, time::Duration};
use tokio::time::sleep;

use miden_client::{
    account::{
        AccountBuilder, AccountComponent, AccountId, AccountStorageMode, AccountType, StorageSlot,
        StorageSlotName,
    },
    assembly::{
        Assembler, CodeBuilder, DefaultSourceManager, Library, Module, ModuleKind,
        Path as AssemblyPath,
    },
    auth::NoAuth,
    builder::ClientBuilder,
    keystore::FilesystemKeyStore,
    rpc::{domain::account::AccountStorageRequirements, Endpoint, GrpcClient},
    store::AccountRecordData,
    transaction::{ForeignAccount, TransactionKernel, TransactionRequestBuilder},
    ClientError, Felt, Word,
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
    // STEP 1: Create the Count Reader Contract
    // -------------------------------------------------------------------------
    println!("\n[STEP 1] Creating count reader contract.");

    // Load the MASM file for the counter contract
    let count_reader_path = Path::new("../masm/accounts/count_reader.masm");
    let count_reader_code = fs::read_to_string(count_reader_path).unwrap();

    // Prepare assembler (debug mode = true)
    let assembler = TransactionKernel::assembler();

    // Compile the account code into `AccountComponent` with one storage slot
    let count_reader_slot_name =
        StorageSlotName::new("miden::tutorials::count_reader").expect("valid slot name");
    let count_reader_component_code = CodeBuilder::new()
        .compile_component_code(
            "external_contract::count_reader_contract",
            &count_reader_code,
        )
        .unwrap();
    let count_reader_component = AccountComponent::new(
        count_reader_component_code,
        vec![StorageSlot::with_value(
            count_reader_slot_name.clone(),
            Word::default(),
        )],
    )
    .unwrap()
    .with_supports_all_types();

    // Init seed for the counter contract
    let mut init_seed = [0_u8; 32];
    client.rng().fill_bytes(&mut init_seed);

    // Build the new `Account` with the component
    let count_reader_contract = AccountBuilder::new(init_seed)
        .account_type(AccountType::RegularAccountImmutableCode)
        .storage_mode(AccountStorageMode::Public)
        .with_component(count_reader_component.clone())
        .with_auth_component(NoAuth)
        .build()
        .unwrap();

    println!(
        "count_reader hash: {:?}",
        count_reader_contract.commitment()
    );
    println!("contract id: {:?}", count_reader_contract.id());

    client
        .add_account(&count_reader_contract, false)
        .await
        .unwrap();

    // -------------------------------------------------------------------------
    // STEP 2: Build & Get State of the Counter Contract
    // -------------------------------------------------------------------------
    println!("\n[STEP 2] Building counter contract from public state");

    // Define the Counter Contract account id from counter contract deploy
    let (_, counter_contract_id) =
        AccountId::from_bech32("mtst1apfclszryn8a5qqae6sa6hscfgn4mnqp").unwrap();

    println!("counter contract id: {:?}", counter_contract_id);

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
    // STEP 3: Call the Counter Contract via Foreign Procedure Invocation (FPI)
    // -------------------------------------------------------------------------
    println!("\n[STEP 3] Call counter contract with FPI from count copy contract");

    let counter_contract_path = Path::new("../masm/accounts/counter.masm");
    let counter_contract_code = fs::read_to_string(counter_contract_path).unwrap();

    let counter_contract_component_code = CodeBuilder::new()
        .compile_component_code(
            "external_contract::counter_contract",
            &counter_contract_code,
        )
        .unwrap();
    let counter_contract_component = AccountComponent::new(counter_contract_component_code, vec![])
        .unwrap()
        .with_supports_all_types();

    let library = counter_contract_component.component_code().as_library();
    let get_count_hash = library
        .get_procedure_root_by_path("external_contract::counter_contract::get_count")
        .expect("get_count export not found")
        .as_elements()
        .iter()
        .map(|f: &Felt| format!("{}", f.as_int()))
        .collect::<Vec<_>>()
        .join(".");

    println!("get count hash: {:?}", get_count_hash);
    println!("counter id prefix: {:?}", counter_contract_id.prefix());
    println!("suffix: {:?}", counter_contract_id.suffix());

    // Build the script that calls the count_copy_contract
    let script_path = Path::new("../masm/scripts/reader_script.masm");
    let script_code_original = fs::read_to_string(script_path).unwrap();
    let script_code = script_code_original
        .replace("{get_count_proc_hash}", &get_count_hash)
        .replace(
            "{account_id_suffix}",
            &counter_contract_id.suffix().to_string(),
        )
        .replace(
            "{account_id_prefix}",
            &counter_contract_id.prefix().to_string(),
        );

    let account_component_lib = create_library(
        assembler.clone(),
        "external_contract::count_reader_contract",
        &count_reader_code,
    )
    .unwrap();

    let tx_script = client
        .code_builder()
        .with_dynamically_linked_library(&account_component_lib)
        .unwrap()
        .compile_tx_script(&script_code)
        .unwrap();

    let foreign_account =
        ForeignAccount::public(counter_contract_id, AccountStorageRequirements::default()).unwrap();

    // Build a transaction request with the custom script
    let tx_request = TransactionRequestBuilder::new()
        .foreign_accounts([foreign_account])
        .custom_script(tx_script)
        .build()
        .unwrap();

    // Execute and submit the transaction
    let tx_id = client
        .submit_new_transaction(count_reader_contract.id(), tx_request)
        .await
        .unwrap();

    println!(
        "View transaction on MidenScan: https://testnet.midenscan.com/tx/{:?}",
        tx_id
    );

    client.sync_state().await.unwrap();

    sleep(Duration::from_secs(5)).await;

    client.sync_state().await.unwrap();

    // Retrieve updated contract data to see the incremented counter
    let counter_slot_name =
        StorageSlotName::new("miden::tutorials::counter").expect("valid slot name");
    let account_1 = client
        .get_account(counter_contract_id)
        .await
        .unwrap()
        .expect("counter contract not found");
    let account_1 = match account_1.account_data() {
        AccountRecordData::Full(account) => account,
        AccountRecordData::Partial(_) => panic!("counter contract is missing full account data"),
    };
    println!(
        "counter contract storage: {:?}",
        account_1.storage().get_item(&counter_slot_name)
    );

    let account_2_record = client
        .get_account(count_reader_contract.id())
        .await
        .unwrap()
        .expect("count reader contract not found");
    let account_2 = match account_2_record.account_data() {
        AccountRecordData::Full(account) => account,
        AccountRecordData::Partial(_) => {
            panic!("count reader contract is missing full account data")
        }
    };
    println!(
        "count reader contract storage: {:?}",
        account_2.storage().get_item(&count_reader_slot_name)
    );

    Ok(())
}
