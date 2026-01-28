---
title: "Deploying a Counter Contract"
sidebar_position: 4
---

# Deploying a Counter Contract

_Using the Miden client in Rust to deploy and interact with a custom smart contract on Miden_

## Overview

In this tutorial, we will build a simple counter smart contract that maintains a count, deploy it to the Miden testnet, and interact with it by incrementing the count. You can also deploy the counter contract on a locally running Miden node, similar to previous tutorials.

Using a script, we will invoke the increment function within the counter contract to update the count. This tutorial provides a foundational understanding of developing and deploying custom smart contracts on Miden.

## What we'll cover

- Deploying a custom smart contract on Miden
- Getting up to speed with the basics of Miden assembly
- Calling procedures in an account
- Pure vs state changing procedures

## Prerequisites

This tutorial assumes you have a basic understanding of Miden assembly. To quickly get up to speed with Miden assembly (MASM), please play around with running basic Miden assembly programs in the [Miden playground](https://0xMiden.github.io/examples/).

## Step 1: Initialize your repository

Create a new Rust repository for your Miden project and navigate to it with the following command:

```bash
cargo new miden-counter-contract
cd miden-counter-contract
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

### Set up your `src/main.rs` file

In the previous section, we explained how to instantiate the Miden client. We can reuse the same `initialize_client` function for our counter contract.

Copy and paste the following code into your `src/main.rs` file:

```rust no_run
use miden_client::auth::NoAuth;
use miden_client::transaction::TransactionKernel;
use rand::RngCore;
use std::{fs, path::Path, sync::Arc};

use miden_client::{
    address::NetworkId,
    assembly::{
        Assembler,
        CodeBuilder,
        DefaultSourceManager,
        Module,
        ModuleKind,
        Path as AssemblyPath,
    },
    builder::ClientBuilder,
    keystore::FilesystemKeyStore,
    rpc::{Endpoint, GrpcClient},
    store::AccountRecordData,
    transaction::TransactionRequestBuilder,
    ClientError,
};
use miden_client_sqlite_store::ClientBuilderSqliteExt;
use miden_client::{
    account::{
        AccountBuilder, AccountComponent, AccountStorageMode, AccountType, StorageSlot,
        StorageSlotName,
    },
    Word,
};

fn create_library(
    assembler: Assembler,
    library_path: &str,
    source_code: &str,
) -> Result<miden_client::assembly::Library, Box<dyn std::error::Error>> {
    let source_manager = Arc::new(DefaultSourceManager::default());
    let module = Module::parser(ModuleKind::Library).parse_str(
        AssemblyPath::new(library_path),
        source_code,
        source_manager.clone(),
    )?;
    let library = assembler.clone().assemble_library([module])?;
    Ok(library)
}

#[tokio::main]
async fn main() -> Result<(), ClientError> {
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

    Ok(())
}
```

_When running the code above, there will be some unused imports, however, we will use these imports later on in the tutorial._

**Note**: Running the code above, will generate a `store.sqlite3` file and a `keystore` directory. The Miden client uses the `store.sqlite3` file to keep track of the state of accounts and notes. The `keystore` directory keeps track of private keys used by accounts. Be sure to add both to your `.gitignore`!

## Step 2: Build the counter contract

For better code organization, we will separate the Miden assembly code from our Rust code.

Create a directory named `masm` at the **root** of your `miden-counter-contract` directory. This will contain our contract and script masm code.

Initialize the `masm` directory:

```bash
mkdir -p masm/accounts masm/scripts
```

This will create:

```text
masm/
├── accounts/
└── scripts/
```

### Custom Miden smart contract

Below is our counter contract. It has a two exported procedures: `get_count` and `increment_count`.

At the beginning of the MASM file, we define our imports. In this case, we import
`miden::protocol::active_account`, `miden::protocol::native_account`, `miden::core::word`, and
`miden::core::sys`.

The `miden::protocol::active_account` and `miden::protocol::native_account` modules contain
procedures for reading and writing contract state. We use `miden::core::word` to convert the slot
name into a slot ID for the account storage APIs.

The import `miden::core::sys` contains a useful procedure for truncating the operand stack at the
end of a procedure.

#### Here's a breakdown of what the `get_count` procedure does:

1. Pushes the slot ID prefix and suffix for `miden::tutorials::counter` onto the stack.
2. Calls `active_account::get_item` with the slot ID.
3. Calls `sys::truncate_stack` to truncate the stack to size 16.
4. The value returned from `active_account::get_item` is still on the stack and will be returned
   when this procedure is called.

#### Here's a breakdown of what the `increment_count` procedure does:

1. Pushes the slot ID prefix and suffix for `miden::tutorials::counter` onto the stack.
2. Calls `active_account::get_item` with the slot ID.
3. Pushes `1` onto the stack.
4. Adds `1` to the count value returned from `active_account::get_item`.
5. Pushes the slot ID prefix and suffix again so we can write the updated count.
6. Calls `native_account::set_item` which saves the incremented count to storage.
7. Calls `sys::truncate_stack` to clean up the stack.

Inside of the `masm/accounts/` directory, create the `counter.masm` file:

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

**Note**: _It's a good habit to add comments below each line of MASM code with the expected stack state. This improves readability and helps with debugging._

### Authentication Component

**Important**: Starting with Miden Client 0.10.0, all accounts must have an authentication component. For smart contracts that don't require authentication (like our counter contract), we use a `NoAuth` component.

This `NoAuth` component allows any user to interact with the smart contract without requiring signature verification.

### Custom script

This is a Miden assembly script that will call the `increment_count` procedure during the transaction.

The string `{increment_count}` will be replaced with the hash of the `increment_count` procedure in our rust program.

Inside of the `masm/scripts/` directory, create the `counter_script.masm` file:

```masm
use external_contract::counter_contract

begin
    call.counter_contract::increment_count
end
```

## Step 3: Build the counter smart contract

To build the counter contract copy and paste the following code at the end of your `src/main.rs` file:

```rust ignore
// -------------------------------------------------------------------------
// STEP 1: Create a basic counter contract
// -------------------------------------------------------------------------
println!("\n[STEP 1] Creating counter contract.");

// Load the MASM file for the counter contract
let counter_path = Path::new("../masm/accounts/counter.masm");
let counter_code = fs::read_to_string(counter_path).unwrap();

// Compile the account code into `AccountComponent` with one storage slot
let counter_slot_name =
    StorageSlotName::new("miden::tutorials::counter").expect("valid slot name");
let component_code = CodeBuilder::new()
    .compile_component_code("external_contract::counter_contract", &counter_code)
    .unwrap();
let counter_component = AccountComponent::new(
    component_code,
    vec![StorageSlot::with_value(counter_slot_name.clone(), Word::default())],
)
.unwrap()
.with_supports_all_types();

// Init seed for the counter contract
let mut seed = [0_u8; 32];
client.rng().fill_bytes(&mut seed);

// Build the new `Account` with the component
let counter_contract = AccountBuilder::new(seed)
    .account_type(AccountType::RegularAccountImmutableCode)
    .storage_mode(AccountStorageMode::Public)
    .with_component(counter_component.clone())
    .with_auth_component(NoAuth)
    .build()
    .unwrap();

println!(
    "counter_contract commitment: {:?}",
    counter_contract.commitment()
);
println!("counter_contract id: {:?}", counter_contract.id());
println!("counter_contract storage: {:?}", counter_contract.storage());

client.add_account(&counter_contract, false).await.unwrap();
```

Run the following command to execute `src/main.rs`:

```bash
cargo run --release
```

After the program executes, you should see the counter contract hash and contract id printed to the terminal, for example:

```text
[STEP 1] Creating counter contract.
counter_contract commitment: RpoDigest([3700134472268167470, 14878091556015233722, 3335592073702485043, 16978997897830363420])
counter_contract id: "mtst1qql030hpsp0yyqra494lcwazxsym7add"
counter_contract storage: AccountStorage { slots: [Value([0, 0, 0, 0]), Value([0, 0, 0, 0])] }
```

## Step 4: Incrementing the count

Now that we built the counter contract, lets create a transaction request to increment the count:

Paste the following code at the end of your `src/main.rs` file:

```rust ignore
// -------------------------------------------------------------------------
// STEP 2: Call the Counter Contract with a script
// -------------------------------------------------------------------------
println!("\n[STEP 2] Call Counter Contract With Script");

// Load the MASM script referencing the increment procedure
let script_path = Path::new("../masm/scripts/counter_script.masm");
let script_code = fs::read_to_string(script_path).unwrap();

// Create a library from the counter contract code
let assembler = TransactionKernel::assembler();
let account_component_lib = create_library(
    assembler.clone(),
    "external_contract::counter_contract",
    &counter_code,
)
.unwrap();

let tx_script = client
    .code_builder()
    .with_dynamically_linked_library(&account_component_lib)
    .unwrap()
    .compile_tx_script(&script_code)
    .unwrap();

// Build a transaction request with the custom script
let tx_increment_request = TransactionRequestBuilder::new()
    .custom_script(tx_script)
    .build()
    .unwrap();

// Execute and submit the transaction
let tx_id = client
    .submit_new_transaction(counter_contract.id(), tx_increment_request)
    .await
    .unwrap();

println!(
    "View transaction on MidenScan: https://testnet.midenscan.com/tx/{:?}",
    tx_id
);

println!(
    "Counter contract id: {:?}",
    counter_contract.id().to_bech32(NetworkId::Testnet)
);

client.sync_state().await.unwrap();

// Retrieve updated contract data to see the incremented counter
let account_record = client
    .get_account(counter_contract.id())
    .await
    .unwrap()
    .expect("counter contract not found");
let account = match account_record.account_data() {
    AccountRecordData::Full(account) => account,
    AccountRecordData::Partial(_) => panic!("counter contract is missing full account data"),
};
println!(
    "counter contract storage: {:?}",
    account.storage().get_item(&counter_slot_name)
);
```

**Note**: _Once our counter contract is deployed, other users can increment the count of the smart contract simply by knowing the account id of the contract and the procedure hash of the `increment_count` procedure._

## Summary

The final `src/main.rs` file should look like this:

```rust no_run
use miden_client::auth::NoAuth;
use miden_client::transaction::TransactionKernel;
use rand::RngCore;
use std::{fs, path::Path, sync::Arc};

use miden_client::{
    address::NetworkId,
    assembly::{
        Assembler,
        CodeBuilder,
        DefaultSourceManager,
        Module,
        ModuleKind,
        Path as AssemblyPath,
    },
    builder::ClientBuilder,
    keystore::FilesystemKeyStore,
    rpc::{Endpoint, GrpcClient},
    store::AccountRecordData,
    transaction::TransactionRequestBuilder,
    ClientError,
};
use miden_client_sqlite_store::ClientBuilderSqliteExt;
use miden_client::{
    account::{
        AccountBuilder, AccountComponent, AccountStorageMode, AccountType, StorageSlot,
        StorageSlotName,
    },
    Word,
};

fn create_library(
    assembler: Assembler,
    library_path: &str,
    source_code: &str,
) -> Result<miden_client::assembly::Library, Box<dyn std::error::Error>> {
    let source_manager = Arc::new(DefaultSourceManager::default());
    let module = Module::parser(ModuleKind::Library).parse_str(
        AssemblyPath::new(library_path),
        source_code,
        source_manager.clone(),
    )?;
    let library = assembler.clone().assemble_library([module])?;
    Ok(library)
}

#[tokio::main]
async fn main() -> Result<(), ClientError> {
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
    // STEP 1: Create a basic counter contract
    // -------------------------------------------------------------------------
    println!("\n[STEP 1] Creating counter contract.");

    // Load the MASM file for the counter contract
    let counter_path = Path::new("../masm/accounts/counter.masm");
    let counter_code = fs::read_to_string(counter_path).unwrap();

    // Compile the account code into `AccountComponent` with one storage slot
    let counter_slot_name =
        StorageSlotName::new("miden::tutorials::counter").expect("valid slot name");
    let component_code = CodeBuilder::new()
        .compile_component_code("external_contract::counter_contract", &counter_code)
        .unwrap();
    let counter_component = AccountComponent::new(
        component_code,
        vec![StorageSlot::with_value(counter_slot_name.clone(), Word::default())],
    )
    .unwrap()
    .with_supports_all_types();

    // Init seed for the counter contract
    let mut seed = [0_u8; 32];
    client.rng().fill_bytes(&mut seed);

    // Build the new `Account` with the component
    let counter_contract = AccountBuilder::new(seed)
        .account_type(AccountType::RegularAccountImmutableCode)
        .storage_mode(AccountStorageMode::Public)
        .with_component(counter_component.clone())
        .with_auth_component(NoAuth)
        .build()
        .unwrap();

    println!(
        "counter_contract commitment: {:?}",
        counter_contract.commitment()
    );
    println!("counter_contract id: {:?}", counter_contract.id());
    println!("counter_contract storage: {:?}", counter_contract.storage());

    client.add_account(&counter_contract, false).await.unwrap();

    // -------------------------------------------------------------------------
    // STEP 2: Call the Counter Contract with a script
    // -------------------------------------------------------------------------
    println!("\n[STEP 2] Call Counter Contract With Script");

    // Load the MASM script referencing the increment procedure
    let script_path = Path::new("../masm/scripts/counter_script.masm");
    let script_code = fs::read_to_string(script_path).unwrap();

    // Create a library from the counter contract code
    let assembler = TransactionKernel::assembler();
    let account_component_lib = create_library(
        assembler.clone(),
        "external_contract::counter_contract",
        &counter_code,
    )
    .unwrap();

    let tx_script = client
        .code_builder()
        .with_dynamically_linked_library(&account_component_lib)
        .unwrap()
        .compile_tx_script(&script_code)
        .unwrap();

    // Build a transaction request with the custom script
    let tx_increment_request = TransactionRequestBuilder::new()
        .custom_script(tx_script)
        .build()
        .unwrap();

    // Execute and submit the transaction
    let tx_id = client
        .submit_new_transaction(counter_contract.id(), tx_increment_request)
        .await
        .unwrap();

    println!(
        "View transaction on MidenScan: https://testnet.midenscan.com/tx/{:?}",
        tx_id
    );

    println!(
        "Counter contract id: {:?}",
        counter_contract.id().to_bech32(NetworkId::Testnet)
    );

    client.sync_state().await.unwrap();

    // Retrieve updated contract data to see the incremented counter
    let account_record = client
        .get_account(counter_contract.id())
        .await
        .unwrap()
        .expect("counter contract not found");
    let account = match account_record.account_data() {
        AccountRecordData::Full(account) => account,
        AccountRecordData::Partial(_) => panic!("counter contract is missing full account data"),
    };
    println!(
        "counter contract storage: {:?}",
        account.storage().get_item(&counter_slot_name)
    );

    Ok(())
}
```

The output of our program will look something like this:

```text
Latest block: 374255

[STEP 1] Creating counter contract.
one or more warnings were emitted
counter_contract commitment: Word([3964727668949550262, 4265714847747507878, 5784293172192015964, 16803438753763367241])
counter_contract id: "mtst1qre73e6qcrfevqqngx8wewvveacqqjh8p2a"
counter_contract storage: AccountStorage { slots: [Value(Word([0, 0, 0, 0]))] }

[STEP 2] Call Counter Contract With Script
Stack state before step 2610:
├──  0: 1
├──  1: 0
├──  2: 0
├──  3: 0
├──  4: 0
├──  5: 0
├──  6: 0
├──  7: 0
├──  8: 0
├──  9: 0
├── 10: 0
├── 11: 0
├── 12: 0
├── 13: 0
├── 14: 0
├── 15: 0
├── 16: 0
├── 17: 0
├── 18: 0
└── 19: 0

└── (0 more items)

View transaction on MidenScan: https://testnet.midenscan.com/tx/0x9767940bbed7bd3a74c24dc43f1ea8fe90a876dc7925621c217f648c63c4ab7a
counter contract storage: Ok(Word([0, 0, 0, 1]))
```

The line in the output `Stack state before step 2505` ouputs the stack state when we call "debug.stack" in the `counter.masm` file.

To increment the count of the counter contract all you need is to know the account id of the counter and the procedure hash of the `increment_count` procedure. To increment the count without deploying the counter each time, you can modify the program above to hardcode the account id of the counter and the procedure hash of the `increment_count` prodedure in the masm script.

### Running the example

To run the full example, navigate to the `rust-client` directory in the [miden-tutorials](https://github.com/0xMiden/miden-tutorials/) repository and run this command:

```bash
cd rust-client
cargo run --release --bin counter_contract_deploy
```

### Continue learning

Next tutorial: [Interacting with Public Smart Contracts](public_account_interaction_tutorial.md)
