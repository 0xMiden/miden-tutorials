use std::{fs, path::Path, sync::Arc};

use miden_client::{
    account::{
        AccountBuilder, AccountIdAddress, AccountStorageMode, AccountType, Address,
        AddressInterface, StorageSlot,
    },
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
use miden_lib::account::{
    auth::{self, AuthRpoFalcon512},
    wallets::BasicWallet,
};
use miden_lib::transaction::TransactionKernel;
use miden_lib::utils::ScriptBuilder;
use miden_objects::{
    account::AccountComponent,
    account::NetworkId,
    assembly::{Assembler, DefaultSourceManager, Library, LibraryPath, Module, ModuleKind},
};
use rand::RngCore;
use tokio::time::{sleep, Duration};

/// Waits for a specific transaction to be committed.
async fn wait_for_tx(
    client: &mut Client<FilesystemKeyStore<rand::prelude::StdRng>>,
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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

    let keystore = FilesystemKeyStore::new("./keystore".into()).unwrap();

    let sync_summary = client.sync_state().await.unwrap();
    println!("Latest block: {}", sync_summary.block_num);

    // -------------------------------------------------------------------------
    // STEP 1: Create Basic User Account
    // -------------------------------------------------------------------------
    println!("\n[STEP 1] Creating a new account for Alice");

    // Account seed
    let mut init_seed = [0_u8; 32];
    client.rng().fill_bytes(&mut init_seed);

    let key_pair = SecretKey::with_rng(client.rng());

    // Build the account
    let builder = AccountBuilder::new(init_seed)
        .account_type(AccountType::RegularAccountUpdatableCode)
        .storage_mode(AccountStorageMode::Public)
        .with_auth_component(AuthRpoFalcon512::new(key_pair.public_key()))
        .with_component(BasicWallet);

    let (alice_account, seed) = builder.build().unwrap();

    // Add the account to the client
    client
        .add_account(&alice_account, Some(seed), false)
        .await?;

    // Add the key pair to the keystore
    keystore
        .add_key(&AuthSecretKey::RpoFalcon512(key_pair))
        .unwrap();

    println!(
        "Alice's account ID: {:?}",
        Address::from(AccountIdAddress::new(
            alice_account.id(),
            AddressInterface::Unspecified
        ))
        .to_bech32(NetworkId::Testnet)
    );

    // -------------------------------------------------------------------------
    // STEP 2: Create Network Counter Smart Contract
    // -------------------------------------------------------------------------
    println!("\n[STEP 2] Creating a network counter smart contract");

    let counter_code = fs::read_to_string(Path::new("../masm/accounts/counter.masm")).unwrap();

    // Create the network counter smart contract account
    // First, compile the MASM code into an account component
    let assembler: Assembler = TransactionKernel::assembler().with_debug_mode(true);
    let counter_component = AccountComponent::compile(
        counter_code.to_string(),
        assembler.clone(),
        vec![StorageSlot::Value([Felt::new(0); 4].into())], // Initialize counter storage to 0
    )
    .unwrap()
    .with_supports_all_types();

    // Generate a random seed for the account
    let mut init_seed = [0_u8; 32];
    client.rng().fill_bytes(&mut init_seed);

    // Build the immutable network account with no authentication
    let (counter_contract, counter_seed) = AccountBuilder::new(init_seed)
        .account_type(AccountType::RegularAccountImmutableCode) // Immutable code
        .storage_mode(AccountStorageMode::Network) // Stored on network
        .with_auth_component(auth::NoAuth) // No authentication required
        .with_component(counter_component)
        .build()
        .unwrap();

    client
        .add_account(&counter_contract, Some(counter_seed), false)
        .await
        .unwrap();

    println!(
        "contract id: {:?}",
        Address::from(AccountIdAddress::new(
            counter_contract.id(),
            AddressInterface::Unspecified
        ))
        .to_bech32(NetworkId::Testnet)
    );

    // -------------------------------------------------------------------------
    // STEP 3: Deploy Network Account with Transaction Script
    // -------------------------------------------------------------------------
    println!("\n[STEP 3] Deploy network counter smart contract");

    let script_code = fs::read_to_string(Path::new("../masm/scripts/counter_script.masm")).unwrap();

    let account_code = fs::read_to_string(Path::new("../masm/accounts/counter.masm")).unwrap();
    let library_path = "external_contract::counter_contract";

    let library = create_library(account_code, library_path).unwrap();

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
    println!("\n[STEP 4] Creating a network note for network counter contract");

    let network_note_code =
        fs::read_to_string(Path::new("../masm/notes/network_increment_note.masm")).unwrap();
    let account_code = fs::read_to_string(Path::new("../masm/accounts/counter.masm")).unwrap();

    let library_path = "external_contract::counter_contract";
    let library = create_library(account_code, library_path).unwrap();

    // Create and submit the network note that will increment the counter
    // Generate a random serial number for the note
    let rng = client.rng();
    let serial_num = rng.inner_mut().draw_word();

    // Compile the note script with the counter contract library
    let note_script = ScriptBuilder::default()
        .with_dynamically_linked_library(&library)?
        .compile_note_script(network_note_code)?;

    // Create note recipient with empty inputs
    let note_inputs = NoteInputs::new([].to_vec())?;
    let recipient = NoteRecipient::new(serial_num, note_script, note_inputs);

    // Set up note metadata - tag it with the counter contract ID so it gets consumed
    let tag = NoteTag::from_account_id(counter_contract.id());
    let metadata = NoteMetadata::new(
        alice_account.id(),
        NoteType::Public,
        tag,
        NoteExecutionHint::none(),
        Felt::new(0),
    )?;

    // Create the complete note
    let increment_note = Note::new(NoteAssets::default(), metadata, recipient);

    // Build and submit the transaction containing the note
    let note_req = TransactionRequestBuilder::new()
        .own_output_notes(vec![OutputNote::Full(increment_note)])
        .build()?;

    let tx_result = client.new_transaction(alice_account.id(), note_req).await?;

    let _ = client.submit_transaction(tx_result.clone()).await;

    let note_tx_id = tx_result.executed_transaction().id();
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

    client.sync_state().await?;

    // Checking updated state
    let new_account_state = client.get_account(counter_contract.id()).await.unwrap();

    if let Some(account) = new_account_state.as_ref() {
        let count: Word = account.account().storage().get_item(0).unwrap().into();
        let val = count.get(3).unwrap().as_int();
        assert_eq!(val, 2);
        println!("🔢 Final counter value: {}", val);
    }

    Ok(())
}
