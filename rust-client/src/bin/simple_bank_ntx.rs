use miden_lib::account::wallets::BasicWallet;
use rand::{rngs::StdRng, RngCore};
use std::{fs, path::Path, sync::Arc};
use tokio::time::{sleep, Duration};

use miden_assembly::{
    ast::{Module, ModuleKind},
    LibraryPath,
};
use miden_client::{
    account::{
        component::{BasicFungibleFaucet, RpoFalcon512},
        Account, AccountBuilder, AccountId, AccountStorageMode, AccountType, StorageMap,
        StorageSlot,
    },
    asset::{FungibleAsset, TokenSymbol},
    auth::AuthSecretKey,
    builder::ClientBuilder,
    crypto::{FeltRng, SecretKey},
    keystore::FilesystemKeyStore,
    note::{
        create_p2id_note, Note, NoteAssets, NoteExecutionHint, NoteExecutionMode, NoteInputs,
        NoteMetadata, NoteRecipient, NoteRelevance, NoteScript, NoteTag, NoteType,
    },
    rpc::{Endpoint, TonicRpcClient},
    store::{InputNoteRecord, NoteFilter, TransactionFilter},
    transaction::{
        OutputNote, TransactionId, TransactionKernel, TransactionRequestBuilder, TransactionStatus,
    },
    Client, ClientError, Felt, Word,
};
use miden_lib::account::auth::NoAuth;
use miden_lib::utils::ScriptBuilder;
use miden_objects::{
    account::{AccountComponent, NetworkId},
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

// Helper to create a basic account
async fn create_basic_account(
    client: &mut Client,
    keystore: FilesystemKeyStore<StdRng>,
) -> Result<Account, ClientError> {
    let mut init_seed = [0_u8; 32];
    client.rng().fill_bytes(&mut init_seed);

    let key_pair = SecretKey::with_rng(client.rng());
    let builder = AccountBuilder::new(init_seed)
        .account_type(AccountType::RegularAccountUpdatableCode)
        .storage_mode(AccountStorageMode::Public)
        .with_auth_component(RpoFalcon512::new(key_pair.public_key()))
        .with_component(BasicWallet);
    let (account, seed) = builder.build().unwrap();
    client.add_account(&account, Some(seed), false).await?;
    keystore
        .add_key(&AuthSecretKey::RpoFalcon512(key_pair))
        .unwrap();

    Ok(account)
}

async fn create_basic_faucet(
    client: &mut Client,
    keystore: FilesystemKeyStore<StdRng>,
) -> Result<Account, ClientError> {
    let mut init_seed = [0u8; 32];
    client.rng().fill_bytes(&mut init_seed);
    let key_pair = SecretKey::with_rng(client.rng());
    let symbol = TokenSymbol::new("MID").unwrap();
    let decimals = 8;
    let max_supply = Felt::new(1_000_000);
    let builder = AccountBuilder::new(init_seed)
        .account_type(AccountType::FungibleFaucet)
        .storage_mode(AccountStorageMode::Public)
        .with_auth_component(RpoFalcon512::new(key_pair.public_key()))
        .with_component(BasicFungibleFaucet::new(symbol, decimals, max_supply).unwrap());
    let (account, seed) = builder.build().unwrap();
    client.add_account(&account, Some(seed), false).await?;
    keystore
        .add_key(&AuthSecretKey::RpoFalcon512(key_pair))
        .unwrap();
    Ok(account)
}

/// Waits for a specific transaction to be committed.
pub async fn wait_for_note(
    client: &mut Client,
    account_id: Option<Account>,
    expected: &Note,
    tx_id: TransactionId,
) -> Result<(), ClientError> {
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
            println!("✅ Transaction committed successfully");
            break;
        } else {
            println!("⏳ Waiting for transaction commitment...");
        }

        sleep(Duration::from_secs(2)).await;
    }

    Ok(())
}

/// Waits for a specific transaction to be committed.
pub async fn wait_for_tx(client: &mut Client, tx_id: TransactionId) -> Result<(), ClientError> {
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
            println!("✅ Transaction committed successfully");
            break;
        } else {
            println!("⏳ Waiting for transaction commitment...");
        }

        sleep(Duration::from_secs(2)).await;
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize client & keystore
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

    let keystore = FilesystemKeyStore::new("./keystore".into()).unwrap();

    // -------------------------------------------------------------------------
    // STEP 1: Create accounts and deploy faucet
    // -------------------------------------------------------------------------
    println!("\n[STEP 1] Creating new accounts");
    let alice_account = create_basic_account(&mut client, keystore.clone()).await?;
    let alice_account_id = alice_account.id();
    println!(
        "Alice's account ID: {:?}",
        alice_account_id.to_bech32(NetworkId::Testnet)
    );

    println!("\nDeploying a new fungible faucet.");
    let faucet = create_basic_faucet(&mut client, keystore.clone()).await?;
    println!(
        "Faucet account ID: {:?}",
        faucet.id().to_bech32(NetworkId::Testnet)
    );
    client.sync_state().await?;

    // -------------------------------------------------------------------------
    // STEP 2: Create deposit_withdraw contract (NETWORK ACCOUNT)
    // -------------------------------------------------------------------------
    println!("\n[STEP 2] Creating deposit_withdraw network contract.");

    // Prepare assembler (debug mode = true)
    let assembler: Assembler = TransactionKernel::assembler().with_debug_mode(true);

    // Load the MASM file for the deposit_withdraw contract
    let contract_path = Path::new("../masm/accounts/deposit_withdraw.masm");
    let contract_code = fs::read_to_string(contract_path).unwrap();

    let storage_map = StorageMap::new();
    let storage_slot_map = StorageSlot::Map(storage_map.clone());

    // Compile the account code into `AccountComponent` with one storage slot for balance
    let contract_component =
        AccountComponent::compile(contract_code.clone(), assembler, vec![storage_slot_map])
            .unwrap()
            .with_supports_all_types();

    // Init seed for the deposit_withdraw contract
    let mut seed = [0_u8; 32];
    client.rng().fill_bytes(&mut seed);

    // Build the new `Account` with the component - NETWORK ACCOUNT
    let (deposit_contract, contract_seed) = AccountBuilder::new(seed)
        .account_type(AccountType::RegularAccountImmutableCode)
        .storage_mode(AccountStorageMode::Network) // NETWORK STORAGE
        .with_component(BasicWallet)
        .with_component(contract_component.clone())
        .with_auth_component(NoAuth) // NO AUTH FOR NETWORK ACCOUNTS
        .build()
        .unwrap();

    println!(
        "deposit_contract commitment: {:?}",
        deposit_contract.commitment()
    );
    println!(
        "deposit_contract id: {:?}",
        deposit_contract.id().to_bech32(NetworkId::Testnet)
    );
    println!("deposit_contract storage: {:?}", deposit_contract.storage());

    client
        .add_account(&deposit_contract.clone(), Some(contract_seed), false)
        .await
        .unwrap();

    // -------------------------------------------------------------------------
    // STEP 3: Deploy Network Contract with Transaction Script
    // -------------------------------------------------------------------------
    println!("\n[STEP 3] Deploy network deposit_withdraw smart contract");

    let script_code = fs::read_to_string(Path::new(
        "../masm/scripts/deploy_deposit_withdraw_script.masm",
    ))
    .unwrap();

    let library_path = "external_contract::deposit_withdraw_contract";
    let contract_lib = create_library(
        TransactionKernel::assembler().with_debug_mode(true),
        library_path,
        &contract_code,
    )
    .unwrap();

    let tx_script = ScriptBuilder::default()
        .with_dynamically_linked_library(&contract_lib)
        .map_err(|e| format!("Failed to link library: {}", e))?
        .compile_tx_script(script_code)
        .map_err(|e| format!("Failed to compile script: {}", e))?;

    let tx_deploy_request = TransactionRequestBuilder::new()
        .custom_script(tx_script)
        .build()
        .unwrap();

    let tx_result = client
        .new_transaction(deposit_contract.id(), tx_deploy_request)
        .await
        .unwrap();

    let _ = client.submit_transaction(tx_result.clone()).await;

    let tx_id = tx_result.executed_transaction().id();
    println!(
        "View deployment transaction on MidenScan: https://testnet.midenscan.com/tx/{}",
        tx_id.to_hex()
    );

    // Wait for the deployment transaction to be committed
    wait_for_tx(&mut client, tx_id).await?;

    // -------------------------------------------------------------------------
    // STEP 4: Mint tokens for Alice
    // -------------------------------------------------------------------------
    println!("\n[STEP 4] Mint tokens for Alice");
    let faucet_id = faucet.id();
    let amount: u64 = 100;
    let mint_amount = FungibleAsset::new(faucet_id, amount).unwrap();
    let tx_request = TransactionRequestBuilder::new()
        .build_mint_fungible_asset(
            mint_amount,
            alice_account_id,
            NoteType::Public,
            client.rng(),
        )
        .unwrap();
    let tx_exec = client.new_transaction(faucet.id(), tx_request).await?;
    client.submit_transaction(tx_exec.clone()).await?;

    let p2id_note = if let OutputNote::Full(note) = tx_exec.created_notes().get_note(0) {
        note.clone()
    } else {
        panic!("Expected OutputNote::Full");
    };

    // Wait for the P2ID note to be available
    wait_for_note(
        &mut client,
        Some(alice_account.clone()),
        &p2id_note,
        tx_exec.executed_transaction().id(),
    )
    .await?;

    let consume_request = TransactionRequestBuilder::new()
        .authenticated_input_notes([(p2id_note.id(), None)])
        .build()
        .unwrap();
    let tx_exec = client
        .new_transaction(alice_account_id, consume_request)
        .await?;
    client.submit_transaction(tx_exec).await?;
    client.sync_state().await?;

    // -------------------------------------------------------------------------
    // STEP 5: Create deposit note with assets (TAGGED FOR NETWORK CONTRACT)
    // -------------------------------------------------------------------------
    println!("\n[STEP 5] Create deposit note with assets");

    let assembler = TransactionKernel::assembler().with_debug_mode(true);

    // Create library from the deposit contract code so the note can call its procedures
    let contract_lib = create_library(
        assembler.clone(),
        "external_contract::deposit_withdraw_contract",
        &contract_code,
    )
    .unwrap();

    let note_code =
        fs::read_to_string(Path::new("../masm/notes/deposit_withdraw_note.masm")).unwrap();
    let serial_num = client.rng().draw_word();
    let note_script =
        NoteScript::compile(note_code, assembler.with_library(&contract_lib).unwrap()).unwrap();
    let note_inputs = NoteInputs::new(vec![]).unwrap(); // No special inputs needed
    let recipient = NoteRecipient::new(serial_num, note_script, note_inputs);

    // TAG THE NOTE WITH THE DEPOSIT CONTRACT ID FOR NETWORK ROUTING
    let tag = NoteTag::from_account_id(deposit_contract.id());

    let metadata = NoteMetadata::new(
        alice_account_id,
        NoteType::Public,
        tag,
        NoteExecutionHint::always(),
        Felt::new(0),
    )?;
    let vault = NoteAssets::new(vec![mint_amount.into()])?;
    let deposit_note = Note::new(vault, metadata, recipient);
    println!("deposit network note id: {:?}", deposit_note.id().to_hex());

    let note_request = TransactionRequestBuilder::new()
        .own_output_notes(vec![OutputNote::Full(deposit_note.clone())])
        .build()
        .unwrap();
    let tx_result = client
        .new_transaction(alice_account_id, note_request)
        .await
        .unwrap();
    println!(
        "View transaction on MidenScan: https://testnet.midenscan.com/tx/{}",
        tx_result.executed_transaction().id().to_hex()
    );
    let _ = client.submit_transaction(tx_result.clone()).await;
    client.sync_state().await?;

    wait_for_note(
        &mut client,
        None, // No specific account filter for network notes
        &deposit_note,
        tx_result.executed_transaction().id(),
    )
    .await?;

    // Wait for network to process the tagged note
    println!("Waiting for network to process tagged deposit note...");
    sleep(Duration::from_secs(10)).await;
    client.sync_state().await?;

    // -------------------------------------------------------------------------
    // STEP 6: Check contract state after network processing
    // -------------------------------------------------------------------------
    println!("\n[STEP 6] Checking contract state after network deposit");

    // Retrieve updated contract data to see the balance
    let account = client.get_account(deposit_contract.id()).await.unwrap();
    if let Some(account_data) = account {
        println!("📊 Contract balance updated by network");
        println!(
            "Contract storage: {:?}",
            account_data.account().storage().get_item(0)
        );
    }

    // -------------------------------------------------------------------------
    // STEP 7: Create P2ID withdraw note for Alice
    // -------------------------------------------------------------------------
    println!("\n[STEP 7] Create P2ID withdraw note for Alice");

    // Create a P2ID note with the same asset amount, targeted to Alice
    let withdraw_p2id_note = create_p2id_note(
        deposit_contract.id(),    // sender (the contract)
        alice_account_id,         // target (Alice)
        vec![mint_amount.into()], // same asset that was deposited
        NoteType::Private,
        Felt::new(0),
        client.rng(),
    )
    .unwrap();

    println!(
        "Withdraw P2ID note id: {:?}",
        withdraw_p2id_note.id().to_hex()
    );
    println!("Withdraw note assets: {:?}", withdraw_p2id_note.assets());

    // -------------------------------------------------------------------------
    // STEP 8: Create withdrawal note (TAGGED FOR NETWORK CONTRACT)
    // -------------------------------------------------------------------------
    println!("\n[STEP 8] Create withdrawal note");

    let assembler = TransactionKernel::assembler().with_debug_mode(true);

    // Create library from the deposit contract code so the note can call its procedures
    let contract_lib = create_library(
        assembler.clone(),
        "external_contract::deposit_withdraw_contract",
        &contract_code,
    )
    .unwrap();

    let note_code =
        fs::read_to_string(Path::new("../masm/notes/deposit_withdraw_note.masm")).unwrap();
    let serial_num = client.rng().draw_word();
    let note_script =
        NoteScript::compile(note_code, assembler.with_library(&contract_lib).unwrap()).unwrap();

    let p2id_withdraw_recipient: Word = withdraw_p2id_note.recipient().digest().into();

    let note_inputs = NoteInputs::new(vec![
        p2id_withdraw_recipient[0],
        p2id_withdraw_recipient[1],
        p2id_withdraw_recipient[2],
        p2id_withdraw_recipient[3],
        withdraw_p2id_note.metadata().execution_hint().into(),
        withdraw_p2id_note.metadata().note_type().into(),
        Felt::new(0),
        withdraw_p2id_note.metadata().tag().into(),
    ])
    .unwrap();

    let withdrawal_note_recipient = NoteRecipient::new(serial_num, note_script, note_inputs);

    // TAG THE WITHDRAWAL NOTE WITH THE DEPOSIT CONTRACT ID FOR NETWORK ROUTING
    let tag = NoteTag::from_account_id(deposit_contract.id());

    let metadata = NoteMetadata::new(
        alice_account_id,
        NoteType::Public,
        tag,
        NoteExecutionHint::always(),
        Felt::new(0),
    )?;
    let vault = NoteAssets::new(vec![])?;
    let withdrawal_note = Note::new(vault, metadata, withdrawal_note_recipient);
    println!("withdrawal note id: {:?}", withdrawal_note.id().to_hex());

    let note_request = TransactionRequestBuilder::new()
        .own_output_notes(vec![OutputNote::Full(withdrawal_note.clone())])
        .build()
        .unwrap();
    let tx_result = client
        .new_transaction(alice_account_id, note_request)
        .await
        .unwrap();
    println!(
        "View transaction on MidenScan: https://testnet.midenscan.com/tx/{}",
        tx_result.executed_transaction().id().to_hex()
    );
    let _ = client.submit_transaction(tx_result.clone()).await;
    client.sync_state().await?;

    // Wait for the withdrawal note to be available
    wait_for_note(
        &mut client,
        None, // No specific account filter for network notes
        &withdrawal_note,
        tx_result.executed_transaction().id(),
    )
    .await?;

    // Wait for network to process the tagged withdrawal note
    println!("Waiting for network to process tagged withdrawal note...");
    sleep(Duration::from_secs(10)).await;
    client.sync_state().await?;

    // -------------------------------------------------------------------------
    // STEP 9: Alice consumes the P2ID withdraw note
    // -------------------------------------------------------------------------
    println!("\n[STEP 9] Alice consumes the P2ID withdraw note");

    let consume_p2id_request = TransactionRequestBuilder::new()
        .unauthenticated_input_notes([(withdraw_p2id_note.clone(), None)])
        .build()
        .unwrap();

    let tx_result = client
        .new_transaction(alice_account_id, consume_p2id_request)
        .await
        .unwrap();

    println!(
        "P2ID consumption Tx on MidenScan: https://testnet.midenscan.com/tx/{}",
        tx_result.executed_transaction().id().to_hex()
    );
    println!(
        "Alice's account delta: {:?}",
        tx_result.account_delta().vault()
    );
    let _ = client.submit_transaction(tx_result.clone()).await;

    client.sync_state().await.unwrap();

    println!("\n🎉 Network deposit and withdrawal flow completed successfully!");
    println!("✅ Assets were deposited into network contract via tagged note");
    println!("✅ Assets were withdrawn from network contract via tagged note");
    println!("✅ Alice received her assets back through P2ID note");

    Ok(())
}
