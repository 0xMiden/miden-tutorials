use std::{fs, path::Path, sync::Arc};

use miden_client::{
    account::{
        component::BasicWallet,
        AccountBuilder,
        AccountComponent,
        AccountStorageMode,
        AccountType,
        StorageSlot,
        StorageSlotName,
    },
    crypto::FeltRng,
    address::NetworkId,
    assembly::{
        Assembler,
        CodeBuilder,
        DefaultSourceManager,
        Library,
        Module,
        ModuleKind,
        Path as AssemblyPath,
    },
    auth::{self, AuthFalcon512Rpo, AuthSecretKey},
    builder::ClientBuilder,
    keystore::FilesystemKeyStore,
    note::{Note, NoteAssets, NoteInputs, NoteMetadata, NoteRecipient, NoteTag, NoteType},
    rpc::{Endpoint, GrpcClient},
    store::AccountRecordData,
    store::TransactionFilter,
    transaction::{
        OutputNote,
        TransactionId,
        TransactionKernel,
        TransactionRequestBuilder,
        TransactionStatus,
    },
    Client, ClientError, Felt, Word,
};
use miden_client_sqlite_store::ClientBuilderSqliteExt;
use rand::RngCore;
use tokio::time::{sleep, Duration};

/// Waits for a specific transaction to be committed.
async fn wait_for_tx(
    client: &mut Client<FilesystemKeyStore>,
    tx_id: TransactionId,
) -> Result<(), ClientError> {
    loop {
        client.sync_state().await?;

        // Check transaction status
        let txs = client
            .get_transactions(TransactionFilter::Ids(vec![tx_id]))
            .await?;
        let tx_committed = if !txs.is_empty() {
            matches!(txs[0].status, TransactionStatus::Committed { .. })
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
    let assembler: Assembler = TransactionKernel::assembler();
    let source_manager = Arc::new(DefaultSourceManager::default());
    let module = Module::parser(ModuleKind::Library).parse_str(
        AssemblyPath::new(library_path),
        account_code,
        source_manager.clone(),
    )?;
    let library = assembler.clone().assemble_library([module])?;
    Ok(library)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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
    // STEP 1: Create Basic User Account
    // -------------------------------------------------------------------------
    println!("\n[STEP 1] Creating a new account for Alice");

    // Account seed
    let mut init_seed = [0_u8; 32];
    client.rng().fill_bytes(&mut init_seed);

    let key_pair = AuthSecretKey::new_falcon512_rpo();

    // Build the account
    let alice_account = AccountBuilder::new(init_seed)
        .account_type(AccountType::RegularAccountUpdatableCode)
        .storage_mode(AccountStorageMode::Public)
        .with_auth_component(AuthFalcon512Rpo::new(key_pair.public_key().to_commitment()))
        .with_component(BasicWallet)
        .build()
        .unwrap();

    // Add the account to the client
    client.add_account(&alice_account, false).await?;

    // Add the key pair to the keystore
    keystore.add_key(&key_pair).unwrap();

    println!(
        "Alice's account ID: {:?}",
        alice_account.id().to_bech32(NetworkId::Testnet)
    );

    // -------------------------------------------------------------------------
    // STEP 2: Create Network Counter Smart Contract
    // -------------------------------------------------------------------------
    println!("\n[STEP 2] Creating a network counter smart contract");

    let counter_code = fs::read_to_string(Path::new("../masm/accounts/counter.masm")).unwrap();

    // Create the network counter smart contract account
    // First, compile the MASM code into an account component
    let counter_slot_name =
        StorageSlotName::new("miden::tutorials::counter").expect("valid slot name");
    let component_code = CodeBuilder::new()
        .compile_component_code("external_contract::counter_contract", &counter_code)?;
    let counter_component = AccountComponent::new(
        component_code,
        vec![StorageSlot::with_value(counter_slot_name.clone(), [Felt::new(0); 4].into())],
    )?
    .with_supports_all_types();

    // Generate a random seed for the account
    let mut init_seed = [0_u8; 32];
    client.rng().fill_bytes(&mut init_seed);

    // Build the immutable network account with no authentication
    let counter_contract = AccountBuilder::new(init_seed)
        .account_type(AccountType::RegularAccountImmutableCode) // Immutable code
        .storage_mode(AccountStorageMode::Network) // Stored on network
        .with_auth_component(auth::NoAuth) // No authentication required
        .with_component(counter_component)
        .build()
        .unwrap();

    client.add_account(&counter_contract, false).await.unwrap();

    println!(
        "contract id: {:?}",
        counter_contract.id().to_bech32(NetworkId::Testnet)
    );

    // -------------------------------------------------------------------------
    // STEP 3: Deploy Network Account with Transaction Script
    // -------------------------------------------------------------------------
    println!("\n[STEP 3] Deploy network counter smart contract");

    let script_code = fs::read_to_string(Path::new("../masm/scripts/counter_script.masm")).unwrap();

    let account_code = fs::read_to_string(Path::new("../masm/accounts/counter.masm")).unwrap();
    let library_path = "external_contract::counter_contract";

    let library = create_library(account_code, library_path).unwrap();

    let tx_script = client
        .code_builder()
        .with_dynamically_linked_library(&library)?
        .compile_tx_script(&script_code)?;

    let tx_increment_request = TransactionRequestBuilder::new()
        .custom_script(tx_script)
        .build()
        .unwrap();

    let tx_id = client
        .submit_new_transaction(counter_contract.id(), tx_increment_request)
        .await
        .unwrap();

    println!(
        "View transaction on MidenScan: https://testnet.midenscan.com/tx/{:?}",
        tx_id
    );

    // Wait for the transaction to be committed
    wait_for_tx(&mut client, tx_id).await.unwrap();

    // -------------------------------------------------------------------------
    // STEP 4: Prepare & Create the Network Note
    // -------------------------------------------------------------------------
    println!("\n[STEP 4] Creating a network note for network counter contract");

    let network_note_code =
        fs::read_to_string(Path::new("../masm/notes/network_increment_note.masm")).unwrap();
    let account_code = fs::read_to_string(Path::new("../masm/accounts/counter.masm")).unwrap();

    let library_path = "external_contract::counter_contract";
    let library = create_library(account_code, library_path).unwrap();

    // Create and submit the network note that will increment the counter
    // Generate a random serial number for the note
    let serial_num = client.rng().draw_word();

    // Compile the note script with the counter contract library
    let note_script = client
        .code_builder()
        .with_dynamically_linked_library(&library)?
        .compile_note_script(&network_note_code)?;

    // Create note recipient with empty inputs
    let note_inputs = NoteInputs::new([].to_vec())?;
    let recipient = NoteRecipient::new(serial_num, note_script, note_inputs);

    // Set up note metadata - tag it with the counter contract ID so it gets consumed
    let tag = NoteTag::with_account_target(counter_contract.id());
    let metadata = NoteMetadata::new(alice_account.id(), NoteType::Public, tag);

    // Create the complete note
    let increment_note = Note::new(NoteAssets::default(), metadata, recipient);

    // Build and submit the transaction containing the note
    let note_req = TransactionRequestBuilder::new()
        .own_output_notes(vec![OutputNote::Full(increment_note)])
        .build()?;

    let note_tx_id = client
        .submit_new_transaction(alice_account.id(), note_req)
        .await?;

    println!(
        "View transaction on MidenScan: https://testnet.midenscan.com/tx/{:?}",
        note_tx_id
    );

    client.sync_state().await?;

    println!("network increment note creation tx submitted, waiting for onchain commitment");

    // Wait for the note transaction to be committed
    wait_for_tx(&mut client, note_tx_id).await.unwrap();

    // Waiting for network note to be picked up by the network transaction builder
    sleep(Duration::from_secs(6)).await;

    let mut last_val = None;
    for _ in 0..10 {
        client.sync_state().await?;

        // Checking updated state
        let new_account_state = client.get_account(counter_contract.id()).await.unwrap();

        if let Some(account_record) = new_account_state.as_ref() {
            let account = match account_record.account_data() {
                AccountRecordData::Full(account) => account,
                AccountRecordData::Partial(_) => {
                    panic!("counter contract is missing full account data")
                }
            };
            let count: Word = account
                .storage()
                .get_item(&counter_slot_name)
                .unwrap()
                .into();
            let val = count.get(3).unwrap().as_int();
            if val >= 2 {
                println!("🔢 Final counter value: {}", val);
                return Ok(());
            }
            last_val = Some(val);
        }

        // Give the network note builder time to process the note.
        sleep(Duration::from_secs(6)).await;
    }

    if let Some(val) = last_val {
        println!(
            "Counter value did not reach 2 yet (last observed value: {}).",
            val
        );
    } else {
        println!("Counter value not available yet.");
    }

    Ok(())
}
