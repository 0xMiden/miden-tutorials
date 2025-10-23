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
miden-client = { version = "0.11", features = ["testing", "tonic", "sqlite"] }
miden-lib = { version = "0.11", default-features = false }
miden-objects = { version = "0.11", default-features = false, features = ["testing"] }
miden-crypto = { version = "0.15.9", features = ["executable"] }
miden-assembly = "0.17.0"
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
use miden_lib::account::auth::NoAuth;
use rand::RngCore;
use std::{fs, path::Path, sync::Arc};

use miden_assembly::{
    ast::{Module, ModuleKind},
    LibraryPath,
};
use miden_client::{
    account::{
        AccountBuilder, AccountIdAddress, AccountStorageMode, AccountType, Address,
        AddressInterface, StorageSlot,
    },
    builder::ClientBuilder,
    keystore::FilesystemKeyStore,
    rpc::{Endpoint, TonicRpcClient},
    transaction::{TransactionKernel, TransactionRequestBuilder},
    ClientError, Felt, ScriptBuilder,
};
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

#[tokio::main]
async fn main() -> Result<(), ClientError> {
    // Initialize client
    let endpoint = Endpoint::testnet();
    let timeout_ms = 10_000;
    let rpc_api = Arc::new(TonicRpcClient::new(&endpoint, timeout_ms));
    let keystore = FilesystemKeyStore::new("./keystore".into()).unwrap().into();

    let mut client = ClientBuilder::new()
        .rpc(rpc_api)
        .authenticator(keystore)
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

At the beginning of the MASM file, we define our imports. In this case, we import `miden::account` and `std::sys`.

The import `miden::account` contains useful procedures for interacting with a smart contract's state.

The import `std::sys` contains a useful procedure for truncating the operand stack at the end of a procedure.

#### Here's a breakdown of what the `get_count` procedure does:

1. Pushes `0` (COUNTER_SLOT) onto the stack, representing the index of the storage slot to read.
2. Calls `account::get_item` with the index of `0`.
3. Calls `sys::truncate_stack` to truncate the stack to size 16.
4. The value returned from `account::get_item` is still on the stack and will be returned when this procedure is called.

#### Here's a breakdown of what the `increment_count` procedure does:

1. Pushes `0` (COUNTER_SLOT) onto the stack, representing the index of the storage slot to read.
2. Calls `account::get_item` with the index of `0`.
3. Pushes `1` onto the stack.
4. Adds `1` to the count value returned from `account::get_item`.
5. _For demonstration purposes_, calls `debug.stack` to see the state of the stack
6. Pushes `0` (COUNTER_SLOT) onto the stack, which is the index of the storage slot we want to write to.
7. Calls `account::set_item` which saves the incremented count to storage at index `0`
8. Calls `sys::truncate_stack` to truncate the stack to size 16.

Inside of the `masm/accounts/` directory, create the `counter.masm` file:

```masm
use.miden::account
use.std::sys

const.COUNTER_SLOT=0
# => []
export.get_count
    push.COUNTER_SLOT
    # => [index]

    exec.account::get_item
    # => [count]
end

# => []
export.increment_count
    push.COUNTER_SLOT
    # => [index]

    exec.account::get_item
    # => [count]

    push.1 add
    # => [count+1]

    # debug statement with client
    debug.stack

    push.COUNTER_SLOT
    # [index, count+1]

    exec.account::set_item
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
use.external_contract::counter_contract

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

// Prepare assembler (debug mode = true)
let assembler: Assembler = TransactionKernel::assembler().with_debug_mode(true);

// Load the MASM file for the counter contract
let counter_path = Path::new("./masm/accounts/counter.masm");
let counter_code = fs::read_to_string(counter_path).unwrap();

// Compile the account code into `AccountComponent` with one storage slot
let counter_component = AccountComponent::compile(
    counter_code.clone(),
    assembler,
    vec![StorageSlot::Value(
        [Felt::new(0), Felt::new(0), Felt::new(0), Felt::new(0)].into(),
    )],
)
.unwrap()
.with_supports_all_types();

// Init seed for the counter contract
let mut seed = [0_u8; 32];
client.rng().fill_bytes(&mut seed);

// Build the new `Account` with the component
let (counter_contract, counter_seed) = AccountBuilder::new(seed)
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
println!(
    "counter_contract id: {:?}",
    Address::from(AccountIdAddress::new(
        counter_contract.id(),
        AddressInterface::Unspecified
    ))
    .to_bech32(NetworkId::Testnet)
);
println!("counter_contract storage: {:?}", counter_contract.storage());

client
    .add_account(&counter_contract.clone(), Some(counter_seed), false)
    .await
    .unwrap();
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
let script_path = Path::new("./masm/scripts/counter_script.masm");
let script_code = fs::read_to_string(script_path).unwrap();

let assembler: Assembler = TransactionKernel::assembler().with_debug_mode(true);
let account_component_lib = create_library(
    assembler.clone(),
    "external_contract::counter_contract",
    &counter_code,
)
.unwrap();

let tx_script = ScriptBuilder::new(true)
    .with_dynamically_linked_library(&account_component_lib)
    .unwrap()
    .compile_tx_script(script_code)
    .unwrap();

// Build a transaction request with the custom script
let tx_increment_request = TransactionRequestBuilder::new()
    .custom_script(tx_script)
    .build()
    .unwrap();

// Execute the transaction locally
let tx_result = client
    .new_transaction(counter_contract.id(), tx_increment_request)
    .await
    .unwrap();

let tx_id = tx_result.executed_transaction().id();
println!(
    "View transaction on MidenScan: https://testnet.midenscan.com/tx/{:?}",
    tx_id
);

// Submit transaction to the network
let _ = client.submit_transaction(tx_result).await;

client.sync_state().await.unwrap();

// Retrieve updated contract data to see the incremented counter
let account = client.get_account(counter_contract.id()).await.unwrap();
println!(
    "counter contract storage: {:?}",
    account.unwrap().account().storage().get_item(0)
);
```

**Note**: _Once our counter contract is deployed, other users can increment the count of the smart contract simply by knowing the account id of the contract and the procedure hash of the `increment_count` procedure._

## Summary

The final `src/main.rs` file should look like this:

```rust
use miden_lib::account::auth::NoAuth;
use rand::RngCore;
use std::{fs, path::Path, sync::Arc};

use miden_assembly::{
    ast::{Module, ModuleKind},
    LibraryPath,
};
use miden_client::{
    account::{
        AccountBuilder, AccountIdAddress, AccountStorageMode, AccountType, Address,
        AddressInterface, StorageSlot,
    },
    builder::ClientBuilder,
    keystore::FilesystemKeyStore,
    rpc::{Endpoint, TonicRpcClient},
    transaction::{TransactionKernel, TransactionRequestBuilder},
    ClientError, Felt, ScriptBuilder,
};
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

#[tokio::main]
async fn main() -> Result<(), ClientError> {
    // Initialize client
    let endpoint = Endpoint::testnet();
    let timeout_ms = 10_000;
    let rpc_api = Arc::new(TonicRpcClient::new(&endpoint, timeout_ms));
    let keystore = FilesystemKeyStore::new("./keystore".into()).unwrap().into();

    let mut client = ClientBuilder::new()
        .rpc(rpc_api)
        .authenticator(keystore)
        .in_debug_mode(true.into())
        .build()
        .await?;

    let sync_summary = client.sync_state().await.unwrap();
    println!("Latest block: {}", sync_summary.block_num);

    // -------------------------------------------------------------------------
    // STEP 1: Create a basic counter contract
    // -------------------------------------------------------------------------
    println!("\n[STEP 1] Creating counter contract.");

    // Prepare assembler (debug mode = true)
    let assembler: Assembler = TransactionKernel::assembler().with_debug_mode(true);

    // Load the MASM file for the counter contract
    let counter_path = Path::new("./masm/accounts/counter.masm");
    let counter_code = fs::read_to_string(counter_path).unwrap();

    // Compile the account code into `AccountComponent` with one storage slot
    let counter_component = AccountComponent::compile(
        counter_code.clone(),
        assembler,
        vec![StorageSlot::Value(
            [Felt::new(0), Felt::new(0), Felt::new(0), Felt::new(0)].into(),
        )],
    )
    .unwrap()
    .with_supports_all_types();

    // Init seed for the counter contract
    let mut seed = [0_u8; 32];
    client.rng().fill_bytes(&mut seed);

    // Build the new `Account` with the component
    let (counter_contract, counter_seed) = AccountBuilder::new(seed)
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
    println!(
        "counter_contract id: {:?}",
        Address::from(AccountIdAddress::new(
            counter_contract.id(),
            AddressInterface::Unspecified
        ))
        .to_bech32(NetworkId::Testnet)
    );
    println!("counter_contract storage: {:?}", counter_contract.storage());

    client
        .add_account(&counter_contract.clone(), Some(counter_seed), false)
        .await
        .unwrap();

    // -------------------------------------------------------------------------
    // STEP 2: Call the Counter Contract with a script
    // -------------------------------------------------------------------------
    println!("\n[STEP 2] Call Counter Contract With Script");

    // Load the MASM script referencing the increment procedure
    let script_path = Path::new("./masm/scripts/counter_script.masm");
    let script_code = fs::read_to_string(script_path).unwrap();

    let assembler: Assembler = TransactionKernel::assembler().with_debug_mode(true);
    let account_component_lib = create_library(
        assembler.clone(),
        "external_contract::counter_contract",
        &counter_code,
    )
    .unwrap();

    let tx_script = ScriptBuilder::new(true)
        .with_dynamically_linked_library(&account_component_lib)
        .unwrap()
        .compile_tx_script(script_code)
        .unwrap();

    // Build a transaction request with the custom script
    let tx_increment_request = TransactionRequestBuilder::new()
        .custom_script(tx_script)
        .build()
        .unwrap();

    // Execute the transaction locally
    let tx_result = client
        .new_transaction(counter_contract.id(), tx_increment_request)
        .await
        .unwrap();

    let tx_id = tx_result.executed_transaction().id();
    println!(
        "View transaction on MidenScan: https://testnet.midenscan.com/tx/{:?}",
        tx_id
    );

    // Submit transaction to the network
    let _ = client.submit_transaction(tx_result).await;

    client.sync_state().await.unwrap();

    // Retrieve updated contract data to see the incremented counter
    let account = client.get_account(counter_contract.id()).await.unwrap();
    println!(
        "counter contract storage: {:?}",
        account.unwrap().account().storage().get_item(0)
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
