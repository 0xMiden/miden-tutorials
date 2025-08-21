use miden_lib::account::auth::NoAuth;
use rand::RngCore;
use std::{fs, path::Path, sync::Arc};

use miden_assembly::{
    ast::{Module, ModuleKind},
    LibraryPath,
};
use miden_client::{
    account::{AccountBuilder, AccountStorageMode, AccountType, StorageSlot},
    builder::ClientBuilder,
    crypto::FeltRng,
    keystore::FilesystemKeyStore,
    note::{
        Note, NoteAssets, NoteExecutionHint, NoteExecutionMode, NoteInputs, NoteMetadata,
        NoteRecipient, NoteScript, NoteTag, NoteType,
    },
    rpc::{Endpoint, TonicRpcClient},
    transaction::{OutputNote, TransactionKernel, TransactionRequestBuilder, TransactionScript},
    ClientError, Felt,
};
use miden_client_tools::create_basic_account;
use miden_objects::{
    account::{AccountComponent, NetworkId, StorageMap},
    assembly::Assembler,
    assembly::DefaultSourceManager,
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
        .rpc(rpc_api)
        .filesystem_keystore("./keystore")
        .in_debug_mode(true)
        .build()
        .await?;

    let sync_summary = client.sync_state().await.unwrap();
    println!("Latest block: {}", sync_summary.block_num);

    // -------------------------------------------------------------------------
    // STEP 1: Create Alice and Bob accounts (players)
    // -------------------------------------------------------------------------

    let keystore: FilesystemKeyStore<rand::prelude::StdRng> =
        FilesystemKeyStore::new("./keystore".into()).unwrap();

    let (alice_account, _) = create_basic_account(&mut client, keystore.clone())
        .await
        .unwrap();

    let (bob_account, _) = create_basic_account(&mut client, keystore.clone())
        .await
        .unwrap();

    // print suffix and prefix for both alice and bob
    println!("alice prefix: {:?}", alice_account.id().prefix().as_felt());
    println!("alice suffix: {:?}", alice_account.id().suffix());
    println!("bob prefix: {:?}", bob_account.id().prefix().as_felt());
    println!("bob suffix: {:?}", bob_account.id().suffix());

    // -------------------------------------------------------------------------
    // STEP 2: Create the tic tac toe game contract
    // -------------------------------------------------------------------------
    println!("\n[STEP 2] Creating tic tac toe game contract.");

    // Prepare assembler (debug mode = true)
    let assembler: Assembler = TransactionKernel::assembler().with_debug_mode(true);

    // Load the MASM file for the tic tac toe game contract
    let game_path = Path::new("../masm/accounts/tic_tac_toe.masm");
    let game_code = fs::read_to_string(game_path).unwrap();

    let empty_storage_slot = StorageSlot::Value([Felt::new(0); 4]);

    let storage_map = StorageMap::new();
    let storage_slot_map = StorageSlot::Map(storage_map.clone());

    // Compile the account code into `AccountComponent` with one storage slot
    let game_component = AccountComponent::compile(
        game_code.clone(),
        assembler,
        vec![
            // player1 storage slot
            empty_storage_slot.clone(),
            // player2 storage slot
            empty_storage_slot.clone(),
            // flag storage slot
            empty_storage_slot.clone(),
            // mapping storage slot
            storage_slot_map,
        ],
    )
    .unwrap()
    .with_supports_all_types();

    // Init seed for the counter contract
    let mut seed = [0_u8; 32];
    client.rng().fill_bytes(&mut seed);

    // Build the new `Account` with the component
    let (game_contract, game_seed) = AccountBuilder::new(seed)
        .account_type(AccountType::RegularAccountImmutableCode)
        .storage_mode(AccountStorageMode::Public)
        .with_component(game_component.clone())
        .with_auth_component(NoAuth)
        .build()
        .unwrap();

    println!(
        "game_contract id: {:?}",
        game_contract.id().to_bech32(NetworkId::Testnet)
    );

    client
        .add_account(&game_contract.clone(), Some(game_seed), false)
        .await
        .unwrap();

    // -------------------------------------------------------------------------
    // STEP 3: Call the Game Contract with a script
    // -------------------------------------------------------------------------
    println!("\n[STEP 3] Call Game Contract Constructor");

    // Compose TX script input arguments
    let deployment_script_arg: [Felt; 4] = [
        bob_account.id().suffix(),
        bob_account.id().prefix().as_felt(),
        alice_account.id().suffix(),
        alice_account.id().prefix().as_felt(),
    ];

    // Load the MASM script referencing the game deployment procedure
    let deployment_script_path = Path::new("../masm/scripts/game_deployment_script.masm");
    let deployment_script_code = fs::read_to_string(deployment_script_path).unwrap();

    let assembler: Assembler = TransactionKernel::assembler().with_debug_mode(true);
    let account_component_lib = create_library(
        assembler.clone(),
        "external_contract::game_contract",
        &game_code,
    )
    .unwrap();

    let deployment_script = TransactionScript::compile(
        deployment_script_code,
        assembler
            .clone()
            .with_library(&account_component_lib)
            .unwrap(),
    )
    .unwrap();

    // Build a transaction request with the custom script
    let tx_game_deployment_request = TransactionRequestBuilder::new()
        .custom_script(deployment_script)
        .script_arg(deployment_script_arg)
        .build()
        .unwrap();

    // Execute the transaction locally
    let tx_result = client
        .new_transaction(game_contract.id(), tx_game_deployment_request)
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

    // Retrieve updated contract data to see the incremented game
    let account = client.get_account(game_contract.id()).await.unwrap();
    let account_data = account.unwrap().account().clone();
    println!(
        "game contract player1 storage: {:?}",
        account_data.storage().get_item(0)
    );
    println!(
        "game contract player2 storage: {:?}",
        account_data.storage().get_item(1)
    );
    println!(
        "game contract flag storage: {:?}",
        account_data.storage().get_item(2)
    );
    println!(
        "game contract mapping storage: {:?}",
        account_data.storage().get_item(3)
    );

    // -------------------------------------------------------------------------
    // STEP 4: Call the Game Contract with "make a move" note
    // -------------------------------------------------------------------------
    println!("\n[STEP 4] Create 'make a move' note");

    let note_code = fs::read_to_string(Path::new("../masm/notes/make_a_move_note.masm")).unwrap();
    let note_script = NoteScript::compile(
        note_code,
        assembler.with_library(&account_component_lib).unwrap(),
    )
    .unwrap();

    let index: u64 = 5;
    let note_inputs = NoteInputs::new(vec![Felt::new(index)]).unwrap();
    let serial_num = client.rng().draw_word();
    let recipient = NoteRecipient::new(serial_num, note_script, note_inputs);
    let tag = NoteTag::for_public_use_case(0, 0, NoteExecutionMode::Local).unwrap();
    let metadata = NoteMetadata::new(
        alice_account.id(),
        NoteType::Public,
        tag,
        NoteExecutionHint::always(),
        Felt::new(0),
    )?;
    let assets = NoteAssets::new(vec![])?;
    let make_a_move_note = Note::new(assets, metadata, recipient);

    println!("Make a move note ID: {:?}", make_a_move_note.id().to_hex());

    // build and submit transaction
    let note_request = TransactionRequestBuilder::new()
        .own_output_notes(vec![OutputNote::Full(make_a_move_note.clone())])
        .build()
        .unwrap();
    let tx_result = client
        .new_transaction(alice_account.id(), note_request)
        .await
        .unwrap();
    let _ = client.submit_transaction(tx_result.clone()).await;
    client.sync_state().await?;

    // -------------------------------------------------------------------------
    // STEP 5: Call the Game Contract with "make a move" note
    // -------------------------------------------------------------------------
    println!("\n[STEP 5] Call Game Contract With 'make a move' note");

    println!("Consuming note as beneficiary");
    let consume_custom_request = TransactionRequestBuilder::new()
        .unauthenticated_input_notes([(make_a_move_note, None)])
        .build()
        .unwrap();
    let tx_result = client
        .new_transaction(game_contract.id(), consume_custom_request)
        .await
        .unwrap();
    let _ = client.submit_transaction(tx_result.clone()).await;
    client.sync_state().await?;

    let account = client.get_account(game_contract.id()).await.unwrap();
    println!(
        "game contract storage: {:?}",
        account.unwrap().account().storage().get_item(0)
    );

    Ok(())
}
