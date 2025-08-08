use miden_assembly::{
    ast::{Module, ModuleKind},
    Assembler, DefaultSourceManager, LibraryPath,
};
use rand::RngCore;
use std::{fs, path::Path, sync::Arc};

use miden_client::{
    account::{
        component::AccountComponent, AccountBuilder, AccountId, AccountStorageMode, AccountType,
        StorageSlot,
    },
    builder::ClientBuilder,
    rpc::{
        domain::account::{AccountStorageRequirements, StorageMapKey},
        Endpoint, TonicRpcClient,
    },
    transaction::{
        ForeignAccount, TransactionKernel, TransactionRequestBuilder, TransactionScript,
    },
    Client, ClientError, Felt, Word, ZERO,
};
use miden_lib::account::auth::NoAuth;

/// Import the oracle + its publishers and return the ForeignAccount list
/// Due to Pragma's decentralized oracle architecture, we need to get the
/// list of all data publisher accounts to read price from via a nested FPI call
pub async fn get_oracle_foreign_accounts(
    client: &mut Client,
    oracle_account_id: AccountId,
    trading_pair: u64,
) -> Result<Vec<ForeignAccount>, ClientError> {
    client.import_account_by_id(oracle_account_id).await?;

    let oracle_record = client
        .get_account(oracle_account_id)
        .await
        .expect("RPC failed")
        .expect("oracle account not found");

    let storage = oracle_record.account().storage();
    let publisher_count = storage.get_item(1).unwrap()[0].as_int();

    let publisher_ids: Vec<AccountId> = (1..publisher_count.saturating_sub(1))
        .map(|i| {
            let digest = storage.get_item(2 + i as u8).unwrap();
            let words: Word = digest.into();
            AccountId::new_unchecked([words[3], words[2]])
        })
        .collect();

    let mut foreign_accounts = Vec::with_capacity(publisher_ids.len() + 1);

    for pid in publisher_ids {
        client.import_account_by_id(pid).await?;

        foreign_accounts.push(ForeignAccount::public(
            pid,
            AccountStorageRequirements::new([(
                1u8,
                &[StorageMapKey::from([
                    ZERO,
                    ZERO,
                    ZERO,
                    Felt::new(trading_pair),
                ])],
            )]),
        )?);
    }

    foreign_accounts.push(ForeignAccount::public(
        oracle_account_id,
        AccountStorageRequirements::default(),
    )?);

    Ok(foreign_accounts)
}

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
    // -------------------------------------------------------------------------
    // Initialize Client
    // -------------------------------------------------------------------------
    let endpoint = Endpoint::testnet();
    let timeout_ms = 10_000;
    let rpc_api = Arc::new(TonicRpcClient::new(&endpoint, timeout_ms));
    let mut client = ClientBuilder::new()
        .rpc(rpc_api)
        .filesystem_keystore("./keystore")
        .in_debug_mode(true)
        .build()
        .await?;

    println!("Latest block: {}", client.sync_state().await?.block_num);

    // -------------------------------------------------------------------------
    // Get all foreign accounts for oracle data
    // -------------------------------------------------------------------------
    let (_, oracle_account_id) =
        AccountId::from_bech32("mtst1qq0zffxzdykm7qqqqdt24cc2du5ghx99").unwrap();
    let btc_usd_pair_id = 120195681;
    let foreign_accounts: Vec<ForeignAccount> =
        get_oracle_foreign_accounts(&mut client, oracle_account_id, btc_usd_pair_id).await?;

    println!(
        "Oracle accountId prefix: {:?} suffix: {:?}",
        oracle_account_id.prefix(),
        oracle_account_id.suffix()
    );

    // -------------------------------------------------------------------------
    // Create Oracle Reader contract
    // -------------------------------------------------------------------------
    let contract_code =
        fs::read_to_string(Path::new("../masm/accounts/oracle_reader.masm")).unwrap();

    let assembler = TransactionKernel::assembler().with_debug_mode(true);

    let contract_component = AccountComponent::compile(
        contract_code.clone(),
        assembler,
        vec![StorageSlot::empty_value()],
    )
    .unwrap()
    .with_supports_all_types();

    let mut seed = [0_u8; 32];
    client.rng().fill_bytes(&mut seed);

    let (oracle_reader_contract, seed) = AccountBuilder::new(seed)
        .account_type(AccountType::RegularAccountImmutableCode)
        .storage_mode(AccountStorageMode::Public)
        .with_component(contract_component.clone())
        .with_auth_component(NoAuth)
        .build()
        .unwrap();

    client
        .add_account(&oracle_reader_contract.clone(), Some(seed), false)
        .await
        .unwrap();

    // -------------------------------------------------------------------------
    // Build the script that calls our `get_price` procedure
    // -------------------------------------------------------------------------
    let script_path = Path::new("../masm/scripts/oracle_reader_script.masm");
    let script_code = fs::read_to_string(script_path).unwrap();

    let assembler = TransactionKernel::assembler().with_debug_mode(true);
    let library_path = "external_contract::oracle_reader";
    let account_component_lib =
        create_library(assembler.clone(), library_path, &contract_code).unwrap();

    let tx_script = TransactionScript::compile(
        script_code,
        assembler.with_library(&account_component_lib).unwrap(),
    )
    .unwrap();

    let tx_increment_request = TransactionRequestBuilder::new()
        .foreign_accounts(foreign_accounts)
        .custom_script(tx_script)
        .build()
        .unwrap();

    let tx_result = client
        .new_transaction(oracle_reader_contract.id(), tx_increment_request)
        .await
        .unwrap();

    let tx_id = tx_result.executed_transaction().id();
    println!(
        "View transaction on MidenScan: https://testnet.midenscan.com/tx/{:?}",
        tx_id
    );
    // -------------------------------------------------------------------------
    //  Submit transaction to the network
    // -------------------------------------------------------------------------
    let _ = client.submit_transaction(tx_result).await;

    client.sync_state().await.unwrap();

    Ok(())
}
