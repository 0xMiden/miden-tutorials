use std::sync::Arc;

use miden_client::{
    account::{AccountBuilder, AccountStorageMode, AccountType, StorageMap, StorageSlot},
    asset::{FungibleAsset, TokenSymbol},
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
    auth::{self, RpoFalcon512},
    faucets::BasicFungibleFaucet,
    wallets::BasicWallet,
};
use miden_lib::transaction::TransactionKernel;
use miden_lib::utils::ScriptBuilder;
use miden_objects::{
    account::{AccountComponent, NetworkId},
    assembly::{Assembler, DefaultSourceManager, Library, LibraryPath, Module, ModuleKind},
    FieldElement,
};
use rand::RngCore;
use tokio::time::{sleep, Duration};

/// Staking smart contract (account code)
pub const STAKING_CONTRACT: &str = "
use.miden::account
use.std::sys

const.STAKING_SLOT=0

# Constructor: just stores 1 at slot 1 as a initial transaction to deploy
# => []
export.deploy
    push.1 dup
    # => [1, 1]

    exec.account::set_item
    # => []

    exec.sys::truncate_stack
    # => []
end


# => []
export.get_count
    push.STAKING_SLOT
    # => [index]

    exec.account::get_item
    # => [count]

    exec.sys::truncate_stack
    # => []
end

# => [sender_acct_id_pre, sender_acct_id_suf, ASSET]
export.stake_info
    # (0, 0, sender_acct_id_pre, sender_acct_id_suf) will be KEY
    push.0.0
    # => [0, 0, sender_acct_id_pre, sender_acct_id_suf, ASSET]

    push.STAKING_SLOT
    # => [index, KEY, ASSET]

    debug.stack

    exec.account::set_map_item dropw dropw
    # => []

    exec.sys::truncate_stack
    # => []
end
";

/// Deployment script (transaction script to deploy)
pub const DEPLOY_SCRIPT: &str = "
use.external_contract::staking_contract

begin
    call.staking_contract::deploy
end

";

/// Staking note script
pub const NOTE_SCRIPT_TO_STAKE: &str = "
use.external_contract::staking_contract
use.miden::contracts::wallets::basic->wallet
use.miden::note

begin
    # store the asset of the note at position 1 and load it
    push.1 exec.note::get_assets assert
    # => [1]

    padw mem_loadw
    # => [ASSET]

    exec.note::get_sender
    # => [sender_id_prefix, sender_id_suffix, ASSET]

    debug.stack

    # Store info about Staker (KEY) and Asset (VALUE) in contract
    call.staking_contract::stake_info

    # Call receive asset in wallet
    call.wallet::receive_asset
    # => []
end

";

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

    let keystore = FilesystemKeyStore::new("./keystore".into()).unwrap();

    let sync_summary = client.sync_state().await.unwrap();
    println!("Latest block: {}", sync_summary.block_num);

    // -------------------------------------------------------------------------
    // STEP 1: Create a Faucet to get Alice some tokens to stake
    // -------------------------------------------------------------------------
    println!("\n[STEP 1] Creating a new faucet");

    // Faucet seed
    let mut init_seed = [0u8; 32];
    client.rng().fill_bytes(&mut init_seed);

    // Faucet parameters
    let symbol = TokenSymbol::new("MID").unwrap();
    let decimals = 8;
    let max_supply = Felt::new(1_000_000);

    // Generate key pair
    let key_pair = SecretKey::with_rng(client.rng());

    // Build the account
    let builder = AccountBuilder::new(init_seed)
        .account_type(AccountType::FungibleFaucet)
        .storage_mode(AccountStorageMode::Public)
        .with_auth_component(RpoFalcon512::new(key_pair.public_key()))
        .with_component(BasicFungibleFaucet::new(symbol, decimals, max_supply).unwrap());

    let (faucet_account, seed) = builder.build().unwrap();

    // Add the faucet to the client
    client
        .add_account(&faucet_account, Some(seed), false)
        .await?;

    // Add the key pair to the keystore
    keystore
        .add_key(&AuthSecretKey::RpoFalcon512(key_pair))
        .unwrap();

    println!(
        "Faucet account ID: {:?}",
        faucet_account.id().to_bech32(NetworkId::Testnet)
    );

    // Resync to show newly deployed faucet
    client.sync_state().await?;
    tokio::time::sleep(Duration::from_secs(2)).await;

    // -------------------------------------------------------------------------
    // STEP 2: Create Basic User Account Alice
    // -------------------------------------------------------------------------
    println!("\n[STEP 2] Creating a new account for Alice");

    // Account seed
    let mut init_seed = [0_u8; 32];
    client.rng().fill_bytes(&mut init_seed);

    let key_pair = SecretKey::with_rng(client.rng());

    // Build the account
    let builder = AccountBuilder::new(init_seed)
        .account_type(AccountType::RegularAccountUpdatableCode)
        .storage_mode(AccountStorageMode::Public)
        .with_auth_component(RpoFalcon512::new(key_pair.public_key()))
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
        alice_account.id().to_bech32(NetworkId::Testnet)
    );

    //------------------------------------------------------------
    // STEP 3: Mint 1 notes of 1000 tokens for Alice
    //------------------------------------------------------------
    println!("\n[STEP 3] Minting 1 note of 1000 tokens for Alice.");

    let amount: u64 = 1000;
    let fungible_asset = FungibleAsset::new(faucet_account.id(), amount).unwrap();

    let transaction_request = TransactionRequestBuilder::new()
        .build_mint_fungible_asset(
            fungible_asset,
            alice_account.id(),
            NoteType::Public,
            client.rng(),
        )
        .unwrap();

    println!("minting request built");

    let tx_execution_result = client
        .new_transaction(faucet_account.id(), transaction_request)
        .await?;
    client.submit_transaction(tx_execution_result).await?;

    println!("Note minted for Alice successfully!");

    // Re-sync so minted notes become visible
    client.sync_state().await?;

    //------------------------------------------------------------
    // STEP 4: Alice consumes her note
    //------------------------------------------------------------
    println!("\n[STEP 4] Alice will now consume her notes to have something to stake.");

    loop {
        // Resync to get the latest data
        client.sync_state().await?;

        let consumable_notes = client
            .get_consumable_notes(Some(alice_account.id()))
            .await?;
        let list_of_note_ids: Vec<_> = consumable_notes.iter().map(|(note, _)| note.id()).collect();

        if list_of_note_ids.len() == 1 {
            println!("Found Alice's note onchain. Consuming it now...");
            let transaction_request = TransactionRequestBuilder::new()
                .build_consume_notes(list_of_note_ids)
                .unwrap();
            let tx_execution_result = client
                .new_transaction(alice_account.id(), transaction_request)
                .await?;

            client.submit_transaction(tx_execution_result).await?;
            println!("All of Alice's notes consumed successfully.");
            break;
        } else {
            println!(
                "Currently, Alice has {} consumable notes. Waiting...",
                list_of_note_ids.len()
            );
            tokio::time::sleep(Duration::from_secs(3)).await;
        }
    }

    // -------------------------------------------------------------------------
    // STEP 5: Create Network Staking Smart Contract
    // -------------------------------------------------------------------------
    println!("\n[STEP 5] Creating a network staking smart contract");

    let staking_code: String = STAKING_CONTRACT.to_owned();

    // Create the network staking smart contract account
    // First, compile the MASM code into an account component
    let assembler: Assembler = TransactionKernel::assembler().with_debug_mode(true);
    let staking_component = AccountComponent::compile(
        staking_code.to_string(),
        assembler.clone(),
        vec![
            StorageSlot::Map(StorageMap::default()),
            StorageSlot::Value([Felt::new(0); 4]),
        ], // Initialize storage slots
    )
    .unwrap()
    .with_supports_all_types();

    // Generate a random seed for the account
    let mut init_seed = [0_u8; 32];
    client.rng().fill_bytes(&mut init_seed);

    // Build the immutable network account with no authentication
    let (staking_contract, counter_seed) = AccountBuilder::new(init_seed)
        .account_type(AccountType::RegularAccountImmutableCode) // Immutable code
        .storage_mode(AccountStorageMode::Public) // Stored on network
        .with_auth_component(auth::NoAuth) // No authentication required
        .with_component(staking_component)
        .with_component(BasicWallet)
        .build()
        .unwrap();

    client
        .add_account(&staking_contract, Some(counter_seed), false)
        .await
        .unwrap();

    println!(
        "contract id: {:?}",
        staking_contract.id().to_bech32(NetworkId::Testnet)
    );

    // -------------------------------------------------------------------------
    // STEP 6: Deploy Network Account with Transaction Script
    // -------------------------------------------------------------------------
    println!("\n[STEP 6] Deploy network staking smart contract");

    let tx_script_code = DEPLOY_SCRIPT.to_owned();
    let staking_code: String = STAKING_CONTRACT.to_owned();

    let library_path = "external_contract::staking_contract";

    let library = create_library(staking_code, library_path).unwrap();

    let tx_script = ScriptBuilder::default()
        .with_dynamically_linked_library(&library)?
        .compile_tx_script(tx_script_code)?;

    let tx_deploy_request = TransactionRequestBuilder::new()
        .custom_script(tx_script)
        .build()
        .unwrap();

    let tx_result = client
        .new_transaction(staking_contract.id(), tx_deploy_request)
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

    println!("test");

    // -------------------------------------------------------------------------
    // STEP 7: Prepare & Create the Staking Network Note (Alice wants to stake)
    // -------------------------------------------------------------------------
    println!("\n[STEP 7] Creating a network note for network counter contract");

    let staking_network_note_code = NOTE_SCRIPT_TO_STAKE.to_owned();
    let account_code = STAKING_CONTRACT.to_owned();

    let library_path = "external_contract::staking_contract";
    let library = create_library(account_code, library_path).unwrap();

    // Create and submit the network note that will increment the counter
    // Generate a random serial number for the note
    let rng = client.rng();
    let serial_num = rng.inner_mut().draw_word();

    // Compile the note script with the counter contract library
    let note_script = ScriptBuilder::default()
        .with_dynamically_linked_library(&library)?
        .compile_note_script(staking_network_note_code)?;

    // Create note recipient with empty inputs
    let note_inputs = NoteInputs::new([].to_vec())?;
    let recipient = NoteRecipient::new(serial_num, note_script, note_inputs);

    // Set up note metadata - tag it with the counter contract ID so it gets consumed
    let tag = NoteTag::from_account_id(staking_contract.id());
    let metadata = NoteMetadata::new(
        alice_account.id(),
        NoteType::Public,
        tag,
        NoteExecutionHint::none(),
        Felt::new(0),
    )?;

    // Put some assets to stake into the note
    let stake_amount = 333;
    let fungible_asset = FungibleAsset::new(faucet_account.id(), stake_amount).unwrap();
    let note_vault = NoteAssets::new(vec![fungible_asset.into()])?;

    // Create the complete note
    let staking_note = Note::new(note_vault, metadata, recipient);

    // Build and submit the transaction containing the note
    let note_req = TransactionRequestBuilder::new()
        .own_output_notes(vec![OutputNote::Full(staking_note)])
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

    // DEBUGGING: Consume staking note locally
    println!("\n[STEP 8] Staking contarct will now consume Alice's note.");
    client.add_note_tag(tag).await?;

    loop {
        // Resync to get the latest data
        client.sync_state().await?;

        let consumable_notes = client
            .get_consumable_notes(Some(staking_contract.id()))
            .await?;
        let list_of_note_ids: Vec<_> = consumable_notes.iter().map(|(note, _)| note.id()).collect();

        if list_of_note_ids.len() == 1 {
            println!("Found Staking note onchain. Consuming it now...");
            let transaction_request = TransactionRequestBuilder::new()
                .build_consume_notes(list_of_note_ids)
                .unwrap();
            let tx_execution_result = client
                .new_transaction(staking_contract.id(), transaction_request)
                .await?;

            client.submit_transaction(tx_execution_result).await?;
            println!("Worked.");
            break;
        } else {
            println!(
                "Currently, Staking Contract has {} consumable notes. Waiting...",
                list_of_note_ids.len()
            );
            tokio::time::sleep(Duration::from_secs(3)).await;
        }
    }

    // Checking updated state
    let new_account_state = client.get_account(staking_contract.id()).await.unwrap();

    if let Some(account) = new_account_state.as_ref() {
        let key: Word = [
            Felt::ZERO,
            Felt::ZERO,
            Felt::from(alice_account.id().prefix()),
            Felt::from(alice_account.id().suffix()),
        ]
        .into();
        let staking_info: Word = account
            .account()
            .storage()
            .get_map_item(0, key)
            .unwrap()
            .into();

        let vault_info = account.account().vault().assets().count();
        println!("🔢 Final staking storage info: {:?}", staking_info);
        println!("🔢 Final staking vault info: {}", vault_info);
    }

    Ok(())
}
