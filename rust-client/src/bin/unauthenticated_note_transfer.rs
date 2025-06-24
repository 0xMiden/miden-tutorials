use rand::RngCore;
use std::sync::Arc;
use tokio::time::{Duration, Instant};

use miden_client::{
    account::{
        component::{BasicFungibleFaucet, BasicWallet, RpoFalcon512},
        AccountBuilder, AccountStorageMode, AccountType,
    },
    asset::{FungibleAsset, TokenSymbol},
    auth::AuthSecretKey,
    builder::ClientBuilder,
    crypto::SecretKey,
    keystore::FilesystemKeyStore,
    note::{create_p2id_note, Note, NoteType},
    rpc::{Endpoint, TonicRpcClient},
    transaction::{OutputNote, TransactionRequestBuilder},
    utils::{Deserializable, Serializable},
    ClientError, Felt,
};
use miden_objects::account::NetworkId;
#[tokio::main]
async fn main() -> Result<(), ClientError> {
    // Initialize client & keystore
    let endpoint = Endpoint::testnet();
    let timeout_ms = 10_000;
    let rpc_api = Arc::new(TonicRpcClient::new(&endpoint, timeout_ms));

    let mut client = ClientBuilder::new()
        .with_rpc(rpc_api)
        .with_filesystem_keystore("./keystore")
        .in_debug_mode(true)
        .build()
        .await?;

    let sync_summary = client.sync_state().await.unwrap();
    println!("Latest block: {}", sync_summary.block_num);

    let keystore = FilesystemKeyStore::new("./keystore".into()).unwrap();

    //------------------------------------------------------------
    // STEP 1: Deploy a fungible faucet
    //------------------------------------------------------------
    println!("\n[STEP 1] Deploying a new fungible faucet.");

    // Faucet seed
    let mut init_seed = [0_u8; 32];
    client.rng().fill_bytes(&mut init_seed);

    let key_pair = SecretKey::with_rng(client.rng());

    // Anchor block
    let anchor_block = client.get_latest_epoch_block().await.unwrap();

    // Faucet parameters
    let symbol = TokenSymbol::new("MID").unwrap();
    let decimals = 8;
    let max_supply = Felt::new(1_000_000);

    // Generate key pair

    // Build the account
    let builder = AccountBuilder::new(init_seed)
        .anchor((&anchor_block).try_into().unwrap())
        .account_type(AccountType::FungibleFaucet)
        .storage_mode(AccountStorageMode::Public)
        .with_component(RpoFalcon512::new(key_pair.public_key()))
        .with_component(BasicFungibleFaucet::new(symbol, decimals, max_supply).unwrap());

    let (faucet_account, seed) = builder.build().unwrap();

    // Add the faucet to the client
    client
        .add_account(&faucet_account, Some(seed), false)
        .await?;
    println!(
        "Faucet account ID: {}",
        faucet_account.id().to_bech32(NetworkId::Testnet)
    );

    // Add the key pair to the keystore
    keystore
        .add_key(&AuthSecretKey::RpoFalcon512(key_pair))
        .unwrap();

    // Resync to show newly deployed faucet
    tokio::time::sleep(Duration::from_secs(2)).await;
    client.sync_state().await?;

    //------------------------------------------------------------
    // STEP 2: Create basic wallet accounts
    //------------------------------------------------------------
    println!("\n[STEP 2] Creating new accounts");

    let mut accounts = vec![];
    let number_of_accounts = 10;

    for i in 0..number_of_accounts {
        let mut init_seed = [0_u8; 32];
        client.rng().fill_bytes(&mut init_seed);
        let key_pair = SecretKey::with_rng(client.rng());
        let builder = AccountBuilder::new(init_seed)
            .anchor((&anchor_block).try_into().unwrap())
            .account_type(AccountType::RegularAccountUpdatableCode)
            .storage_mode(AccountStorageMode::Public)
            .with_component(RpoFalcon512::new(key_pair.public_key()))
            .with_component(BasicWallet);

        let (account, seed) = builder.build().unwrap();
        accounts.push(account.clone());
        println!(
            "account id {:?}: {}",
            i,
            account.id().to_bech32(NetworkId::Testnet)
        );
        client.add_account(&account, Some(seed), true).await?;

        // Add the key pair to the keystore
        keystore
            .add_key(&AuthSecretKey::RpoFalcon512(key_pair))
            .unwrap();
    }

    // For demo purposes, Alice is the first account.
    let alice = &accounts[0];

    //------------------------------------------------------------
    // STEP 3: Mint and consume tokens for Alice
    //------------------------------------------------------------
    println!("\n[STEP 3] Mint tokens");
    println!("Minting tokens for Alice...");
    let amount: u64 = 100;
    let fungible_asset_mint_amount = FungibleAsset::new(faucet_account.id(), amount).unwrap();
    let transaction_request = TransactionRequestBuilder::new()
        .build_mint_fungible_asset(
            fungible_asset_mint_amount,
            alice.id(),
            NoteType::Public,
            client.rng(),
        )
        .unwrap();

    let tx_execution_result = client
        .new_transaction(faucet_account.id(), transaction_request)
        .await?;
    client
        .submit_transaction(tx_execution_result.clone())
        .await?;

    // The minted fungible asset is public so output is a `Full` note type
    let p2id_note: Note =
        if let OutputNote::Full(note) = tx_execution_result.created_notes().get_note(0) {
            note.clone()
        } else {
            panic!("Expected Full note type");
        };

    let transaction_request = TransactionRequestBuilder::new()
        .with_unauthenticated_input_notes([(p2id_note, None)])
        .build()
        .unwrap();
    let tx_execution_result = client
        .new_transaction(alice.id(), transaction_request)
        .await?;
    client.submit_transaction(tx_execution_result).await?;
    client.sync_state().await?;

    //------------------------------------------------------------
    // STEP 4: Create unauthenticated note tx chain
    //------------------------------------------------------------
    println!("\n[STEP 4] Create unauthenticated note tx chain");
    let mut landed_blocks = vec![];
    let start = Instant::now();

    for i in 0..number_of_accounts - 1 {
        let loop_start = Instant::now();
        println!("\nunauthenticated tx {:?}", i + 1);
        println!("sender: {}", accounts[i].id().to_bech32(NetworkId::Testnet));
        println!(
            "target: {}",
            accounts[i + 1].id().to_bech32(NetworkId::Testnet)
        );

        // Time the creation of the p2id note
        let send_amount = 20;
        let fungible_asset_send_amount =
            FungibleAsset::new(faucet_account.id(), send_amount).unwrap();

        // for demo purposes, unauthenticated notes can be public or private
        let note_type = if i % 2 == 0 {
            NoteType::Private
        } else {
            NoteType::Public
        };

        let p2id_note = create_p2id_note(
            accounts[i].id(),
            accounts[i + 1].id(),
            vec![fungible_asset_send_amount.into()],
            note_type,
            Felt::new(0),
            client.rng(),
        )
        .unwrap();

        let output_note = OutputNote::Full(p2id_note.clone());

        // Time transaction request building
        let transaction_request = TransactionRequestBuilder::new()
            .with_own_output_notes(vec![output_note])
            .build()
            .unwrap();
        let tx_execution_result = client
            .new_transaction(accounts[i].id(), transaction_request)
            .await?;
        client.submit_transaction(tx_execution_result).await?;

        // Note serialization/deserialization
        // This demonstrates how you could send the serialized note to another client instance
        let serialized = p2id_note.to_bytes();
        let deserialized_p2id_note = Note::read_from_bytes(&serialized).unwrap();

        // Time consume note request building
        let consume_note_request = TransactionRequestBuilder::new()
            .with_unauthenticated_input_notes([(deserialized_p2id_note, None)])
            .build()
            .unwrap();

        let tx_execution_result = client
            .new_transaction(accounts[i + 1].id(), consume_note_request)
            .await?;

        landed_blocks.push(tx_execution_result.block_num());
        client
            .submit_transaction(tx_execution_result.clone())
            .await?;

        println!(
            "Consumed Note Tx on MidenScan: https://testnet.midenscan.com/tx/{:?}",
            tx_execution_result.executed_transaction().id()
        );
        println!(
            "Total time for loop iteration {}: {:?}",
            i,
            loop_start.elapsed()
        );
    }

    println!(
        "\nTotal execution time for unauthenticated note txs: {:?}",
        start.elapsed()
    );
    println!("blocks: {:?}", landed_blocks);

    // Final resync and display account balances
    tokio::time::sleep(Duration::from_secs(3)).await;
    client.sync_state().await?;
    for account in accounts.clone() {
        let new_account = client.get_account(account.id()).await.unwrap().unwrap();
        let balance = new_account
            .account()
            .vault()
            .get_balance(faucet_account.id())
            .unwrap();
        println!(
            "Account: {} balance: {}",
            account.id().to_bech32(NetworkId::Testnet),
            balance
        );
    }

    Ok(())
}
