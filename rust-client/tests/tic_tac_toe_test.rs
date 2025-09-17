use anyhow::Result;
use miden_crypto::Word;
use miden_lib::account::auth::AuthRpoFalcon512;
use rand::{rngs::StdRng, RngCore};
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
    Client, ClientError, Felt, ScriptBuilder,
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

async fn create_basic_account(
    client: &mut Client<FilesystemKeyStore<rand::prelude::StdRng>>,
    keystore: FilesystemKeyStore<StdRng>,
) -> Result<miden_client::account::Account, ClientError> {
    let mut init_seed = [0_u8; 32];
    client.rng().fill_bytes(&mut init_seed);

    let key_pair = SecretKey::with_rng(client.rng());
    let builder = AccountBuilder::new(init_seed)
        .account_type(AccountType::RegularAccountUpdatableCode)
        .storage_mode(AccountStorageMode::Public)
        .with_auth_component(AuthRpoFalcon512::new(key_pair.public_key()))
        .with_component(BasicWallet);
    let (account, seed) = builder.build().unwrap();
    client.add_account(&account, Some(seed), false).await?;
    keystore
        .add_key(&AuthSecretKey::RpoFalcon512(key_pair))
        .unwrap();

    Ok(account)
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
    let _ = client.submit_transaction(tx_result).await;

    client.sync_state().await.unwrap();

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
) -> Result<(), ClientError> {
    let consume_request = TransactionRequestBuilder::new()
        .unauthenticated_input_notes([(note, None)])
        .build()
        .unwrap();
    let tx_result = client
        .new_transaction(game_contract.id(), consume_request)
        .await
        .unwrap();
    let _ = client.submit_transaction(tx_result.clone()).await.unwrap();
    client.sync_state().await?;
    Ok(())
}

#[tokio::test]
async fn test_tic_tac_toe_game() -> Result<()> {
    // Initialize client
    let endpoint = Endpoint::testnet();
    let timeout_ms = 10_000;
    let rpc_api = Arc::new(TonicRpcClient::new(&endpoint, timeout_ms));

    let mut client = ClientBuilder::new()
        .rpc(rpc_api)
        .filesystem_keystore("./keystore")
        .in_debug_mode(true.into())
        .build()
        .await?;

    let sync_summary = client.sync_state().await.unwrap();
    println!("Latest block: {}", sync_summary.block_num);

    // -------------------------------------------------------------------------
    // STEP 1: Create Alice and Bob accounts (players)
    // -------------------------------------------------------------------------
    println!("\n[STEP 1] Creating Alice and Bob accounts");

    let keystore: FilesystemKeyStore<rand::prelude::StdRng> =
        FilesystemKeyStore::new("./keystore".into()).unwrap();

    let alice_account = create_basic_account(&mut client, keystore.clone())
        .await
        .unwrap();

    let bob_account = create_basic_account(&mut client, keystore.clone())
        .await
        .unwrap();

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

    let game_contract = create_game_contract(&mut client, &game_code).await?;

    println!(
        "game_contract id: {:?}",
        Address::from(AccountIdAddress::new(
            game_contract.id(),
            AddressInterface::Unspecified
        ))
        .to_bech32(NetworkId::Testnet)
    );

    // Deploy the contract
    deploy_game_contract(&mut client, &game_contract, &game_code).await?;

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
    consume_note(&mut client, &game_contract, create_game_note).await?;
    println!("Consumed create game note");

    // -------------------------------------------------------------------------
    // STEP 4: Play the game with 3 moves per player (6 moves total)
    // -------------------------------------------------------------------------
    println!("\n[STEP 4] Playing the game with 3 moves per player");

    let make_a_move_note_code =
        fs::read_to_string(Path::new("../masm/notes/make_a_move_note.masm")).unwrap();

    // Define the moves: [field_index, nonce] for each move
    // Alice (player 1) moves: positions 0, 1, 2
    // Bob (player 2) moves: positions 3, 4, 5
    let moves = vec![
        (0, 1), // Alice: position 0, nonce 1
        (3, 2), // Bob: position 3, nonce 2
        (1, 3), // Alice: position 1, nonce 3
        (4, 4), // Bob: position 4, nonce 4
        (2, 5), // Alice: position 2, nonce 5
        (5, 6), // Bob: position 5, nonce 6
    ];

    for (move_index, (field_index, nonce)) in moves.iter().enumerate() {
        let player = if move_index % 2 == 0 { "Alice" } else { "Bob" };
        let player_account = if move_index % 2 == 0 {
            &alice_account
        } else {
            &bob_account
        };

        println!(
            "\n[Move {}] {} making move at position {} with nonce {}",
            move_index + 1,
            player,
            field_index,
            nonce
        );

        let move_inputs = vec![Felt::new(*field_index), Felt::new(*nonce)];

        let move_note = create_and_submit_note(
            &mut client,
            player_account,
            &make_a_move_note_code,
            move_inputs,
            &game_code,
        )
        .await?;

        println!("Move note ID: {:?}", move_note.id().to_hex());

        // Consume the move note
        consume_note(&mut client, &game_contract, move_note).await?;
        println!("Consumed move note for {}", player);

        // Small delay to ensure proper sequencing
        sleep(Duration::from_millis(500)).await;
    }

    // -------------------------------------------------------------------------
    // STEP 5: Check final game state
    // -------------------------------------------------------------------------
    println!("\n[STEP 5] Checking final game state");

    // Retrieve updated contract data
    let account = client.get_account(game_contract.id()).await.unwrap();
    let account_data = account.unwrap().account().clone();

    println!(
        "nonce storage slot: {:?}",
        account_data.storage().get_item(0)
    );
    println!(
        "player ids mapping storage slot: {:?}",
        account_data.storage().get_map_item(
            1,
            Word::new([Felt::new(0), Felt::new(0), Felt::new(0), Felt::new(1)].into())
        )
    );
    println!(
        "player1 values mapping storage slot: {:?}",
        account_data.storage().get_item(2)
    );
    println!(
        "player2 values mapping storage slot: {:?}",
        account_data.storage().get_item(3)
    );
    println!(
        "winners mapping storage slot: {:?}",
        account_data.storage().get_item(4)
    );
    println!(
        "winner lines mapping storage slot: {:?}",
        account_data.storage().get_item(5)
    );

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

    println!("\nTest completed successfully! Game played with 3 moves per player.");
    Ok(())
}
