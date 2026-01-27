use rand::RngCore;
use std::{fs, path::Path, sync::Arc};
use tokio::time::{sleep, Duration};

use miden_client::{
    account::{
        component::{BasicFungibleFaucet, BasicWallet},
        Account,
        AccountBuilder,
        AccountStorageMode,
        AccountType,
    },
    crypto::FeltRng,
    address::NetworkId,
    asset::{FungibleAsset, TokenSymbol},
    auth::{AuthFalcon512Rpo, AuthSecretKey},
    builder::ClientBuilder,
    keystore::FilesystemKeyStore,
    note::{
        Note, NoteAssets, NoteInputs, NoteMetadata, NoteRecipient, NoteTag, NoteType,
    },
    rpc::{Endpoint, GrpcClient},
    store::TransactionFilter,
    transaction::{OutputNote, TransactionId, TransactionRequestBuilder, TransactionStatus},
    Client, ClientError, Felt,
};
use miden_client_sqlite_store::ClientBuilderSqliteExt;
use miden_protocol::Hasher;

// Helper to create a basic account
async fn create_basic_account(
    client: &mut Client<FilesystemKeyStore>,
    keystore: &Arc<FilesystemKeyStore>,
) -> Result<Account, ClientError> {
    let mut init_seed = [0_u8; 32];
    client.rng().fill_bytes(&mut init_seed);

    let key_pair = AuthSecretKey::new_falcon512_rpo();

    let account = AccountBuilder::new(init_seed)
        .account_type(AccountType::RegularAccountUpdatableCode)
        .storage_mode(AccountStorageMode::Public)
        .with_auth_component(AuthFalcon512Rpo::new(key_pair.public_key().to_commitment()))
        .with_component(BasicWallet)
        .build()
        .unwrap();

    client.add_account(&account, false).await?;
    keystore.add_key(&key_pair).unwrap();

    Ok(account)
}

async fn create_basic_faucet(
    client: &mut Client<FilesystemKeyStore>,
    keystore: &Arc<FilesystemKeyStore>,
) -> Result<Account, ClientError> {
    let mut init_seed = [0u8; 32];
    client.rng().fill_bytes(&mut init_seed);

    let key_pair = AuthSecretKey::new_falcon512_rpo();
    let symbol = TokenSymbol::new("MID").unwrap();
    let decimals = 8;
    let max_supply = Felt::new(1_000_000);

    let account = AccountBuilder::new(init_seed)
        .account_type(AccountType::FungibleFaucet)
        .storage_mode(AccountStorageMode::Public)
        .with_auth_component(AuthFalcon512Rpo::new(key_pair.public_key().to_commitment()))
        .with_component(BasicFungibleFaucet::new(symbol, decimals, max_supply).unwrap())
        .build()
        .unwrap();

    client.add_account(&account, false).await?;
    keystore.add_key(&key_pair).unwrap();

    Ok(account)
}

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

#[tokio::main]
async fn main() -> Result<(), ClientError> {
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
    // STEP 1: Create accounts and deploy faucet
    // -------------------------------------------------------------------------
    println!("\n[STEP 1] Creating new accounts");
    let alice_account = create_basic_account(&mut client, &keystore).await?;
    println!(
        "Alice's account ID: {:?}",
        alice_account.id().to_bech32(NetworkId::Testnet)
    );
    let bob_account = create_basic_account(&mut client, &keystore).await?;
    println!(
        "Bob's account ID: {:?}",
        bob_account.id().to_bech32(NetworkId::Testnet)
    );

    println!("\nDeploying a new fungible faucet.");
    let faucet = create_basic_faucet(&mut client, &keystore).await?;
    println!(
        "Faucet account ID: {:?}",
        faucet.id().to_bech32(NetworkId::Testnet)
    );
    client.sync_state().await?;

    // -------------------------------------------------------------------------
    // STEP 2: Mint tokens with P2ID
    // -------------------------------------------------------------------------
    println!("\n[STEP 2] Mint tokens with P2ID");
    let faucet_id = faucet.id();
    let amount: u64 = 100;
    let mint_amount = FungibleAsset::new(faucet_id, amount).unwrap();
    let tx_request = TransactionRequestBuilder::new()
        .build_mint_fungible_asset(
            mint_amount,
            alice_account.id(),
            NoteType::Public,
            client.rng(),
        )
        .unwrap();

    let tx_id = client
        .submit_new_transaction(faucet.id(), tx_request)
        .await?;
    println!("Minted tokens. TX: {:?}", tx_id);

    // Wait for the note to be available
    client.sync_state().await?;
    wait_for_tx(&mut client, tx_id).await?;

    // Consume the minted note
    let consumable_notes = client
        .get_consumable_notes(Some(alice_account.id()))
        .await?;

    if let Some((note_record, _)) = consumable_notes.first() {
        let note: Note = note_record.clone().try_into()?;
        let consume_request = TransactionRequestBuilder::new().build_consume_notes(vec![note])?;

        let tx_id = client
            .submit_new_transaction(alice_account.id(), consume_request)
            .await?;
        println!("Consumed minted note. TX: {:?}", tx_id);
    }

    client.sync_state().await?;

    // -------------------------------------------------------------------------
    // STEP 3: Create custom note
    // -------------------------------------------------------------------------
    println!("\n[STEP 3] Create custom note");
    let secret_vals = vec![Felt::new(1), Felt::new(2), Felt::new(3), Felt::new(4)];
    let digest = Hasher::hash_elements(&secret_vals);
    println!("digest: {:?}", digest);

    let code = fs::read_to_string(Path::new("../masm/notes/hash_preimage_note.masm")).unwrap();
    let serial_num = client.rng().draw_word();

    let note_script = client.code_builder().compile_note_script(code).unwrap();
    let note_inputs = NoteInputs::new(digest.to_vec()).unwrap();
    let recipient = NoteRecipient::new(serial_num, note_script, note_inputs);
    let tag = NoteTag::new(0);
    let metadata = NoteMetadata::new(alice_account.id(), NoteType::Public, tag);
    let vault = NoteAssets::new(vec![mint_amount.into()])?;
    let custom_note = Note::new(vault, metadata, recipient);
    println!("note hash: {:?}", custom_note.id().to_hex());

    let note_request = TransactionRequestBuilder::new()
        .own_output_notes(vec![OutputNote::Full(custom_note.clone())])
        .build()
        .unwrap();

    let tx_id = client
        .submit_new_transaction(alice_account.id(), note_request)
        .await?;
    println!(
        "View transaction on MidenScan: https://testnet.midenscan.com/tx/{:?}",
        tx_id
    );

    client.sync_state().await?;

    // -------------------------------------------------------------------------
    // STEP 4: Consume the Custom Note
    // -------------------------------------------------------------------------
    println!("\n[STEP 4] Bob consumes the Custom Note with Correct Secret");

    let secret = [Felt::new(1), Felt::new(2), Felt::new(3), Felt::new(4)];
    let consume_custom_request = TransactionRequestBuilder::new()
        .input_notes([(custom_note, Some(secret.into()))])
        .build()
        .unwrap();

    let tx_id = client
        .submit_new_transaction(bob_account.id(), consume_custom_request)
        .await?;
    println!(
        "Consumed Note Tx on MidenScan: https://testnet.midenscan.com/tx/{:?} \n",
        tx_id
    );

    Ok(())
}
