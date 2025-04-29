# Mint, Consume, and Create Notes

_Using the Miden client in Rust to mint, consume, and create notes_

## Overview

In the previous section, we initialized our repository and covered how to create an account and deploy a faucet. In this section, we will mint tokens from the faucet for _Alice_, consume the newly created notes, and demonstrate how to send assets to other accounts.

## What we'll cover

- Minting tokens from a faucet
- Consuming notes to fund an account
- Sending tokens to other users

## Step 1: Minting tokens from the faucet

To mint notes with tokens from the faucet we created, Alice needs to call the faucet with a mint transaction request.

_In essence, a transaction request is a structured template that outlines the data required to generate a zero-knowledge proof of a state change of an account. It specifies which input notes (if any) will be consumed, includes an optional transaction script to execute, and enumerates the set of notes expected to be created (if any)._

Below is an example of a transaction request minting tokens from the faucet for Alice. This code snippet will create 5 transaction mint transaction requests.

Add this snippet to the end of your file in the `main()` function that we created in the previous chapter:

```rust
//------------------------------------------------------------
// STEP 3: Mint 5 notes of 100 tokens for Alice
//------------------------------------------------------------
println!("\n[STEP 3] Minting 5 notes of 100 tokens each for Alice.");

let amount: u64 = 100;
let fungible_asset = FungibleAsset::new(faucet_account.id(), amount).unwrap();

for i in 1..=5 {
    let transaction_request = TransactionRequestBuilder::mint_fungible_asset(
        fungible_asset,
        alice_account.id(),
        NoteType::Public,
        client.rng(),
    )
    .unwrap()
    .build()
    .unwrap();

    println!("tx request built");

    let tx_execution_result = client
        .new_transaction(faucet_account.id(), transaction_request)
        .await?;
    client.submit_transaction(tx_execution_result).await?;
    println!("Minted note #{} of {} tokens for Alice.", i, amount);
}
println!("All 5 notes minted for Alice successfully!");

// Re-sync so minted notes become visible
client.sync_state().await?;
```

## Step 2: Identifying consumable notes

Once Alice has minted a note from the faucet, she will eventually want to spend the tokens that she received in the note created by the mint transaction.

Minting a note from a faucet on Miden means a faucet account creates a new note targeted to the requesting account. The requesting account needs to consume this new note to have the assets appear in their account.

To identify consumable notes, the Miden client provides the `get_consumable_notes` function. Before calling it, ensure that the client state is synced.

_Tip: If you know how many notes to expect after a transaction, use an await or loop condition to check how many notes of the type you expect are available for consumption instead of using a set timeout before calling `get_consumable_notes`. This ensures your application isn't idle for longer than necessary._

#### Identifying which notes are available:

```rust
let consumable_notes = client.get_consumable_notes(Some(alice_account.id())).await?;
```

## Step 3: Consuming multiple notes in a single transaction:

Now that we know how to identify notes ready to consume, let's consume the notes created by the faucet in a single transaction. After consuming the notes, Alice's wallet balance will be updated.

The following code snippet identifies consumable notes and consumes them in a single transaction.

Add this snippet to the end of your file in the `main()` function:

```Rust
//------------------------------------------------------------
// STEP 4: Alice consumes all her notes
//------------------------------------------------------------
println!("\n[STEP 4] Alice will now consume all of her notes to consolidate them.");

// Consume all minted notes in a single transaction
loop {
    // Resync to get the latest data
    client.sync_state().await?;

    let consumable_notes = client
        .get_consumable_notes(Some(alice_account.id()))
        .await?;
    let list_of_note_ids: Vec<_> = consumable_notes.iter().map(|(note, _)| note.id()).collect();

    if list_of_note_ids.len() == 5 {
        println!("Found 5 consumable notes for Alice. Consuming them now...");
        let transaction_request = TransactionRequestBuilder::consume_notes(list_of_note_ids)
            .build()
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
```

## Step 4: Sending tokens to other accounts

After consuming the notes, Alice has tokens in her wallet. Now, she wants to send tokens to her friends. She has two options: create a separate transaction for each transfer or batch multiple transfers into a single transaction.

_The standard asset transfer note on Miden is the P2ID note (Pay to Id). There is also the P2IDR (Pay to Id Reclaimable) variant which allows the creator of the note to reclaim the note after a certain block height._

In our example, Alice will now send 50 tokens to 5 different accounts.

For the sake of the example, the first four P2ID transfers are handled in a single transaction, and the fifth transfer is a standard P2ID transfer.

### Output multiple P2ID notes in a single transaction

To output multiple notes in a single transaction we need to create a list of our expected output notes. The expected output notes are the notes that we expect to create in our transaction request.

In the snippet below, we create an empty vector to store five P2ID output notes, loop over five iterations `(using 0..=4)` to create five unique dummy account IDs, build a P2ID note for each one, and push each note onto the vector. Finally, we build a transaction request using `.with_own_output_notes()`—passing in all five notes—and submit it to the node.

Add this snippet to the end of your file in the `main()` function:

```Rust
//------------------------------------------------------------
// STEP 5: Alice sends 5 notes of 50 tokens to 5 users
//------------------------------------------------------------
println!("\n[STEP 5] Alice sends 5 notes of 50 tokens each to 5 different users.");

// Send 50 tokens to 4 accounts in one transaction
println!("Creating multiple P2ID notes for 4 target accounts in one transaction...");
let mut p2id_notes = vec![];

// Creating 4 P2ID notes to 4 'dummy' AccountIds
for _ in 1..=4 {
    let init_seed: [u8; 15] = {
        let mut init_seed = [0_u8; 15];
        client.rng().fill_bytes(&mut init_seed);
        init_seed
    };
    let target_account_id = AccountId::dummy(
        init_seed,
        AccountIdVersion::Version0,
        AccountType::RegularAccountUpdatableCode,
        AccountStorageMode::Public,
    );

    let send_amount = 50;
    let fungible_asset = FungibleAsset::new(faucet_account.id(), send_amount).unwrap();

    let p2id_note = create_p2id_note(
        alice_account.id(),
        target_account_id,
        vec![fungible_asset.into()],
        NoteType::Public,
        Felt::new(0),
        client.rng(),
    )?;
    p2id_notes.push(p2id_note);
}

// Specifying output notes and creating a tx request to create them
let output_notes: Vec<OutputNote> = p2id_notes.into_iter().map(OutputNote::Full).collect();
let transaction_request = TransactionRequestBuilder::new()
    .with_own_output_notes(output_notes)
    .build()
    .unwrap();

let tx_execution_result = client
    .new_transaction(alice_account.id(), transaction_request)
    .await?;

// Submitting the transaction
client.submit_transaction(tx_execution_result).await?;
println!("Submitted a transaction with 4 P2ID notes.");
```

### Basic P2ID transfer

Now as an example, Alice will send some tokens to an account in a single transaction.

Add this snippet to the end of your file in the `main()` function:

```Rust
println!("Submitting one more single P2ID transaction...");
let init_seed: [u8; 15] = {
    let mut init_seed = [0_u8; 15];
    client.rng().fill_bytes(&mut init_seed);
    init_seed
};
let target_account_id = AccountId::dummy(
    init_seed,
    AccountIdVersion::Version0,
    AccountType::RegularAccountUpdatableCode,
    AccountStorageMode::Public,
);

let send_amount = 50;
let fungible_asset = FungibleAsset::new(faucet_account.id(), send_amount).unwrap();

let payment_transaction = PaymentTransactionData::new(
    vec![fungible_asset.into()],
    alice_account.id(),
    target_account_id,
);
let transaction_request = TransactionRequestBuilder::pay_to_id(
    payment_transaction,
    None,             // recall_height
    NoteType::Public, // note type
    client.rng(),     // rng
)
.unwrap()
.build()
.unwrap();
let tx_execution_result = client
    .new_transaction(alice_account.id(), transaction_request)
    .await?;

client.submit_transaction(tx_execution_result).await?;

```

Note: _In a production environment do not use `AccountId::new_dummy()`, this is simply for the sake of the tutorial example._

## Summary

Your `src/main.rs` function should now look like this:

```rust
use rand::RngCore;
use std::sync::Arc;
use tokio::time::Duration;

use miden_client::{
    account::{
        component::{BasicFungibleFaucet, BasicWallet, RpoFalcon512},
        AccountBuilder, AccountId, AccountStorageMode, AccountType,
    },
    asset::{FungibleAsset, TokenSymbol},
    auth::AuthSecretKey,
    builder::ClientBuilder,
    crypto::SecretKey,
    keystore::FilesystemKeyStore,
    note::{create_p2id_note, NoteType},
    rpc::{Endpoint, TonicRpcClient},
    transaction::{OutputNote, PaymentTransactionData, TransactionRequestBuilder},
    ClientError, Felt,
};
use miden_objects::account::AccountIdVersion;

#[tokio::main]
async fn main() -> Result<(), ClientError> {
    // Initialize client & keystore
    let endpoint = Endpoint::new(
        "https".to_string(),
        "rpc.testnet.miden.io".to_string(),
        Some(443),
    );
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

    let keystore: FilesystemKeyStore<rand::prelude::StdRng> =
        FilesystemKeyStore::new("./keystore".into()).unwrap();

    //------------------------------------------------------------
    // STEP 1: Create a basic wallet for Alice
    //------------------------------------------------------------
    println!("\n[STEP 1] Creating a new account for Alice");

    // Account seed
    let mut init_seed = [0_u8; 32];
    client.rng().fill_bytes(&mut init_seed);

    let key_pair = SecretKey::with_rng(client.rng());

    // Anchor block
    let anchor_block = client.get_latest_epoch_block().await.unwrap();

    // Build the account
    let builder = AccountBuilder::new(init_seed)
        .anchor((&anchor_block).try_into().unwrap())
        .account_type(AccountType::RegularAccountUpdatableCode)
        .storage_mode(AccountStorageMode::Public)
        .with_component(RpoFalcon512::new(key_pair.public_key()))
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

    println!("Alice's account ID: {:?}", alice_account.id().to_hex());

    //------------------------------------------------------------
    // STEP 2: Deploy a fungible faucet
    //------------------------------------------------------------
    println!("\n[STEP 2] Deploying a new fungible faucet.");

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

    // Add the key pair to the keystore
    keystore
        .add_key(&AuthSecretKey::RpoFalcon512(key_pair))
        .unwrap();

    println!("Faucet account ID: {:?}", faucet_account.id().to_hex());

    // Resync to show newly deployed faucet
    client.sync_state().await?;
    tokio::time::sleep(Duration::from_secs(2)).await;

    //------------------------------------------------------------
    // STEP 3: Mint 5 notes of 100 tokens for Alice
    //------------------------------------------------------------
    println!("\n[STEP 3] Minting 5 notes of 100 tokens each for Alice.");

    let amount: u64 = 100;
    let fungible_asset = FungibleAsset::new(faucet_account.id(), amount).unwrap();

    for i in 1..=5 {
        let transaction_request = TransactionRequestBuilder::mint_fungible_asset(
            fungible_asset,
            alice_account.id(),
            NoteType::Public,
            client.rng(),
        )
        .unwrap()
        .build()
        .unwrap();

        println!("tx request built");

        let tx_execution_result = client
            .new_transaction(faucet_account.id(), transaction_request)
            .await?;
        client.submit_transaction(tx_execution_result).await?;
        println!("Minted note #{} of {} tokens for Alice.", i, amount);
    }
    println!("All 5 notes minted for Alice successfully!");

    // Re-sync so minted notes become visible
    client.sync_state().await?;

    //------------------------------------------------------------
    // STEP 4: Alice consumes all her notes
    //------------------------------------------------------------
    println!("\n[STEP 4] Alice will now consume all of her notes to consolidate them.");

    // Consume all minted notes in a single transaction
    loop {
        // Resync to get the latest data
        client.sync_state().await?;

        let consumable_notes = client
            .get_consumable_notes(Some(alice_account.id()))
            .await?;
        let list_of_note_ids: Vec<_> = consumable_notes.iter().map(|(note, _)| note.id()).collect();

        if list_of_note_ids.len() == 5 {
            println!("Found 5 consumable notes for Alice. Consuming them now...");
            let transaction_request = TransactionRequestBuilder::consume_notes(list_of_note_ids)
                .build()
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

    //------------------------------------------------------------
    // STEP 5: Alice sends 5 notes of 50 tokens to 5 users
    //------------------------------------------------------------
    println!("\n[STEP 5] Alice sends 5 notes of 50 tokens each to 5 different users.");

    // Send 50 tokens to 4 accounts in one transaction
    println!("Creating multiple P2ID notes for 4 target accounts in one transaction...");
    let mut p2id_notes = vec![];

    // Creating 4 P2ID notes to 4 'dummy' AccountIds
    for _ in 1..=4 {
        let init_seed: [u8; 15] = {
            let mut init_seed = [0_u8; 15];
            client.rng().fill_bytes(&mut init_seed);
            init_seed
        };
        let target_account_id = AccountId::dummy(
            init_seed,
            AccountIdVersion::Version0,
            AccountType::RegularAccountUpdatableCode,
            AccountStorageMode::Public,
        );

        let send_amount = 50;
        let fungible_asset = FungibleAsset::new(faucet_account.id(), send_amount).unwrap();

        let p2id_note = create_p2id_note(
            alice_account.id(),
            target_account_id,
            vec![fungible_asset.into()],
            NoteType::Public,
            Felt::new(0),
            client.rng(),
        )?;
        p2id_notes.push(p2id_note);
    }

    // Specifying output notes and creating a tx request to create them
    let output_notes: Vec<OutputNote> = p2id_notes.into_iter().map(OutputNote::Full).collect();
    let transaction_request = TransactionRequestBuilder::new()
        .with_own_output_notes(output_notes)
        .build()
        .unwrap();

    let tx_execution_result = client
        .new_transaction(alice_account.id(), transaction_request)
        .await?;

    // Submitting the transaction
    client.submit_transaction(tx_execution_result).await?;
    println!("Submitted a transaction with 4 P2ID notes.");

    println!("Submitting one more single P2ID transaction...");
    let init_seed: [u8; 15] = {
        let mut init_seed = [0_u8; 15];
        client.rng().fill_bytes(&mut init_seed);
        init_seed
    };
    let target_account_id = AccountId::dummy(
        init_seed,
        AccountIdVersion::Version0,
        AccountType::RegularAccountUpdatableCode,
        AccountStorageMode::Public,
    );

    let send_amount = 50;
    let fungible_asset = FungibleAsset::new(faucet_account.id(), send_amount).unwrap();

    let payment_transaction = PaymentTransactionData::new(
        vec![fungible_asset.into()],
        alice_account.id(),
        target_account_id,
    );
    let transaction_request = TransactionRequestBuilder::pay_to_id(
        payment_transaction,
        None,             // recall_height
        NoteType::Public, // note type
        client.rng(),     // rng
    )
    .unwrap()
    .build()
    .unwrap();
    let tx_execution_result = client
        .new_transaction(alice_account.id(), transaction_request)
        .await?;

    client.submit_transaction(tx_execution_result).await?;

    println!("\nAll steps completed successfully!");
    println!("Alice created a wallet, a faucet was deployed,");
    println!("5 notes of 100 tokens were minted to Alice, those notes were consumed,");
    println!("and then Alice sent 5 separate 50-token notes to 5 different users.");

    Ok(())
}
```

Let's run the `src/main.rs` program again:

```bash
cargo run --release
```

The output will look like this:

```
Latest block: 17795

[STEP 1] Creating a new account for Alice
Alice's account ID: "0xebc34ec1637352100000cb8699d5c0"

[STEP 2] Deploying a new fungible faucet.
Faucet account ID: "0xd0736ee6005c0e200000d6f081f0ef"

[STEP 3] Minting 5 notes of 100 tokens each for Alice.
tx request built
Minted note #1 of 100 tokens for Alice.
tx request built
Minted note #2 of 100 tokens for Alice.
tx request built
Minted note #3 of 100 tokens for Alice.
tx request built
Minted note #4 of 100 tokens for Alice.
tx request built
Minted note #5 of 100 tokens for Alice.
All 5 notes minted for Alice successfully!

[STEP 4] Alice will now consume all of her notes to consolidate them.
Currently, Alice has 3 consumable notes. Waiting...
Found 5 consumable notes for Alice. Consuming them now...
All of Alice's notes consumed successfully.

[STEP 5] Alice sends 5 notes of 50 tokens each to 5 different users.
Creating multiple P2ID notes for 4 target accounts in one transaction...
Submitted a transaction with 4 P2ID notes.
Submitting one more single P2ID transaction...

All steps completed successfully!
Alice created a wallet, a faucet was deployed,
5 notes of 100 tokens were minted to Alice, those notes were consumed,
and then Alice sent 5 separate 50-token notes to 5 different users.
```

### Running the example

To run a full working example navigate to the `rust-client` directory in the [miden-tutorials](https://github.com/0xPolygonMiden/miden-tutorials/) repository and run this command:

```bash
cd rust-client
cargo run --release --bin create_mint_consume_send
```

### Continue learning

Next tutorial: [Deploying a Counter Contract](counter_contract_tutorial.md)
