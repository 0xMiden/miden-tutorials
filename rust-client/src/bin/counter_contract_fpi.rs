use rand::RngCore;
use std::{fs, path::Path, sync::Arc};

use miden_assembly::{
    ast::{Module, ModuleKind},
    LibraryPath,
};
use miden_client::{
    account::AccountId,
    account::{AccountBuilder, AccountStorageMode, AccountType, StorageSlot},
    builder::ClientBuilder,
    rpc::{domain::account::AccountStorageRequirements, Endpoint, TonicRpcClient},
    transaction::{
        ForeignAccount, TransactionKernel, TransactionRequestBuilder, TransactionScript,
    },
    ClientError, Felt,
};
use miden_objects::{
    account::AccountComponent, assembly::Assembler, assembly::DefaultSourceManager,
};

fn create_library(
    assembler: Assembler,
    library_path: &str,
    source_code: &str,
) -> Result<miden_assembly::Library, Box<dyn std::error::Error>> {
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
    let rpc_api = Arc::new(TonicRpcClient::new(&endpoint, timeout_ms));

    let mut client = ClientBuilder::new()
        .with_rpc(rpc_api)
        .with_filesystem_keystore("./keystore")
        .in_debug_mode(true)
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
    let assembler: Assembler = TransactionKernel::assembler().with_debug_mode(true);

    // Compile the account code into `AccountComponent` with one storage slot
    let counter_component = AccountComponent::compile(
        count_reader_code.clone(),
        assembler.clone(),
        vec![StorageSlot::Value([
            Felt::new(0),
            Felt::new(0),
            Felt::new(0),
            Felt::new(0),
        ])],
    )
    .unwrap()
    .with_supports_all_types();

    // Init seed for the counter contract
    let mut init_seed = [0_u8; 32];
    client.rng().fill_bytes(&mut init_seed);

    // Anchor block of the account
    let anchor_block = client.get_latest_epoch_block().await.unwrap();

    // Build the new `Account` with the component
    let (count_reader_contract, count_reader_seed) = AccountBuilder::new(init_seed)
        .anchor((&anchor_block).try_into().unwrap())
        .account_type(AccountType::RegularAccountImmutableCode)
        .storage_mode(AccountStorageMode::Public)
        .with_component(counter_component.clone())
        .build()
        .unwrap();

    println!(
        "count_reader hash: {:?}",
        count_reader_contract.commitment()
    );
    println!("contract id: {:?}", count_reader_contract.id().to_hex());

    client
        .add_account(
            &count_reader_contract.clone(),
            Some(count_reader_seed),
            false,
        )
        .await
        .unwrap();

    // -------------------------------------------------------------------------
    // STEP 2: Build & Get State of the Counter Contract
    // -------------------------------------------------------------------------
    println!("\n[STEP 2] Building counter contract from public state");

    // Define the Counter Contract account id from counter contract deploy
    let counter_contract_id = AccountId::from_hex("0xac6eeab35afb09000000ea9fae7722").unwrap();

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
    // STEP 3: Call the Counter Contract via Foreign Procedure Invocation (FPI)
    // -------------------------------------------------------------------------
    println!("\n[STEP 3] Call counter contract with FPI from count copy contract");

    let counter_contract_path = Path::new("../masm/accounts/counter.masm");
    let counter_contract_code = fs::read_to_string(counter_contract_path).unwrap();

    let counter_contract_component =
        AccountComponent::compile(counter_contract_code, assembler.clone(), vec![])
            .unwrap()
            .with_supports_all_types();

    // Getting the hash of the `get_count` procedure
    let get_proc_export = counter_contract_component
        .library()
        .exports()
        .find(|export| export.name.as_str() == "get_count")
        .unwrap();

    let get_proc_mast_id = counter_contract_component
        .library()
        .get_export_node_id(get_proc_export);

    let get_count_hash = counter_contract_component
        .library()
        .mast_forest()
        .get_node_by_id(get_proc_mast_id)
        .unwrap()
        .digest()
        .to_hex();

    println!("get count hash: {:?}", get_count_hash);
    println!("counter id prefix: {:?}", counter_contract.id().prefix());
    println!("suffix: {:?}", counter_contract.id().suffix());

    // Build the script that calls the count_copy_contract
    let script_path = Path::new("../masm/scripts/reader_script.masm");
    let script_code_original = fs::read_to_string(script_path).unwrap();
    let script_code = script_code_original
        .replace("{get_count_proc_hash}", &get_count_hash)
        .replace(
            "{account_id_suffix}",
            &counter_contract.id().suffix().to_string(),
        )
        .replace(
            "{account_id_prefix}",
            &counter_contract.id().prefix().to_string(),
        );

    let account_component_lib = create_library(
        assembler.clone(),
        "external_contract::count_reader_contract",
        &count_reader_code,
    )
    .unwrap();

    let tx_script = TransactionScript::compile(
        script_code,
        [],
        assembler.with_library(&account_component_lib).unwrap(),
    )
    .unwrap();

    let foreign_account =
        ForeignAccount::public(counter_contract_id, AccountStorageRequirements::default()).unwrap();

    // Build a transaction request with the custom script
    let tx_request = TransactionRequestBuilder::new()
        .with_foreign_accounts([foreign_account])
        .with_custom_script(tx_script)
        .build()
        .unwrap();

    // Execute the transaction locally
    let tx_result = client
        .new_transaction(count_reader_contract.id(), tx_request)
        .await
        .unwrap();

    let tx_id = tx_result.executed_transaction().id();
    println!(
        "View transaction on MidenScan: https://testnet.midenscan.com/tx/{:?}",
        tx_id
    );

    // Submit transaction to the network
    let _ = client.submit_transaction(tx_result).await;

    client.sync_state().await.unwrap();

    // Retrieve updated contract data to see the incremented counter
    let account_1 = client.get_account(counter_contract.id()).await.unwrap();
    println!(
        "counter contract storage: {:?}",
        account_1.unwrap().account().storage().get_item(0)
    );

    let account_2 = client
        .get_account(count_reader_contract.id())
        .await
        .unwrap();
    println!(
        "count reader contract storage: {:?}",
        account_2.unwrap().account().storage().get_item(0)
    );

    Ok(())
}
