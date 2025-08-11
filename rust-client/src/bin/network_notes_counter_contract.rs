use std::{fs, path::Path, sync::Arc};

use miden_client::{
    account::{Account, AccountBuilder, AccountId, AccountStorageMode, AccountType, StorageSlot},
    auth::AuthSecretKey,
    builder::ClientBuilder,
    crypto::SecretKey,
    keystore::FilesystemKeyStore,
    note::{
        Note, NoteAssets, NoteExecutionHint, NoteInputs, NoteMetadata, NoteRecipient, NoteTag,
        NoteType,
    },
    rpc::{Endpoint, TonicRpcClient},
    store::TransactionFilter,
    transaction::{OutputNote, TransactionId, TransactionRequestBuilder, TransactionStatus},
    Client, ClientError, Felt, Word,
};
use miden_lib::account::wallets::BasicWallet;
use miden_lib::transaction::TransactionKernel;
use miden_lib::utils::ScriptBuilder;
use miden_objects::{
    account::AccountComponent,
    account::NetworkId,
    assembly::{Assembler, DefaultSourceManager, Library, LibraryPath, Module, ModuleKind},
};
use rand::{rngs::StdRng, RngCore};
use tokio::time::{sleep, Duration};

/// Helper to instantiate a `Client` for interacting with Miden.
async fn instantiate_client(
    endpoint: Endpoint,
    store_path: Option<&str>,
) -> Result<Client, ClientError> {
    let timeout_ms = 10_000;
    let rpc_api = Arc::new(TonicRpcClient::new(&endpoint, timeout_ms));

    let client = ClientBuilder::new()
        .rpc(rpc_api.clone())
        .filesystem_keystore("./keystore")
        .sqlite_store(store_path.unwrap_or("./store.sqlite3"))
        .in_debug_mode(true)
        .build()
        .await?;

    Ok(client)
}

/// Creates a public note with the specified parameters and submits it to the network.
async fn create_network_note(
    client: &mut Client,
    note_code: String,
    account_library: Library,
    creator_account: Account,
    counter_contract_id: AccountId,
) -> Result<(Note, TransactionId), Box<dyn std::error::Error>> {
    let rng = client.rng();
    let serial_num = rng.inner_mut().draw_word();

    let note_script = ScriptBuilder::default()
        .with_dynamically_linked_library(&account_library)?
        .compile_note_script(note_code)?;
    let note_inputs = NoteInputs::new([].to_vec())?;
    let recipient = NoteRecipient::new(serial_num, note_script, note_inputs.clone());

    let tag = NoteTag::from_account_id(counter_contract_id);
    let metadata = NoteMetadata::new(
        creator_account.id(),
        NoteType::Public,
        tag,
        NoteExecutionHint::none(),
        Felt::new(0),
    )?;

    let note = Note::new(NoteAssets::default(), metadata, recipient);

    let note_req = TransactionRequestBuilder::new()
        .own_output_notes(vec![OutputNote::Full(note.clone())])
        .build()?;
    let tx_result = client
        .new_transaction(creator_account.id(), note_req)
        .await?;

    let _ = client.submit_transaction(tx_result.clone()).await;

    let tx_id = tx_result.executed_transaction().id();
    println!(
        "View transaction on MidenScan: https://testnet.midenscan.com/tx/{:?}",
        tx_id
    );

    client.sync_state().await?;
    Ok((note, tx_id))
}

/// Creates a basic wallet account with RpoFalcon512 authentication.
async fn create_basic_account(
    client: &mut Client,
    keystore: FilesystemKeyStore<StdRng>,
) -> Result<(Account, SecretKey), ClientError> {
    let mut init_seed = [0_u8; 32];
    client.rng().fill_bytes(&mut init_seed);

    let assembler = TransactionKernel::assembler().with_debug_mode(true);
    let incr_nonce_code =
        fs::read_to_string(Path::new("../masm/accounts/auth/no_auth.masm")).unwrap();

    let incr_nonce_component = AccountComponent::compile(
        incr_nonce_code.to_string(),
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

    let key_pair = SecretKey::with_rng(client.rng());
    let builder = AccountBuilder::new(init_seed)
        .account_type(AccountType::RegularAccountUpdatableCode)
        .storage_mode(AccountStorageMode::Public)
        .with_auth_component(incr_nonce_component)
        .with_component(BasicWallet);
    let (account, seed) = builder.build().unwrap();
    client.add_account(&account, Some(seed), false).await?;
    keystore
        .add_key(&AuthSecretKey::RpoFalcon512(key_pair.clone()))
        .unwrap();

    Ok((account, key_pair))
}

/// Creates an account component with no authentication requirements.
async fn create_no_auth_component() -> Result<AccountComponent, Box<dyn std::error::Error>> {
    let assembler: Assembler = TransactionKernel::assembler().with_debug_mode(true);
    let no_auth_code = fs::read_to_string(Path::new("../masm/accounts/auth/no_auth.masm"))?;
    let no_auth_component = AccountComponent::compile(no_auth_code, assembler.clone(), vec![])?
        .with_supports_all_types();

    Ok(no_auth_component)
}

/// Creates a public immutable network smart contract account from the provided MASM code.
async fn create_network_account(
    client: &mut Client,
    account_code: &str,
) -> Result<(Account, Word), ClientError> {
    let assembler: Assembler = TransactionKernel::assembler().with_debug_mode(true);

    let counter_component = AccountComponent::compile(
        account_code.to_string(),
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

    let mut init_seed = [0_u8; 32];
    client.rng().fill_bytes(&mut init_seed);
    let no_auth_component = create_no_auth_component().await.unwrap();
    let (counter_contract, counter_seed) = AccountBuilder::new(init_seed)
        .account_type(AccountType::RegularAccountImmutableCode)
        .storage_mode(AccountStorageMode::Network)
        .with_auth_component(no_auth_component)
        .with_component(counter_component.clone())
        .build()
        .unwrap();

    Ok((counter_contract, counter_seed))
}

/// Waits for a specific transaction to be committed.
async fn wait_for_tx(client: &mut Client, tx_id: TransactionId) -> Result<(), ClientError> {
    loop {
        client.sync_state().await?;

        // Check transaction status
        let txs = client
            .get_transactions(TransactionFilter::Ids(vec![tx_id]))
            .await?;
        let tx_committed = if !txs.is_empty() {
            matches!(txs[0].status, TransactionStatus::Committed(_))
        } else {
            false
        };

        if tx_committed {
            println!("✅ transaction {} committed", tx_id.to_hex());
            break;
        }

        println!(
            "Transaction {} not yet committed. Waiting...",
            tx_id.to_hex()
        );
        sleep(Duration::from_secs(2)).await;
    }

    Ok(())
}

/// Creates a Miden library from the provided account code and library path.
fn create_library(
    account_code: String,
    library_path: &str,
) -> Result<Library, Box<dyn std::error::Error>> {
    let assembler: Assembler = TransactionKernel::assembler().with_debug_mode(true);
    let source_manager = Arc::new(DefaultSourceManager::default());
    let module = Module::parser(ModuleKind::Library).parse_str(
        LibraryPath::new(library_path)?,
        account_code,
        &source_manager,
    )?;
    let library = assembler.clone().assemble_library([module])?;
    Ok(library)
}

/// Deletes the keystore and store files.
async fn delete_keystore_and_store(store_path: Option<&str>) {
    let store_path = store_path.unwrap_or("./store.sqlite3");
    if tokio::fs::metadata(store_path).await.is_ok() {
        if let Err(e) = tokio::fs::remove_file(store_path).await {
            eprintln!("failed to remove {}: {}", store_path, e);
        } else {
            println!("cleared sqlite store: {}", store_path);
        }
    } else {
        println!("store not found: {}", store_path);
    }

    let keystore_dir = "./keystore";
    match tokio::fs::read_dir(keystore_dir).await {
        Ok(mut dir) => {
            while let Ok(Some(entry)) = dir.next_entry().await {
                let file_path = entry.path();
                if let Err(e) = tokio::fs::remove_file(&file_path).await {
                    eprintln!("failed to remove {}: {}", file_path.display(), e);
                } else {
                    println!("removed file: {}", file_path.display());
                }
            }
        }
        Err(e) => eprintln!("failed to read directory {}: {}", keystore_dir, e),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    delete_keystore_and_store(None).await;

    let endpoint = Endpoint::testnet();
    let mut client = instantiate_client(endpoint.clone(), None).await.unwrap();

    let keystore = FilesystemKeyStore::new("./keystore".into()).unwrap();

    let sync_summary = client.sync_state().await.unwrap();
    println!("Latest block: {}", sync_summary.block_num);

    // -------------------------------------------------------------------------
    // STEP 1: Create Basic User Account
    // -------------------------------------------------------------------------
    let (alice_account, _) = create_basic_account(&mut client, keystore.clone())
        .await
        .unwrap();
    println!(
        "alice account id: {:?}",
        alice_account.id().to_bech32(NetworkId::Testnet)
    );

    // -------------------------------------------------------------------------
    // STEP 2: Create Network Counter Smart Contract
    // -------------------------------------------------------------------------
    let counter_code = fs::read_to_string(Path::new("../masm/accounts/counter.masm")).unwrap();

    let (counter_contract, counter_seed) = create_network_account(&mut client, &counter_code)
        .await
        .unwrap();
    println!(
        "contract id: {:?}",
        counter_contract.id().to_bech32(NetworkId::Testnet)
    );

    // Save the counter contract ID to .env file
    let env_content = format!(
        "NETWORK_COUNTER_CONTRACT_ID={}",
        counter_contract.id().to_hex()
    );
    fs::write(".env", env_content).expect("Failed to write .env file");
    println!("Network counter contract ID saved to .env file");

    client
        .add_account(&counter_contract, Some(counter_seed), false)
        .await
        .unwrap();

    // -------------------------------------------------------------------------
    // STEP 3: Deploy Network Account with Transaction Script
    // -------------------------------------------------------------------------
    let script_code =
        fs::read_to_string(Path::new("../masm/scripts/network_increment_script.masm")).unwrap();

    let account_code =
        fs::read_to_string(Path::new("../masm/accounts/network_counter.masm")).unwrap();
    let library_path = "external_contract::network_counter_contract";

    let library = create_library(account_code, library_path).unwrap();

    // let tx_script = create_tx_script(script_code, Some(library)).unwrap();

    let tx_script = ScriptBuilder::default()
        .with_dynamically_linked_library(&library)?
        .compile_tx_script(script_code)?;

    let tx_increment_request = TransactionRequestBuilder::new()
        .custom_script(tx_script)
        .build()
        .unwrap();

    let tx_result = client
        .new_transaction(counter_contract.id(), tx_increment_request)
        .await
        .unwrap();

    let _ = client.submit_transaction(tx_result.clone()).await;

    let tx_id = tx_result.executed_transaction().id();
    println!(
        "View transaction on MidenScan: https://testnet.midenscan.com/tx/{:?}",
        tx_id
    );

    // Wait for the transaction to be committed
    wait_for_tx(&mut client, tx_id).await.unwrap();

    // -------------------------------------------------------------------------
    // STEP 4: Prepare & Create the Network Note
    // -------------------------------------------------------------------------
    let note_code =
        fs::read_to_string(Path::new("../masm/notes/network_increment_note.masm")).unwrap();
    let account_code =
        fs::read_to_string(Path::new("../masm/accounts/network_counter.masm")).unwrap();

    let library_path = "external_contract::network_counter_contract";
    let library = create_library(account_code, library_path).unwrap();

    let (_increment_note, note_tx_id) = create_network_note(
        &mut client,
        note_code,
        library,
        alice_account.clone(),
        counter_contract.id(),
    )
    .await
    .unwrap();

    println!("increment note created, waiting for onchain commitment");

    // Wait for the note transaction to be committed
    wait_for_tx(&mut client, note_tx_id).await.unwrap();

    // -------------------------------------------------------------------------
    // STEP 5: Validate Updated State
    // -------------------------------------------------------------------------

    delete_keystore_and_store(None).await;

    let mut client = instantiate_client(endpoint, None).await.unwrap();

    client
        .import_account_by_id(counter_contract.id())
        .await
        .unwrap();

    sleep(Duration::from_secs(1)).await;
    client.sync_state().await?;

    let new_account_state = client.get_account(counter_contract.id()).await.unwrap();

    if let Some(account) = new_account_state.as_ref() {
        let count: Word = account.account().storage().get_item(0).unwrap().into();
        let val = count.get(3).unwrap().as_int();
        assert_eq!(val, 2);
        println!("🔢 Final counter value: {}", val);
    }

    Ok(())
}
