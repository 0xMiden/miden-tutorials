# Creating Accounts and Faucets

_Using the Miden client in Rust to create accounts and deploy faucets_

## Overview

In this tutorial, we will create a Miden account for _Alice_ and deploy a fungible faucet. In the next section, we will mint tokens from the faucet to fund her account and transfer tokens from Alice's account to other Miden accounts.

## What we'll cover

- Understanding the differences between public and private accounts & notes
- Instantiating the Miden client
- Creating new accounts (public or private)
- Deploying a faucet to fund an account

## Prerequisites

Before you begin, ensure that a Miden node is running locally in a separate terminal window. To get the Miden node running locally, you can follow the instructions on the [Miden Node Setup](../miden_node_setup.md) page.

## Public vs. private accounts & notes

Before diving into coding, let's clarify the concepts of public and private accounts & notes on Miden:

- Public accounts: The account's data and code are stored on-chain and are openly visible, including its assets.
- Private accounts: The account's state and logic are off-chain, only known to its owner.
- Public notes: The note's state is visible to anyone - perfect for scenarios where transparency is desired.
- Private notes: The note's state is stored off-chain, you will need to share the note data with the relevant parties (via email or Telegram) for them to be able to consume the note.

Note: _The term "account" can be used interchangeably with the term "smart contract" since account abstraction on Miden is handled natively._

_It is useful to think of notes on Miden as "cryptographic cashier's checks" that allow users to send tokens. If the note is private, the note transfer is only known to the sender and receiver._

## Step 1: Initialize your repository

Create a new Rust repository for your Miden project and navigate to it with the following command:

```bash
cargo new miden-rust-client
cd miden-rust-client
```

Add the following dependencies to your `Cargo.toml` file:

```toml
[dependencies]
miden-client = { version = "0.8.1", features = ["testing", "concurrent", "tonic", "sqlite"] }
miden-lib = { version = "0.8", default-features = false }
miden-objects = { version = "0.8", default-features = false }
miden-crypto = { version = "0.14.0", features = ["executable"] }
miden-assembly = "0.14.0"
rand = { version = "0.9" }
serde = { version = "1", features = ["derive"] }
serde_json = { version = "1.0", features = ["raw_value"] }
tokio = { version = "1.40", features = ["rt-multi-thread", "net", "macros"] }
rand_chacha = "0.9.0"
```

## Step 2: Initialize the client

Before interacting with the Miden network, we must instantiate the client. In this step, we specify several parameters:

- **RPC endpoint** - The URL of the Miden node you will connect to.
- **Client RNG** - The random number generator used by the client, ensuring that the serial number of newly created notes are unique.
- **SQLite Store** – An SQL database used by the client to store account and note data.
- **Authenticator** - The component responsible for generating transaction signatures.

Copy and paste the following code into your `src/main.rs` file.

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

    let keystore: FilesystemKeyStore<rand::prelude::StdRng> =
        FilesystemKeyStore::new("./keystore".into()).unwrap();

    Ok(())
}
```

_When running the code above, there will be some unused imports, however, we will use these imports later on in the tutorial._

**Note**: Running the code above, will generate a `store.sqlite3` file and a `keystore` directory. The Miden client uses the `store.sqlite3` file to keep track of the state of accounts and notes. The `keystore` directory keeps track of private keys used by accounts. Be sure to add both to your `.gitignore`!

Run the following command to execute `src/main.rs`:

```bash
cargo run --release
```

After the program executes, you should see the latest block number printed to the terminal, for example:

```
Latest block number: 3855
```

## Step 3: Creating a wallet

Now that we've initialized the client, we can create a wallet for Alice.

To create a wallet for Alice using the Miden client, we define the account type as mutable or immutable and specify whether it is public or private. A mutable wallet means you can change the account code after deployment. A wallet on Miden is simply an account with standardized code.

In the example below we create a mutable public account for Alice.

Add this snippet to the end of your file in the `main()` function:

```rust
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
```

## Step 4: Deploying a fungible faucet

To provide Alice with testnet assets, we must first deploy a faucet. A faucet account on Miden mints fungible tokens.

We'll create a public faucet with a token symbol, decimals, and a max supply. We will use this faucet to mint tokens to Alice's account in the next section.

Add this snippet to the end of your file in the `main()` function:

```rust
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
```

_When tokens are minted from this faucet, each token batch is represented as a "note" (UTXO). You can think of a Miden Note as a cryptographic cashier's check that has certain spend conditions attached to it._

## Summary

Your updated `main()` function in `src/main.rs` should look like this:

```rust
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

    Ok(())
}
```

Let's run the `src/main.rs` program again:

```bash
cargo run --release
```

The output will look like this:

```
Latest block: 17771

[STEP 1] Creating a new account for Alice
Alice's account ID: "0x3cb3e596d14ad410000017901eaa7b"

[STEP 2] Deploying a new fungible faucet.
Faucet account ID: "0x6ad1894ac233e4200000088311bb6b"
```

In this section we explained how to instantiate the Miden client, create a wallet account, and deploy a faucet.

In the next section we will cover how to mint tokens from the faucet, consume notes, and send tokens to other accounts.

### Running the example

To run a full working example navigate to the `rust-client` directory in the [miden-tutorials](https://github.com/0xPolygonMiden/miden-tutorials/) repository and run this command:

```bash
cd rust-client
cargo run --release --bin create_mint_consume_send
```

### Continue learning

Next tutorial: [Mint, Consume, and Create Notes](mint_consume_create_tutorial.md)
