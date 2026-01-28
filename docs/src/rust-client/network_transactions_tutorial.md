---
title: "Network Transactions on Miden"
sidebar_position: 6
---

# Network Transactions on Miden

_Using the Miden client in Rust to deploy and interact with smart contracts using network transactions_

## Overview

In this tutorial, we will explore Network Transactions (NTXs) on Miden - a powerful feature that enables autonomous smart contract execution and public shared state management. Unlike local transactions that require users to execute and prove, network transactions are executed and proven by a network transaction builder.

We'll build a network counter smart contract using the same MASM code as the regular counter, but with different storage configuration in Rust to enable network execution.

## What we'll cover

- Understanding Network Transactions and when to use them
- Deploying smart contracts with network storage mode
- Using transaction scripts to initialize network contracts on-chain
- Creating network notes for user interactions
- Validating network transaction results

## Prerequisites

This tutorial assumes you have completed the [counter contract tutorial](counter_contract_tutorial.md) and understand basic Miden assembly.

## What are Network Transactions?

Network transactions are executed and proven by the Miden operator rather than the client. They are useful for:

- **Public shared state**: Multiple users can interact with the same contract state without race conditions
- **Autonomous execution**: Smart contracts can execute when conditions are met without user intervention
- **Resource-constrained devices**: Clients that can't generate ZK proofs efficiently
- **AMM applications**: Using network notes, you can build sophisticated AMMs where trades execute automatically

The main trade-off is reduced privacy since the operator can see transaction inputs.

## Step 1: Initialize your repository

Create a new Rust repository for your Miden project and navigate to it:

```bash
cargo new miden-network-transactions
cd miden-network-transactions
```

Add the following dependencies to your `Cargo.toml` file:

```toml
[dependencies]
miden-client = { version = "0.13.0", features = ["testing", "tonic"] }
miden-client-sqlite-store = { version = "0.13.0", package = "miden-client-sqlite-store" }
miden-protocol = { version = "0.13.0" }
rand = { version = "0.9" }
serde = { version = "1", features = ["derive"] }
serde_json = { version = "1.0", features = ["raw_value"] }
tokio = { version = "1.46", features = ["rt-multi-thread", "net", "macros", "fs"] }
rand_chacha = "0.9.0"
```

## Step 2: Set up MASM files

Create the directory structure:

```bash
mkdir -p masm/accounts masm/scripts masm/notes
```

### Counter Contract

We'll use the same counter contract MASM code as the regular counter tutorial. The key difference is in the Rust configuration, not the MASM code.

Create `masm/accounts/counter.masm`:

```masm
use miden::protocol::active_account
use miden::protocol::native_account
use miden::core::word
use miden::core::sys

const COUNTER_SLOT = word("miden::tutorials::counter")

#! Inputs:  []
#! Outputs: [count]
pub proc get_count
    push.COUNTER_SLOT[0..2] exec.active_account::get_item
    # => [count]

    exec.sys::truncate_stack
    # => [count]
end

#! Inputs:  []
#! Outputs: []
pub proc increment_count
    push.COUNTER_SLOT[0..2] exec.active_account::get_item
    # => [count]

    add.1
    # => [count+1]

    push.COUNTER_SLOT[0..2] exec.native_account::set_item
    # => []

    exec.sys::truncate_stack
    # => []
end
```

### Transaction Script for Deployment

Create `masm/scripts/counter_script.masm`:

```masm
use external_contract::counter_contract

begin
    call.counter_contract::increment_count
end
```

This script executes a function call (increment) that creates a necessary state change for our contract to be deployed and stored on the network on-chain. In Miden, network contracts must have their state modified through a transaction to be properly registered and committed to the blockchain - simply creating the account isn't sufficient for network storage mode.

### Network Note for User Interaction

Create `masm/notes/network_increment_note.masm`:

```masm
use external_contract::counter_contract

begin
    call.counter_contract::increment_count
end
```

After deployment, users will interact with the contract through these network notes.

## Step 3: Initialize the client and create a user account

Before deploying the network account and creating network notes, we need to set up the client and create a user account that will interact with our network contract.

Copy and paste the following code into your `src/main.rs` file:

```rust no_run
use std::{fs, path::Path, sync::Arc};

use miden_client::account::component::BasicWallet;
use miden_client::{
    address::NetworkId,
    auth::AuthSecretKey,
    crypto::FeltRng,
    builder::ClientBuilder,
    keystore::FilesystemKeyStore,
    note::{
        Note, NoteAssets, NoteInputs, NoteMetadata, NoteRecipient, NoteTag, NoteType,
    },
    rpc::{Endpoint, GrpcClient},
    store::{AccountRecordData, TransactionFilter},
    transaction::{OutputNote, TransactionId, TransactionRequestBuilder, TransactionStatus},
    Client, ClientError, Felt, Word,
};
use miden_client_sqlite_store::ClientBuilderSqliteExt;
use miden_client::auth::{self, AuthFalcon512Rpo};
use miden_client::transaction::TransactionKernel;
use miden_client::{
    account::{
        AccountBuilder, AccountComponent, AccountStorageMode, AccountType, StorageSlot,
        StorageSlotName,
    },
    assembly::{
        Assembler,
        CodeBuilder,
        DefaultSourceManager,
        Library,
        Module,
        ModuleKind,
        Path as AssemblyPath,
    },
};
use rand::RngCore;
use tokio::time::{sleep, Duration};

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

/// Creates a Miden library from the provided account code and library path.
fn create_library(
    account_code: String,
    library_path: &str,
) -> Result<Library, Box<dyn std::error::Error>> {
    let assembler: Assembler = TransactionKernel::assembler();
    let source_manager = Arc::new(DefaultSourceManager::default());
    let module = Module::parser(ModuleKind::Library).parse_str(
        AssemblyPath::new(library_path),
        account_code,
        source_manager.clone(),
    )?;
    let library = assembler.clone().assemble_library([module])?;
    Ok(library)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize client
    let endpoint = Endpoint::testnet();
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
    // STEP 1: Create Basic User Account
    // -------------------------------------------------------------------------
    println!("\n[STEP 1] Creating a new account for Alice");

    // Account seed
    let mut init_seed = [0_u8; 32];
    client.rng().fill_bytes(&mut init_seed);

    let key_pair = AuthSecretKey::new_falcon512_rpo();

    // Build the account
    let alice_account = AccountBuilder::new(init_seed)
        .account_type(AccountType::RegularAccountUpdatableCode)
        .storage_mode(AccountStorageMode::Public)
        .with_auth_component(AuthFalcon512Rpo::new(key_pair.public_key().to_commitment()))
        .with_component(BasicWallet)
        .build()
        .unwrap();

    // Add the account to the client
    client.add_account(&alice_account, false).await?;

    // Add the key pair to the keystore
    keystore.add_key(&key_pair).unwrap();

    println!(
        "Alice's account ID: {:?}",
        alice_account.id().to_bech32(NetworkId::Testnet)
    );

    Ok(())
}
```

This step initializes the Miden client and creates a basic user account (Alice) that will interact with our network contract.

## Step 4: Create the network counter smart contract

Now we'll create a network smart contract. The key difference from regular contracts is using `AccountStorageMode::Network` instead of `AccountStorageMode::Public`.

Add this code to your `main()` function:

```rust ignore
// -------------------------------------------------------------------------
// STEP 2: Create Network Counter Smart Contract
// -------------------------------------------------------------------------
println!("\n[STEP 2] Creating a network counter smart contract");

let counter_code = fs::read_to_string(Path::new("../masm/accounts/counter.masm")).unwrap();

// Create the network counter smart contract account
// First, compile the MASM code into an account component
let counter_slot_name =
    StorageSlotName::new("miden::tutorials::counter").expect("valid slot name");
let component_code = CodeBuilder::new()
    .compile_component_code("external_contract::counter_contract", &counter_code)
    .unwrap();
let counter_component = AccountComponent::new(
    component_code,
    vec![StorageSlot::with_value(counter_slot_name.clone(), [Felt::new(0); 4].into())], // Initialize counter storage to 0
)
.unwrap()
.with_supports_all_types();

// Generate a random seed for the account
let mut init_seed = [0_u8; 32];
client.rng().fill_bytes(&mut init_seed);

// Build the immutable network account with no authentication
let counter_contract = AccountBuilder::new(init_seed)
    .account_type(AccountType::RegularAccountImmutableCode) // Immutable code
    .storage_mode(AccountStorageMode::Network) // Stored on network
    .with_auth_component(auth::NoAuth) // No authentication required
    .with_component(counter_component)
    .build()
    .unwrap();

client.add_account(&counter_contract, false).await.unwrap();

println!(
    "contract id: {:?}",
    counter_contract.id().to_bech32(NetworkId::Testnet)
);
```

This step creates a network smart contract with `AccountStorageMode::Network`, which enables the contract to be executed by the network operator.

## Step 5: Deploy the network account with a transaction script

We use a transaction script to deploy the network account and ensure it's properly registered on-chain. The script calls the `increment` function, which initializes the counter to 1.

Add this code to your `main()` function:

```rust ignore
// -------------------------------------------------------------------------
// STEP 3: Deploy Network Account with Transaction Script
// -------------------------------------------------------------------------
println!("\n[STEP 3] Deploy network counter smart contract");

let script_code = fs::read_to_string(Path::new("../masm/scripts/counter_script.masm")).unwrap();

let account_code = fs::read_to_string(Path::new("../masm/accounts/counter.masm")).unwrap();
let library_path = "external_contract::counter_contract";

let library = create_library(account_code, library_path).unwrap();

let tx_script = client
    .code_builder()
    .with_dynamically_linked_library(&library)?
    .compile_tx_script(&script_code)?;

let tx_increment_request = TransactionRequestBuilder::new()
    .custom_script(tx_script)
    .build()
    .unwrap();

let tx_id = client
    .submit_new_transaction(counter_contract.id(), tx_increment_request)
    .await
    .unwrap();

println!(
    "View transaction on MidenScan: https://testnet.midenscan.com/tx/{:?}",
    tx_id
);

// Wait for the transaction to be committed
wait_for_tx(&mut client, tx_id).await.unwrap();
```

This step uses a transaction script to deploy the network account and ensure it's properly registered on-chain. The script calls the `increment` function, which initializes the counter to 1.

## Step 6: Create a network note for user interaction

We create a public note that the network operator can consume to execute the increment function. This increments the counter from 1 to 2.

Add this code to your `main()` function:

```rust ignore
// -------------------------------------------------------------------------
// STEP 4: Prepare & Create the Network Note
// -------------------------------------------------------------------------
println!("\n[STEP 4] Creating a network note for network counter contract");

let network_note_code =
    fs::read_to_string(Path::new("../masm/notes/network_increment_note.masm")).unwrap();
let account_code = fs::read_to_string(Path::new("../masm/accounts/counter.masm")).unwrap();

let library_path = "external_contract::counter_contract";
let library = create_library(account_code, library_path).unwrap();

// Create and submit the network note that will increment the counter
// Generate a random serial number for the note
let serial_num = client.rng().draw_word();

// Compile the note script with the counter contract library
let note_script = client
    .code_builder()
    .with_dynamically_linked_library(&library)?
    .compile_note_script(&network_note_code)?;

// Create note recipient with empty inputs
let note_inputs = NoteInputs::new([].to_vec())?;
let recipient = NoteRecipient::new(serial_num, note_script, note_inputs);

// Set up note metadata - tag it with the counter contract ID so it gets consumed
let tag = NoteTag::with_account_target(counter_contract.id());
let metadata = NoteMetadata::new(alice_account.id(), NoteType::Public, tag);

// Create the complete note
let increment_note = Note::new(NoteAssets::default(), metadata, recipient);

// Build and submit the transaction containing the note
let note_req = TransactionRequestBuilder::new()
    .own_output_notes(vec![OutputNote::Full(increment_note)])
    .build()?;

let note_tx_id = client
    .submit_new_transaction(alice_account.id(), note_req)
    .await?;

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

let mut last_val = None;
for _ in 0..10 {
    client.sync_state().await?;

    // Checking updated state
    let new_account_state = client.get_account(counter_contract.id()).await.unwrap();

    if let Some(account_record) = new_account_state.as_ref() {
        let account = match account_record.account_data() {
            AccountRecordData::Full(account) => account,
            AccountRecordData::Partial(_) => {
                panic!("counter contract is missing full account data")
            }
        };
        let count: Word = account.storage().get_item(&counter_slot_name).unwrap().into();
        let val = count.get(3).unwrap().as_int();
        if val >= 2 {
            println!("🔢 Final counter value: {}", val);
            return Ok(());
        }
        last_val = Some(val);
    }

    // Give the network note builder time to process the note.
    sleep(Duration::from_secs(6)).await;
}

if let Some(val) = last_val {
    println!(
        "Counter value did not reach 2 yet (last observed value: {}).",
        val
    );
} else {
    println!("Counter value not available yet.");
}
```

This step creates a public note that the network operator can consume to execute the increment function. This increments the counter from 1 to 2.

## Summary

Your complete `main()` function should look like this:

```rust no_run
use std::{fs, path::Path, sync::Arc};

use miden_client::account::component::BasicWallet;
use miden_client::{
    address::NetworkId,
    auth::AuthSecretKey,
    crypto::FeltRng,
    builder::ClientBuilder,
    keystore::FilesystemKeyStore,
    note::{
        Note, NoteAssets, NoteInputs, NoteMetadata, NoteRecipient, NoteTag, NoteType,
    },
    rpc::{Endpoint, GrpcClient},
    store::{AccountRecordData, TransactionFilter},
    transaction::{OutputNote, TransactionId, TransactionRequestBuilder, TransactionStatus},
    Client, ClientError, Felt, Word,
};
use miden_client_sqlite_store::ClientBuilderSqliteExt;
use miden_client::auth::{self, AuthFalcon512Rpo};
use miden_client::transaction::TransactionKernel;
use miden_client::{
    account::{
        AccountBuilder, AccountComponent, AccountStorageMode, AccountType, StorageSlot,
        StorageSlotName,
    },
    assembly::{
        Assembler,
        CodeBuilder,
        DefaultSourceManager,
        Library,
        Module,
        ModuleKind,
        Path as AssemblyPath,
    },
};
use rand::RngCore;
use tokio::time::{sleep, Duration};

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

/// Creates a Miden library from the provided account code and library path.
fn create_library(
    account_code: String,
    library_path: &str,
) -> Result<Library, Box<dyn std::error::Error>> {
    let assembler: Assembler = TransactionKernel::assembler();
    let source_manager = Arc::new(DefaultSourceManager::default());
    let module = Module::parser(ModuleKind::Library).parse_str(
        AssemblyPath::new(library_path),
        account_code,
        source_manager.clone(),
    )?;
    let library = assembler.clone().assemble_library([module])?;
    Ok(library)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize client
    let endpoint = Endpoint::testnet();
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
    // STEP 1: Create Basic User Account
    // -------------------------------------------------------------------------
    println!("\n[STEP 1] Creating a new account for Alice");

    // Account seed
    let mut init_seed = [0_u8; 32];
    client.rng().fill_bytes(&mut init_seed);

    let key_pair = AuthSecretKey::new_falcon512_rpo();

    // Build the account
    let alice_account = AccountBuilder::new(init_seed)
        .account_type(AccountType::RegularAccountUpdatableCode)
        .storage_mode(AccountStorageMode::Public)
        .with_auth_component(AuthFalcon512Rpo::new(key_pair.public_key().to_commitment()))
        .with_component(BasicWallet)
        .build()
        .unwrap();

    // Add the account to the client
    client.add_account(&alice_account, false).await?;

    // Add the key pair to the keystore
    keystore.add_key(&key_pair).unwrap();

    println!(
        "Alice's account ID: {:?}",
        alice_account.id().to_bech32(NetworkId::Testnet)
    );

    // -------------------------------------------------------------------------
    // STEP 2: Create Network Counter Smart Contract
    // -------------------------------------------------------------------------
    println!("\n[STEP 2] Creating a network counter smart contract");

    let counter_code = fs::read_to_string(Path::new("../masm/accounts/counter.masm")).unwrap();

    // Create the network counter smart contract account
    // First, compile the MASM code into an account component
    let counter_slot_name =
        StorageSlotName::new("miden::tutorials::counter").expect("valid slot name");
    let component_code = CodeBuilder::new()
        .compile_component_code("external_contract::counter_contract", &counter_code)
        .unwrap();
    let counter_component = AccountComponent::new(
        component_code,
        vec![StorageSlot::with_value(counter_slot_name.clone(), [Felt::new(0); 4].into())], // Initialize counter storage to 0
    )
    .unwrap()
    .with_supports_all_types();

    // Generate a random seed for the account
    let mut init_seed = [0_u8; 32];
    client.rng().fill_bytes(&mut init_seed);

    // Build the immutable network account with no authentication
    let counter_contract = AccountBuilder::new(init_seed)
        .account_type(AccountType::RegularAccountImmutableCode) // Immutable code
        .storage_mode(AccountStorageMode::Network) // Stored on network
        .with_auth_component(auth::NoAuth) // No authentication required
        .with_component(counter_component)
        .build()
        .unwrap();

    client.add_account(&counter_contract, false).await.unwrap();

    println!(
        "contract id: {:?}",
        counter_contract.id().to_bech32(NetworkId::Testnet)
    );

    // -------------------------------------------------------------------------
    // STEP 3: Deploy Network Account with Transaction Script
    // -------------------------------------------------------------------------
    println!("\n[STEP 3] Deploy network counter smart contract");

    let script_code = fs::read_to_string(Path::new("../masm/scripts/counter_script.masm")).unwrap();

    let account_code = fs::read_to_string(Path::new("../masm/accounts/counter.masm")).unwrap();
    let library_path = "external_contract::counter_contract";

    let library = create_library(account_code, library_path).unwrap();

    let tx_script = client
        .code_builder()
        .with_dynamically_linked_library(&library)?
        .compile_tx_script(&script_code)?;

    let tx_increment_request = TransactionRequestBuilder::new()
        .custom_script(tx_script)
        .build()
        .unwrap();

    let tx_id = client
        .submit_new_transaction(counter_contract.id(), tx_increment_request)
        .await
        .unwrap();

    println!(
        "View transaction on MidenScan: https://testnet.midenscan.com/tx/{:?}",
        tx_id
    );

    // Wait for the transaction to be committed
    wait_for_tx(&mut client, tx_id).await.unwrap();

    // -------------------------------------------------------------------------
    // STEP 4: Prepare & Create the Network Note
    // -------------------------------------------------------------------------
    println!("\n[STEP 4] Creating a network note for network counter contract");

    let network_note_code =
        fs::read_to_string(Path::new("../masm/notes/network_increment_note.masm")).unwrap();
    let account_code = fs::read_to_string(Path::new("../masm/accounts/counter.masm")).unwrap();

    let library_path = "external_contract::counter_contract";
    let library = create_library(account_code, library_path).unwrap();

    // Create and submit the network note that will increment the counter
    // Generate a random serial number for the note
    let serial_num = client.rng().draw_word();

    // Compile the note script with the counter contract library
    let note_script = client
        .code_builder()
        .with_dynamically_linked_library(&library)?
        .compile_note_script(&network_note_code)?;

    // Create note recipient with empty inputs
    let note_inputs = NoteInputs::new([].to_vec())?;
    let recipient = NoteRecipient::new(serial_num, note_script, note_inputs);

    // Set up note metadata - tag it with the counter contract ID so it gets consumed
    let tag = NoteTag::with_account_target(counter_contract.id());
    let metadata = NoteMetadata::new(alice_account.id(), NoteType::Public, tag);

    // Create the complete note
    let increment_note = Note::new(NoteAssets::default(), metadata, recipient);

    // Build and submit the transaction containing the note
    let note_req = TransactionRequestBuilder::new()
        .own_output_notes(vec![OutputNote::Full(increment_note)])
        .build()?;

    let note_tx_id = client
        .submit_new_transaction(alice_account.id(), note_req)
        .await?;

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

    let mut last_val = None;
    for _ in 0..10 {
        client.sync_state().await?;

        // Checking updated state
        let new_account_state = client.get_account(counter_contract.id()).await.unwrap();

        if let Some(account_record) = new_account_state.as_ref() {
            let account = match account_record.account_data() {
                AccountRecordData::Full(account) => account,
                AccountRecordData::Partial(_) => {
                    panic!("counter contract is missing full account data")
                }
            };
            let count: Word = account.storage().get_item(&counter_slot_name).unwrap().into();
            let val = count.get(3).unwrap().as_int();
            if val >= 2 {
                println!("🔢 Final counter value: {}", val);
                return Ok(());
            }
            last_val = Some(val);
        }

        // Give the network note builder time to process the note.
        sleep(Duration::from_secs(6)).await;
    }

    if let Some(val) = last_val {
        println!(
            "Counter value did not reach 2 yet (last observed value: {}).",
            val
        );
    } else {
        println!("Counter value not available yet.");
    }

    Ok(())
}
```

## Step 7: Running the Example

To run the complete network transaction example:

```bash
cd rust-client
cargo run --release --bin network_notes_counter_contract
```

Expected output:

```text
Latest block: 508977

[STEP 1] Creating a new account for Alice
Alice's account ID: "mtst1qrpk3gmyv2p06ypgh7gss9hs0gl80gwl"

[STEP 2] Creating a network counter smart contract
contract id: "mtst1qz95e5k55xeh5sz2zann5xtp4uq9hpht"

[STEP 3] Deploy network counter smart contract
View transaction on MidenScan: https://testnet.midenscan.com/tx/0xbe8dddab0403544a28c9a24d0400837cfd639b030670cf436ba113261fbdfce0
✅ transaction 0xbe8dddab0403544a28c9a24d0400837cfd639b030670cf436ba113261fbdfce0 committed

[STEP 4] Creating a network note for network counter contract
View transaction on MidenScan: https://testnet.midenscan.com/tx/0x0bb5f6b786eb0f129d944975e3fae226084441eaf422f187657afbd74641327c
network increment note created, waiting for onchain commitment
✅ transaction 0x0bb5f6b786eb0f129d944975e3fae226084441eaf422f187657afbd74641327c committed
🔢 Final counter value: 2
```

## Summary

Network transactions on Miden enable powerful use cases by allowing the operator to execute transactions on behalf of users. The key steps are:

1. **Create user account**: Standard account creation for interaction
2. **Create network account**: Use `AccountStorageMode::Network` instead of `Public`
3. **Deploy with transaction script**: Ensures the contract is registered on-chain
4. **Interact with network notes**: Users create public notes that the operator executes

The same MASM code works for both regular and network contracts - the difference is purely in the Rust configuration. This makes network transactions a powerful tool for building applications like AMMs where multiple users need to interact with shared state efficiently.

### Continue learning

Next tutorial: [How To Create Notes with Custom Logic](custom_note_how_to.md)
