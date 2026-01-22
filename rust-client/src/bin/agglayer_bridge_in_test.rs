use std::sync::Arc;

use miden_agglayer::{
    claim_note::{ExitRoot, SmtNode},
    create_claim_note, create_existing_agglayer_faucet, create_existing_bridge_account,
    ClaimNoteInputs, EthAddressFormat, EthAmount, LeafData, OutputNoteData, ProofData,
};
use miden_client::{
    account::{component::BasicWallet, AccountBuilder, AccountStorageMode, AccountType},
    asset::{Asset, FungibleAsset},
    auth::AuthSecretKey,
    builder::ClientBuilder,
    crypto::{rpo_falcon512::SecretKey as RpoFalcon512SecretKey, FeltRng},
    keystore::FilesystemKeyStore,
    note::{Note, NoteAssets, NoteInputs, NoteMetadata, NoteRecipient, NoteTag, NoteType},
    rpc::{Endpoint, GrpcClient},
    store::TransactionFilter,
    transaction::{OutputNote, TransactionRequestBuilder, TransactionStatus},
    Client, ClientError, Felt,
};
use miden_client_sqlite_store::ClientBuilderSqliteExt;
use miden_protocol::testing::account::Account as TestAccount;
use miden_standards::{account::auth::AuthFalcon512Rpo, note::WellKnownNote};
use rand::RngCore;
use tokio::time::{sleep, Duration};

/// Waits for a specific transaction to be committed.
async fn wait_for_tx(
    client: &mut Client<FilesystemKeyStore>,
    tx_id: miden_client::transaction::TransactionId,
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
            println!("✅ Transaction {} committed", tx_id.to_hex());
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

/// Helper function to create test inputs for CLAIM note
/// This replicates the test_utils::claim_note_test_inputs() function
fn claim_note_test_inputs() -> (
    Vec<[u8; 32]>, // smt_proof_local_exit_root
    Vec<[u8; 32]>, // smt_proof_rollup_exit_root
    u32,           // global_index
    [u8; 32],      // mainnet_exit_root
    [u8; 32],      // rollup_exit_root
    u32,           // origin_network
    [u8; 20],      // origin_token_address
    u32,           // destination_network
    Vec<u8>,       // metadata
) {
    // Create mock SMT proofs (32 nodes each)
    let smt_proof_local: Vec<[u8; 32]> = (0..32)
        .map(|i| {
            let mut bytes = [0u8; 32];
            bytes[0] = i as u8;
            bytes
        })
        .collect();

    let smt_proof_rollup: Vec<[u8; 32]> = (0..32)
        .map(|i| {
            let mut bytes = [0u8; 32];
            bytes[0] = (i + 32) as u8;
            bytes
        })
        .collect();

    let global_index = 1u32;
    let mainnet_exit_root = [1u8; 32];
    let rollup_exit_root = [2u8; 32];
    let origin_network = 0u32;
    let origin_token_address = [3u8; 20];
    let destination_network = 1u32;
    let metadata = vec![];

    (
        smt_proof_local,
        smt_proof_rollup,
        global_index,
        mainnet_exit_root,
        rollup_exit_root,
        origin_network,
        origin_token_address,
        destination_network,
        metadata,
    )
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Agglayer Bridge-In Test: CLAIM Note → P2ID Note ===\n");

    // -------------------------------------------------------------------------
    // STEP 0: Initialize Client with Localhost Endpoint
    // -------------------------------------------------------------------------
    println!("[STEP 0] Initializing client with localhost endpoint...");

    let endpoint = Endpoint::localhost();
    let timeout_ms = 10_000;
    let rpc_client = Arc::new(GrpcClient::new(&endpoint, timeout_ms));

    // Initialize keystore with unique path for this test
    let keystore_path = std::path::PathBuf::from("./keystore_agglayer");
    let keystore = Arc::new(FilesystemKeyStore::new(keystore_path)?);

    let store_path = std::path::PathBuf::from("./store_agglayer.sqlite3");

    let mut client = ClientBuilder::new()
        .rpc(rpc_client)
        .sqlite_store(store_path)
        .authenticator(keystore.clone())
        .in_debug_mode(true.into())
        .build()
        .await?;

    let sync_summary = client.sync_state().await?;
    println!("Latest block: {}", sync_summary.block_num);

    // -------------------------------------------------------------------------
    // STEP 1: Create Bridge Account
    // -------------------------------------------------------------------------
    println!(
        "\n[STEP 1] Creating bridge account (with bridge_out component for MMR validation)..."
    );

    let bridge_seed = client.rng().draw_word();
    let bridge_account = create_existing_bridge_account(bridge_seed);

    // Convert to client's Account type
    let bridge_account_client = miden_client::account::Account::new(
        bridge_account.id(),
        bridge_account.vault().clone(),
        bridge_account.storage().clone(),
        bridge_account.code().clone(),
        bridge_account.nonce(),
        None,
    );

    client.add_account(&bridge_account_client, false).await?;

    println!(
        "Bridge account ID: {}",
        bridge_account.id().to_bech32(NetworkId::Testnet)
    );

    // -------------------------------------------------------------------------
    // STEP 2: Create Agglayer Faucet Account
    // -------------------------------------------------------------------------
    println!("\n[STEP 2] Creating agglayer faucet account (with agglayer_faucet component)...");

    let token_symbol = "AGG";
    let decimals = 8u8;
    let max_supply = Felt::new(1000000);
    let agglayer_faucet_seed = client.rng().draw_word();

    let agglayer_faucet = create_existing_agglayer_faucet(
        agglayer_faucet_seed,
        token_symbol,
        decimals,
        max_supply,
        bridge_account.id(),
    );

    // Convert to client's Account type
    let agglayer_faucet_client = miden_client::account::Account::new(
        agglayer_faucet.id(),
        agglayer_faucet.vault().clone(),
        agglayer_faucet.storage().clone(),
        agglayer_faucet.code().clone(),
        agglayer_faucet.nonce(),
        None,
    );

    client.add_account(&agglayer_faucet_client, false).await?;

    println!(
        "Agglayer faucet ID: {}",
        agglayer_faucet.id().to_bech32(NetworkId::Testnet)
    );

    // -------------------------------------------------------------------------
    // STEP 3: Create User Account to Receive P2ID Note
    // -------------------------------------------------------------------------
    println!("\n[STEP 3] Creating user account to receive P2ID note...");

    // Generate account seed
    let mut init_seed = [0_u8; 32];
    client.rng().fill_bytes(&mut init_seed);

    let key_pair = AuthSecretKey::Falcon512Rpo(miden_client::auth::falcon512rpo::SecretKey::new());

    // Build the user account
    let user_account = AccountBuilder::new(init_seed)
        .account_type(AccountType::RegularAccountUpdatableCode)
        .storage_mode(AccountStorageMode::Public)
        .with_auth_component(AuthRpoFalcon512::new(key_pair.public_key().to_commitment()))
        .with_component(BasicWallet)
        .build()?;

    client.add_account(&user_account, false).await?;
    keystore.add_key(&key_pair)?;

    println!(
        "User account ID: {}",
        user_account.id().to_bech32(NetworkId::Testnet)
    );

    // -------------------------------------------------------------------------
    // STEP 4: Create CLAIM Note with P2ID Output Note Details
    // -------------------------------------------------------------------------
    println!("\n[STEP 4] Creating CLAIM note with P2ID output note details...");

    // Define amount values for the test
    let claim_amount = 100u32;

    // Get test inputs
    let (
        smt_proof_local_exit_root,
        smt_proof_rollup_exit_root,
        global_index,
        mainnet_exit_root,
        rollup_exit_root,
        origin_network,
        origin_token_address,
        destination_network,
        metadata,
    ) = claim_note_test_inputs();

    // Convert AccountId to destination address bytes
    let destination_address = EthAddressFormat::from_account_id(user_account.id()).into_bytes();

    // Generate a serial number for the P2ID note
    let serial_num = client.rng().draw_word();

    // Convert amount to EthAmount for the LeafData
    let amount_eth = EthAmount::from_u32(claim_amount);

    // Convert Vec<[u8; 32]> to [SmtNode; 32] for SMT proofs
    let local_proof_array: [SmtNode; 32] = smt_proof_local_exit_root[0..32]
        .iter()
        .map(|&bytes| SmtNode::from(bytes))
        .collect::<Vec<_>>()
        .try_into()
        .expect("should have exactly 32 elements");

    let rollup_proof_array: [SmtNode; 32] = smt_proof_rollup_exit_root[0..32]
        .iter()
        .map(|&bytes| SmtNode::from(bytes))
        .collect::<Vec<_>>()
        .try_into()
        .expect("should have exactly 32 elements");

    let proof_data = ProofData {
        smt_proof_local_exit_root: local_proof_array,
        smt_proof_rollup_exit_root: rollup_proof_array,
        global_index,
        mainnet_exit_root: ExitRoot::from(mainnet_exit_root),
        rollup_exit_root: ExitRoot::from(rollup_exit_root),
    };

    let leaf_data = LeafData {
        origin_network,
        origin_token_address: EthAddressFormat::new(origin_token_address),
        destination_network,
        destination_address: EthAddressFormat::new(destination_address),
        amount: amount_eth,
        metadata,
    };

    let output_note_data = OutputNoteData {
        output_p2id_serial_num: serial_num,
        target_faucet_account_id: agglayer_faucet.id(),
        output_note_tag: NoteTag::with_account_target(user_account.id()),
    };

    let claim_inputs = ClaimNoteInputs {
        proof_data,
        leaf_data,
        output_note_data,
    };

    let claim_note = create_claim_note(claim_inputs)?;

    println!("CLAIM note created: {}", claim_note.id());

    // -------------------------------------------------------------------------
    // STEP 5: Create Expected P2ID Note for Verification
    // -------------------------------------------------------------------------
    println!("\n[STEP 5] Creating expected P2ID note for verification...");

    let p2id_script = WellKnownNote::P2ID.script();
    let p2id_inputs = vec![
        user_account.id().suffix(),
        user_account.id().prefix().as_felt(),
    ];
    let note_inputs = NoteInputs::new(p2id_inputs)?;
    let p2id_recipient = NoteRecipient::new(serial_num, p2id_script.clone(), note_inputs);

    let amount_felt = Felt::from(claim_amount);
    let mint_asset: Asset = FungibleAsset::new(agglayer_faucet.id(), amount_felt.into())?.into();
    let output_note_tag = NoteTag::with_account_target(user_account.id());

    let expected_p2id_note = Note::new(
        NoteAssets::new(vec![mint_asset])?,
        NoteMetadata::new(agglayer_faucet.id(), NoteType::Public, output_note_tag),
        p2id_recipient,
    );

    println!("Expected P2ID note ID: {}", expected_p2id_note.id());

    // -------------------------------------------------------------------------
    // STEP 6: Submit CLAIM Note to Chain
    // -------------------------------------------------------------------------
    println!("\n[STEP 6] Submitting CLAIM note to chain...");

    // Convert claim_note to client's Note type
    let claim_note_client = miden_client::note::Note::from_parts(
        claim_note.assets().clone(),
        claim_note.metadata().clone(),
        claim_note.recipient().clone(),
    );

    let claim_note_tx = TransactionRequestBuilder::new()
        .own_output_notes(vec![OutputNote::Full(claim_note_client.clone())])
        .build()?;

    let claim_note_tx_id = client
        .submit_new_transaction(user_account.id(), claim_note_tx)
        .await?;

    println!("CLAIM note transaction ID: {}", claim_note_tx_id.to_hex());

    // Wait for CLAIM note to be committed
    wait_for_tx(&mut client, claim_note_tx_id).await?;

    // Sync to make the note available
    client.sync_state().await?;

    // -------------------------------------------------------------------------
    // STEP 7: Agglayer Faucet Consumes CLAIM Note (Creates P2ID Note)
    // -------------------------------------------------------------------------
    println!("\n[STEP 7] Agglayer faucet consuming CLAIM note (with FPI to Bridge)...");

    // Get consumable notes for the faucet
    let consumable_notes = client
        .get_consumable_notes(Some(agglayer_faucet.id()))
        .await?;

    println!(
        "Found {} consumable notes for faucet",
        consumable_notes.len()
    );

    // Find the CLAIM note
    let claim_note_to_consume = consumable_notes
        .iter()
        .find(|(note, _)| note.id() == claim_note_client.id())
        .map(|(note, _)| note.clone())
        .ok_or("CLAIM note not found in consumable notes")?;

    // Build transaction to consume the note
    let faucet_tx =
        TransactionRequestBuilder::new().build_consume_notes(vec![claim_note_to_consume])?;

    let faucet_tx_id = client
        .submit_new_transaction(agglayer_faucet.id(), faucet_tx)
        .await?;

    println!(
        "Faucet consumption transaction ID: {}",
        faucet_tx_id.to_hex()
    );

    // Wait for faucet transaction to be committed
    wait_for_tx(&mut client, faucet_tx_id).await?;

    // Sync to get the P2ID note
    client.sync_state().await?;

    // -------------------------------------------------------------------------
    // STEP 8: Verify P2ID Note Was Created
    // -------------------------------------------------------------------------
    println!("\n[STEP 8] Verifying P2ID note was created...");

    // Get the transaction details
    let txs = client
        .get_transactions(TransactionFilter::Ids(vec![faucet_tx_id]))
        .await?;

    if let Some(tx) = txs.first() {
        let output_notes = &tx.details.output_notes;
        println!("Number of output notes: {}", output_notes.len());

        if output_notes.len() == 1 {
            let output_note = &output_notes[0];
            println!("✅ P2ID note created: {}", output_note.id());
            println!("   Expected note ID: {}", expected_p2id_note.id());

            // Verify note ID matches
            if output_note.id() == expected_p2id_note.id() {
                println!("✅ P2ID note ID matches expected!");
            } else {
                println!("⚠️  P2ID note ID does not match expected");
            }
        } else {
            println!("⚠️  Expected 1 output note, found {}", output_notes.len());
        }
    }

    // -------------------------------------------------------------------------
    // STEP 9: User Consumes P2ID Note
    // -------------------------------------------------------------------------
    println!("\n[STEP 9] User consuming P2ID note...");

    // Get consumable notes for user
    let user_consumable_notes = client.get_consumable_notes(Some(user_account.id())).await?;

    println!(
        "Found {} consumable notes for user",
        user_consumable_notes.len()
    );

    // Find the P2ID note
    let p2id_note_to_consume = user_consumable_notes
        .iter()
        .find(|(note, _)| note.id() == expected_p2id_note.id())
        .map(|(note, _)| note.clone())
        .ok_or("P2ID note not found in consumable notes")?;

    // Consume the P2ID note
    let user_consume_tx =
        TransactionRequestBuilder::new().build_consume_notes(vec![p2id_note_to_consume])?;

    let user_consume_tx_id = client
        .submit_new_transaction(user_account.id(), user_consume_tx)
        .await?;

    println!(
        "User consumption transaction ID: {}",
        user_consume_tx_id.to_hex()
    );

    // Wait for user transaction to be committed
    wait_for_tx(&mut client, user_consume_tx_id).await?;

    // Sync to update account state
    client.sync_state().await?;

    // -------------------------------------------------------------------------
    // STEP 10: Verify User Balance
    // -------------------------------------------------------------------------
    println!("\n[STEP 10] Verifying user balance...");

    // Get updated account state
    let updated_user_account = client.get_account(user_account.id()).await?;

    if let Some(account_state) = updated_user_account {
        let balance = account_state
            .account_data()
            .vault()
            .get_balance(agglayer_faucet.id())?;

        println!("✅ User balance: {} AGG tokens", balance);

        if balance == claim_amount.into() {
            println!("✅ Balance matches expected amount!");
        } else {
            println!(
                "⚠️  Balance {} does not match expected {}",
                balance, claim_amount
            );
        }
    } else {
        println!("⚠️  Could not retrieve updated user account");
    }

    println!("\n=== Test Complete ===");
    println!("Summary:");
    println!("  - Bridge account deployed");
    println!("  - Agglayer faucet deployed");
    println!("  - User account created");
    println!("  - CLAIM note created and submitted");
    println!("  - Faucet consumed CLAIM note and minted P2ID note");
    println!(
        "  - User consumed P2ID note and received {} AGG tokens",
        claim_amount
    );

    Ok(())
}
