use rand::RngCore;
use std::{fs, path::Path};

use miden_client::{
    account::{
        component::AccountComponent, AccountBuilder, AccountId, AccountStorageMode, AccountType,
        StorageSlot,
    },
    rpc::{
        domain::account::{AccountStorageRequirements, StorageMapKey},
        Endpoint,
    },
    transaction::{
        ForeignAccount, TransactionKernel, TransactionRequestBuilder, TransactionScript,
    },
    Client, ClientError, Felt, Word, ZERO,
};

use miden_client_tools::{create_library, instantiate_client};

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

#[tokio::main]
async fn main() -> Result<(), ClientError> {
    // -------------------------------------------------------------------------
    // Initialize Client
    // -------------------------------------------------------------------------
    let endpoint = Endpoint::testnet();
    let mut client = instantiate_client(endpoint, None).await.unwrap();

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

    let anchor_block = client.get_latest_epoch_block().await.unwrap();

    let (oracle_reader_contract, seed) = AccountBuilder::new(seed)
        .anchor((&anchor_block).try_into().unwrap())
        .account_type(AccountType::RegularAccountImmutableCode)
        .storage_mode(AccountStorageMode::Public)
        .with_component(contract_component.clone())
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
    let account_component_lib = create_library(contract_code, library_path).unwrap();

    let tx_script = TransactionScript::compile(
        script_code,
        [],
        assembler.with_library(&account_component_lib).unwrap(),
    )
    .unwrap();

    let tx_increment_request = TransactionRequestBuilder::new()
        .with_foreign_accounts(foreign_accounts)
        .with_custom_script(tx_script)
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
