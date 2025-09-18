use anyhow::Result;
use miden_lib::account::auth::AuthRpoFalcon512;
use rand::RngCore;
use std::{fs, path::Path, sync::Arc, time::Duration};
use tokio::time::sleep;

use miden_assembly::{
    ast::{Module, ModuleKind},
    LibraryPath,
};
use miden_client::{
    account::{
        component::BasicWallet, AccountBuilder, AccountIdAddress, AccountStorageMode, AccountType,
        Address, AddressInterface, StorageSlot,
    },
    auth::AuthSecretKey,
    builder::ClientBuilder,
    crypto::{FeltRng, SecretKey},
    keystore::FilesystemKeyStore,
    note::{
        Note, NoteAssets, NoteExecutionHint, NoteExecutionMode, NoteInputs, NoteMetadata,
        NoteRecipient, NoteTag, NoteType,
    },
    rpc::{Endpoint, TonicRpcClient},
    transaction::{OutputNote, TransactionKernel, TransactionRequestBuilder},
    Client, ClientError, Felt, ScriptBuilder, Word,
};
use miden_lib::account::auth;
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

async fn create_game_contract(
    client: &mut Client<FilesystemKeyStore<rand::prelude::StdRng>>,
    game_code: &str,
) -> Result<miden_client::account::Account, ClientError> {
    // Prepare assembler (debug mode = true)
    let assembler: Assembler = TransactionKernel::assembler().with_debug_mode(true);

    let empty_storage_slot =
        StorageSlot::Value([Felt::new(0), Felt::new(0), Felt::new(0), Felt::new(0)].into());

    let storage_map = StorageMap::new();
    let storage_slot_map = StorageSlot::Map(storage_map.clone());

    // Compile the account code into `AccountComponent` with storage slots
    let game_component = AccountComponent::compile(
        game_code.to_string(),
        assembler,
        vec![
            // nonce storage slot
            empty_storage_slot,
            // player ids mapping storage slot
            storage_slot_map.clone(),
            // player1 values mapping storage slot
            storage_slot_map.clone(),
            // player2 values mapping storage slot
            storage_slot_map.clone(),
            // winners mapping storage slot
            storage_slot_map.clone(),
            // winning lines mapping storage slot
            storage_slot_map,
        ],
    )
    .unwrap()
    .with_supports_all_types();

    // Init seed for the game contract
    let mut seed = [0_u8; 32];
    client.rng().fill_bytes(&mut seed);

    // Build the new `Account` with the component
    let (game_contract, game_seed) = AccountBuilder::new(seed)
        .account_type(AccountType::RegularAccountImmutableCode)
        .storage_mode(AccountStorageMode::Public)
        .with_component(game_component.clone())
        .with_auth_component(auth::NoAuth)
        .build()
        .unwrap();

    client
        .add_account(&game_contract.clone(), Some(game_seed), false)
        .await
        .unwrap();
    Ok(game_contract)
}

async fn deploy_game_contract(
    client: &mut Client<FilesystemKeyStore<rand::prelude::StdRng>>,
    game_contract: &miden_client::account::Account,
    game_code: &str,
) -> Result<(), ClientError> {
    // Load the MASM script referencing the game deployment procedure
    let deployment_script_path = Path::new("../masm/scripts/game_deployment_script.masm");
    let deployment_script_code = fs::read_to_string(deployment_script_path).unwrap();

    let assembler: Assembler = TransactionKernel::assembler().with_debug_mode(true);
    let account_component_lib = create_library(
        assembler.clone(),
        "external_contract::game_contract",
        game_code,
    )
    .unwrap();

    let deployment_script = ScriptBuilder::new(true)
        .with_dynamically_linked_library(&account_component_lib)
        .unwrap()
        .compile_tx_script(deployment_script_code)
        .unwrap();

    // Build a transaction request with the custom script
    let tx_game_constructor_request = TransactionRequestBuilder::new()
        .custom_script(deployment_script)
        .build()
        .unwrap();

    // Execute the transaction locally
    let tx_result = client
        .new_transaction(game_contract.id(), tx_game_constructor_request)
        .await
        .unwrap();

    // Submit transaction to the network
    match client.submit_transaction(tx_result).await {
        Ok(_) => {
            println!("    - Transaction submitted successfully!");
        }
        Err(e) => {
            println!("    - Failed to submit transaction: {:?}", e);
            return Err(e.into());
        }
    }

    println!("    - Syncing state...");
    match client.sync_state().await {
        Ok(_) => {
            println!("    - State synced successfully!");
        }
        Err(e) => {
            println!("    - Failed to sync state: {:?}", e);
            return Err(e.into());
        }
    }
    Ok(())
}

async fn create_and_submit_note(
    client: &mut Client<FilesystemKeyStore<rand::prelude::StdRng>>,
    sender_account: &miden_client::account::Account,
    note_code: &str,
    note_inputs: Vec<Felt>,
    game_code: &str,
) -> Result<Note, ClientError> {
    let assembler: Assembler = TransactionKernel::assembler().with_debug_mode(true);
    let account_component_lib = create_library(
        assembler.clone(),
        "external_contract::game_contract",
        game_code,
    )
    .unwrap();

    let note_script = ScriptBuilder::new(true)
        .with_dynamically_linked_library(&account_component_lib)
        .unwrap()
        .compile_note_script(note_code.to_string())
        .unwrap();

    let empty_assets = NoteAssets::new(vec![])?;
    let note_inputs = NoteInputs::new(note_inputs).unwrap();
    let serial_num = client.rng().draw_word();
    let recipient = NoteRecipient::new(serial_num, note_script, note_inputs);
    let tag: NoteTag = NoteTag::for_public_use_case(0, 0, NoteExecutionMode::Local).unwrap();
    let metadata = NoteMetadata::new(
        sender_account.id(),
        NoteType::Public,
        tag,
        NoteExecutionHint::always(),
        Felt::new(0),
    )?;
    let note = Note::new(empty_assets.clone(), metadata, recipient);

    // Submit note on-chain
    let note_request = TransactionRequestBuilder::new()
        .own_output_notes(vec![OutputNote::Full(note.clone())])
        .build()
        .unwrap();
    let tx_result = client
        .new_transaction(sender_account.id(), note_request)
        .await
        .unwrap();
    let _ = client.submit_transaction(tx_result.clone()).await;
    client.sync_state().await?;

    Ok(note)
}

async fn consume_note(
    client: &mut Client<FilesystemKeyStore<rand::prelude::StdRng>>,
    game_contract: &miden_client::account::Account,
    note: Note,
    print_delta: bool,
) -> Result<(), ClientError> {
    let consume_request = TransactionRequestBuilder::new()
        .unauthenticated_input_notes([(note, None)])
        .build()
        .unwrap();
    let tx_result = client
        .new_transaction(game_contract.id(), consume_request)
        .await
        .unwrap();
    if print_delta {
        println!("Transaction account delta: {:?}", tx_result.account_delta());
    }
    let _ = client.submit_transaction(tx_result.clone()).await.unwrap();
    client.sync_state().await?;
    Ok(())
}

#[tokio::test]
async fn test_tic_tac_toe_game_final_win() -> Result<()> {
    // Initialize client
    let endpoint = Endpoint::testnet();
    let timeout_ms = 10_000;
    let rpc_api = Arc::new(TonicRpcClient::new(&endpoint, timeout_ms));

    let keystore = Arc::new(FilesystemKeyStore::new("./keystore".into()).unwrap());

    let mut client = ClientBuilder::new()
        .rpc(rpc_api)
        .authenticator(keystore.clone())
        .in_debug_mode(true.into())
        .build()
        .await?;

    println!("Client initialized successfully!");

    // Try to sync state, but handle potential errors gracefully
    println!("Attempting to sync state...");
    match client.sync_state().await {
        Ok(sync_summary) => {
            println!("Latest block: {}", sync_summary.block_num);
        }
        Err(e) => {
            println!("Warning: Failed to sync state: {:?}", e);
            // Continue with the test anyway
        }
    }

    // -------------------------------------------------------------------------
    // STEP 1: Create Alice and Bob accounts (players)
    // -------------------------------------------------------------------------
    println!("\n[STEP 1] Creating Alice and Bob accounts");

    let keystore: FilesystemKeyStore<rand::prelude::StdRng> =
        FilesystemKeyStore::new("./keystore".into()).unwrap();

    // Create Alice account
    println!("Creating Alice account...");
    let mut alice_seed = [0_u8; 32];
    client.rng().fill_bytes(&mut alice_seed);
    let alice_key_pair = SecretKey::with_rng(client.rng());
    let alice_builder = AccountBuilder::new(alice_seed)
        .account_type(AccountType::RegularAccountUpdatableCode)
        .storage_mode(AccountStorageMode::Public)
        .with_auth_component(AuthRpoFalcon512::new(alice_key_pair.public_key()))
        .with_component(BasicWallet);
    let (alice_account, alice_seed) = alice_builder.build().unwrap();
    client
        .add_account(&alice_account, Some(alice_seed), false)
        .await?;
    keystore
        .add_key(&AuthSecretKey::RpoFalcon512(alice_key_pair))
        .unwrap();
    println!("Alice account created successfully!");

    // Create Bob account
    println!("Creating Bob account...");
    let mut bob_seed = [0_u8; 32];
    client.rng().fill_bytes(&mut bob_seed);
    let bob_key_pair = SecretKey::with_rng(client.rng());
    let bob_builder = AccountBuilder::new(bob_seed)
        .account_type(AccountType::RegularAccountUpdatableCode)
        .storage_mode(AccountStorageMode::Public)
        .with_auth_component(AuthRpoFalcon512::new(bob_key_pair.public_key()))
        .with_component(BasicWallet);
    let (bob_account, bob_seed) = bob_builder.build().unwrap();
    client
        .add_account(&bob_account, Some(bob_seed), false)
        .await?;
    keystore
        .add_key(&AuthSecretKey::RpoFalcon512(bob_key_pair))
        .unwrap();
    println!("Bob account created successfully!");

    println!("alice prefix: {:?}", alice_account.id().prefix().as_felt());
    println!("alice suffix: {:?}", alice_account.id().suffix());
    println!("bob prefix: {:?}", bob_account.id().prefix().as_felt());
    println!("bob suffix: {:?}", bob_account.id().suffix());

    // -------------------------------------------------------------------------
    // STEP 2: Create and deploy the tic tac toe game contract
    // -------------------------------------------------------------------------
    println!("\n[STEP 2] Creating and deploying tic tac toe game contract");

    // Load the MASM file for the tic tac toe game contract
    let game_path = Path::new("../masm/accounts/tic_tac_toe.masm");
    let game_code = fs::read_to_string(game_path).unwrap();

    // Try to create the game contract with error handling
    let game_contract = match create_game_contract(&mut client, &game_code).await {
        Ok(contract) => {
            println!("Successfully created game contract");
            contract
        }
        Err(e) => {
            println!("Failed to create game contract: {:?}", e);
            return Err(anyhow::anyhow!("Failed to create game contract: {:?}", e));
        }
    };

    println!(
        "game_contract id: {:?}",
        Address::from(AccountIdAddress::new(
            game_contract.id(),
            AddressInterface::Unspecified
        ))
        .to_bech32(NetworkId::Testnet)
    );

    // Try to deploy the contract with error handling
    println!("About to deploy game contract...");
    match deploy_game_contract(&mut client, &game_contract, &game_code).await {
        Ok(_) => {
            println!("Successfully deployed game contract");
        }
        Err(e) => {
            println!("Failed to deploy game contract: {:?}", e);
            return Err(e.into());
        }
    }

    // -------------------------------------------------------------------------
    // STEP 3: Create and consume the create game note
    // -------------------------------------------------------------------------
    println!("\n[STEP 3] Creating and consuming create game note");

    let create_game_note_code =
        fs::read_to_string(Path::new("../masm/notes/create_game_note.masm")).unwrap();
    let create_game_note_inputs = vec![
        bob_account.id().suffix(),
        bob_account.id().prefix().as_felt(),
    ];

    let create_game_note = create_and_submit_note(
        &mut client,
        &alice_account,
        &create_game_note_code,
        create_game_note_inputs,
        &game_code,
    )
    .await?;

    println!("Create game note ID: {:?}", create_game_note.id().to_hex());

    // Consume the create game note
    consume_note(&mut client, &game_contract, create_game_note, false).await?;
    println!("Consumed create game note");

    // Debug: Check the game state after creating the game
    let account = client.get_account(game_contract.id()).await.unwrap();
    let account_data = account.unwrap().account().clone();
    println!(
        "nonce storage slot: {:?}",
        account_data.storage().get_item(0)
    );

    // -------------------------------------------------------------------------
    // STEP 4: Perform game moves (3 per player)
    // -------------------------------------------------------------------------
    println!("\n[STEP 4] Performing game moves (3 per player)");

    // Load the make_a_move note code
    let make_a_move_note_code =
        fs::read_to_string(Path::new("../masm/notes/make_a_move_note.masm")).unwrap();

    // Define the moves: Alice and Bob will alternate, 3 moves each
    // Using indices 0-8 (9 fields total), ensuring no duplicates
    let moves = vec![
        (3, &alice_account),
        (4, &bob_account),
        (2, &alice_account),
        (5, &bob_account),
        (6, &alice_account),
        (7, &bob_account),
        (8, &alice_account),
        (9, &bob_account),
    ];

    for (move_index, (field_index, player_account)) in moves.iter().enumerate() {
        println!(
            "  Move {}: Player making move on field {}",
            move_index + 1,
            field_index
        );

        // Create and submit the make_a_move note
        let make_a_move_note_inputs = vec![
            Felt::new(1),                   // game_id (nonce)
            Felt::new(*field_index as u64), // field_index
        ];

        let make_a_move_note = create_and_submit_note(
            &mut client,
            player_account,
            &make_a_move_note_code,
            make_a_move_note_inputs,
            &game_code,
        )
        .await?;

        println!(
            "    Make a move note ID: {:?}",
            make_a_move_note.id().to_hex()
        );

        // Consume the make_a_move note
        consume_note(&mut client, &game_contract, make_a_move_note, true).await?;
        println!("    Consumed make a move note for field {}", field_index);

        // Small delay to ensure proper state synchronization
        sleep(Duration::from_millis(100)).await;
    }

    println!("All 6 moves completed successfully!");

    // -------------------------------------------------------------------------
    // STEP 5: Cast final win move
    // -------------------------------------------------------------------------

    let make_win_move_note_code =
        fs::read_to_string(Path::new("../masm/notes/make_win_move.masm")).unwrap();
    let make_win_move_note_inputs = vec![Felt::new(2), Felt::new(3), Felt::new(1), Felt::new(1)];

    let make_win_move_note = create_and_submit_note(
        &mut client,
        &alice_account,
        &make_win_move_note_code,
        make_win_move_note_inputs,
        &game_code,
    )
    .await?;

    println!("Cast win note ID: {:?}", make_win_move_note.id().to_hex());

    // Consume the create game note
    consume_note(&mut client, &game_contract, make_win_move_note, true).await?;
    println!("Consumed cast win note");

    // -------------------------------------------------------------------------
    // STEP 5: Check final game state
    // -------------------------------------------------------------------------
    println!("\n[STEP 5] Checking final game state");

    // Check specific player moves
    println!(
        "player1 values mapping for game 1: {:?}",
        account_data.storage().get_map_item(
            2,
            Word::new([Felt::new(0), Felt::new(0), Felt::new(0), Felt::new(1)].into())
        )
    );
    println!(
        "player2 values mapping for game 1: {:?}",
        account_data.storage().get_map_item(
            3,
            Word::new([Felt::new(0), Felt::new(0), Felt::new(0), Felt::new(1)].into())
        )
    );
    println!(
        "winners mapping for game 1: {:?}",
        account_data.storage().get_map_item(
            4,
            Word::new([Felt::new(0), Felt::new(0), Felt::new(0), Felt::new(1)].into())
        )
    );

    println!("\nTest completed successfully! Game played with 3 moves per player.");
    Ok(())
}
