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

#[tokio::main]
async fn main() -> Result<(), ClientError> {
    // Initialize client
    // let endpoint = Endpoint::new("http".to_string(), "localhost".to_string(), Some(57291));
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

    let keystore: FilesystemKeyStore<rand::prelude::StdRng> =
        FilesystemKeyStore::new("./keystore".into()).unwrap();

    let alice_account = create_basic_account(&mut client, keystore.clone())
        .await
        .unwrap();

    let bob_account = create_basic_account(&mut client, keystore.clone())
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

    let empty_storage_slot =
        StorageSlot::Value([Felt::new(0), Felt::new(0), Felt::new(0), Felt::new(0)].into());

    let storage_map = StorageMap::new();
    let storage_slot_map = StorageSlot::Map(storage_map.clone());

    // Compile the account code into `AccountComponent` with one storage slot
    let game_component = AccountComponent::compile(
        game_code.clone(),
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

    // Init seed for the counter contract
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

    println!(
        "game_contract id: {:?}",
        Address::from(AccountIdAddress::new(
            game_contract.id(),
            AddressInterface::Unspecified
        ))
        .to_bech32(NetworkId::Testnet)
    );

    client
        .add_account(&game_contract.clone(), Some(game_seed), false)
        .await
        .unwrap();

    // -------------------------------------------------------------------------
    // STEP 3: Call the Game Contract with the constructor
    // -------------------------------------------------------------------------
    println!("\n[STEP 3] Call Game Contract Constructor");

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

    // Retrieve updated contract data to see the incremented game
    let mut account = client.get_account(game_contract.id()).await.unwrap();
    let mut account_data = account.unwrap().account().clone();
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

    // -------------------------------------------------------------------------
    // STEP 4: Call the Game Contract with a create game note
    // -------------------------------------------------------------------------
    println!("\n[STEP 4] Compose create game note");

    let assembler: Assembler = TransactionKernel::assembler().with_debug_mode(true);
    let account_component_lib = create_library(
        assembler.clone(),
        "external_contract::game_contract",
        &game_code,
    )
    .unwrap();

    let note_code = fs::read_to_string(Path::new("../masm/notes/create_game_note.masm")).unwrap();
    let note_script = ScriptBuilder::new(true)
        .with_dynamically_linked_library(&account_component_lib)
        .unwrap()
        .compile_note_script(note_code)
        .unwrap();

    let empty_assets = NoteAssets::new(vec![])?;

    let note_inputs = NoteInputs::new(vec![
        bob_account.id().suffix(),
        bob_account.id().prefix().as_felt(),
    ])
    .unwrap();
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
    let create_game_note = Note::new(empty_assets.clone(), metadata, recipient);

    println!("Create game note ID: {:?}", create_game_note.id().to_hex());

    // -------------------------------------------------------------------------
    // STEP 5: Submit create game note on-chain
    // -------------------------------------------------------------------------
    println!("\n[STEP 5] Submit create game note on-chain");

    // build and submit transaction
    let note_request = TransactionRequestBuilder::new()
        .own_output_notes(vec![OutputNote::Full(create_game_note.clone())])
        .build()
        .unwrap();
    let tx_result = client
        .new_transaction(alice_account.id(), note_request)
        .await
        .unwrap();
    let _ = client.submit_transaction(tx_result.clone()).await;
    client.sync_state().await?;

    println!("Submitted create game note");

    // -------------------------------------------------------------------------
    // STEP 6: Call Game Contract with create game note
    // -------------------------------------------------------------------------
    println!("\n[STEP 6] Call Game Contract with create game note");

    println!("Consuming create game note as beneficiary");
    let consume_custom_request = TransactionRequestBuilder::new()
        .unauthenticated_input_notes([(create_game_note, None)])
        .build()
        .unwrap();
    let tx_result = client
        .new_transaction(game_contract.id(), consume_custom_request)
        .await
        .unwrap();
    let _ = client.submit_transaction(tx_result.clone()).await;
    client.sync_state().await?;

    // -------------------------------------------------------------------------
    // STEP 7: Call the Game Contract with make a move note
    // -------------------------------------------------------------------------
    println!("\n[STEP 7] Compose make a move note");

    let assembler: Assembler = TransactionKernel::assembler().with_debug_mode(true);
    let account_component_lib = create_library(
        assembler.clone(),
        "external_contract::game_contract",
        &game_code,
    )
    .unwrap();

    let make_a_move_note_code =
        fs::read_to_string(Path::new("../masm/notes/make_a_move_note.masm")).unwrap();
    let make_a_move_note_script = ScriptBuilder::new(true)
        .with_dynamically_linked_library(&account_component_lib)
        .unwrap()
        .compile_note_script(make_a_move_note_code)
        .unwrap();

    let make_a_move_note_inputs = NoteInputs::new(vec![Felt::new(1), Felt::new(7)]).unwrap();
    let make_a_move_note_serial_num = client.rng().draw_word();
    let make_a_move_note_recipient = NoteRecipient::new(
        make_a_move_note_serial_num,
        make_a_move_note_script,
        make_a_move_note_inputs,
    );
    let make_a_move_note_tag =
        NoteTag::for_public_use_case(0, 0, NoteExecutionMode::Local).unwrap();
    let make_a_move_note_metadata = NoteMetadata::new(
        alice_account.id(),
        NoteType::Public,
        make_a_move_note_tag,
        NoteExecutionHint::always(),
        Felt::new(0),
    )?;
    let make_a_move_note = Note::new(
        empty_assets.clone(),
        make_a_move_note_metadata,
        make_a_move_note_recipient,
    );

    println!("Make a move note ID: {:?}", make_a_move_note.id().to_hex());

    // -------------------------------------------------------------------------
    // STEP 8: Submit make a move note on-chain
    // -------------------------------------------------------------------------
    println!("\n[STEP 8] Submit make a move note on-chain");

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

    println!("Submitted make a move note");

    // -------------------------------------------------------------------------
    // STEP 9: Consume the make a move note
    // -------------------------------------------------------------------------
    println!("\n[STEP 9] Consume the make a move note");

    let consume_make_a_move_note_request = TransactionRequestBuilder::new()
        .unauthenticated_input_notes([(make_a_move_note, None)])
        .build()
        .unwrap();
    let tx_result = client
        .new_transaction(game_contract.id(), consume_make_a_move_note_request)
        .await
        .unwrap();
    let _ = client.submit_transaction(tx_result.clone()).await.unwrap();

    let make_a_move_note_tx_id = tx_result.executed_transaction().id();
    println!(
        "View transaction on MidenScan: https://testnet.midenscan.com/tx/{:?}",
        make_a_move_note_tx_id
    );

    println!("Transaction account delta: {:?}", tx_result.account_delta());

    sleep(Duration::from_secs(6)).await;
    client.sync_state().await.unwrap();

    account = client.get_account(game_contract.id()).await.unwrap();
    account_data = account.unwrap().account().clone();

    println!("Consumed make a move note");

    println!(
        "player1 values mapping storage slot: {:?}",
        account_data.storage().get_item(2)
    );

    println!(
        "player1 values mapping storage slot: {:?}",
        account_data.storage().get_map_item(
            2,
            Word::new([Felt::new(0), Felt::new(0), Felt::new(0), Felt::new(1)].into())
        )
    );

    Ok(())
}
