# Interacting with Public Smart Contracts

_Using the Miden client in Rust to interact with public smart contracts on Miden_

## Overview

In the previous tutorial, we built a simple counter contract and deployed it to the Miden testnet. However, we only covered how the contract’s deployer could interact with it. Now, let’s explore how anyone can interact with a public smart contract on Miden.

We’ll retrieve the counter contract’s state from the chain and rebuild it locally so a local transaction can be executed against it. In the near future, Miden will support network transactions, making the process of submitting transactions to public smart contracts much more like traditional blockchains.

Just like in the previous tutorial, we will use a script to invoke the increment function within the counter contract to update the count. However, this tutorial demonstrates how to call a procedure in a smart contract that was deployed by a different user on Miden.

## What we'll cover

- Reading state from a public smart contract
- Interacting with public smart contracts on Miden

## Prerequisites

This tutorial assumes you have a basic understanding of Miden assembly and completed the previous tutorial on deploying the counter contract. Although not a requirement, it is recommended to complete the counter contract deployment tutorial before starting this tutorial.

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

Inside of the `masm/accounts/` directory, create the `counter.masm` file:

```masm
use.miden::account
use.std::sys

export.get_count
    # => []
    push.0

    # => [index]
    exec.account::get_item

    # => [count]
    exec.sys::truncate_stack
end

# => []
export.increment_count
    # => []
    push.0

    # => [index]
    exec.account::get_item

    # => [count]
    push.1 add

    # debug statement with client
    debug.stack

    # => [count+1]
    push.0

    # [index, count+1]
    exec.account::set_item

    # => []
    exec.sys::truncate_stack
end
```

Inside of the `masm/scripts/` directory, create the `counter_script.masm` file:

```masm
use.external_contract::counter_contract

begin
    call.counter_contract::increment_count
end

```

**Note**: _We explained in the previous counter contract tutorial what exactly happens at each step in the `increment_count` procedure._

### Step 3: Set up your `src/main.rs` file

Copy and paste the following code into your `src/main.rs` file:

```rust,no_run
use std::{fs, path::Path, sync::Arc};

use miden_assembly::{
    ast::{Module, ModuleKind},
    LibraryPath,
};
use miden_client::{
    account::Address,
    builder::ClientBuilder,
    keystore::FilesystemKeyStore,
    rpc::{Endpoint, TonicRpcClient},
    transaction::{TransactionKernel, TransactionRequestBuilder},
    ClientError, ScriptBuilder,
};
use miden_objects::{assembly::Assembler, assembly::DefaultSourceManager};

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

## Step 4: Reading public state from a smart contract

To read the public storage state of a smart contract on Miden we either instantiate the `TonicRpcClient` by itself, or use the `test_rpc_api()` method on the `Client` instance. In this example, we will be using the `test_rpc_api()` method.

We will be reading the public storage state of the counter contract deployed on the testnet at address `0x303dd027d27adc0000012b07dbf1b4`.

Add the following code snippet to the end of your `src/main.rs` function:

```rust,no_run
# use std::{fs, path::Path, sync::Arc};

# use miden_assembly::{
#     ast::{Module, ModuleKind},
#     LibraryPath,
# };
# use miden_client::{
#     account::Address,
#     builder::ClientBuilder,
#     keystore::FilesystemKeyStore,
#     rpc::{Endpoint, TonicRpcClient},
#     transaction::{TransactionKernel, TransactionRequestBuilder},
#     ClientError, ScriptBuilder,
# };
# use miden_objects::{assembly::Assembler, assembly::DefaultSourceManager};

# fn create_library(
#     assembler: Assembler,
#     library_path: &str,
#     source_code: &str,
# ) -> Result<miden_assembly::Library, Box<dyn std::error::Error>> {
#     let source_manager = Arc::new(DefaultSourceManager::default());
#     let module = Module::parser(ModuleKind::Library).parse_str(
#         LibraryPath::new(library_path)?,
#         source_code,
#         &source_manager,
#     )?;
#     let library = assembler.clone().assemble_library([module])?;
#     Ok(library)
# }

# #[tokio::main]
# async fn main() -> Result<(), ClientError> {
#     // Initialize client
#     let endpoint = Endpoint::testnet();
#     let timeout_ms = 10_000;
#     let rpc_api = Arc::new(TonicRpcClient::new(&endpoint, timeout_ms));
#     let keystore = FilesystemKeyStore::new("./keystore".into()).unwrap().into();

#     let mut client = ClientBuilder::new()
#         .rpc(rpc_api)
#         .authenticator(keystore)
#         .in_debug_mode(true.into())
#         .build()
#         .await?;

#     let sync_summary = client.sync_state().await.unwrap();
#     println!("Latest block: {}", sync_summary.block_num);




// -------------------------------------------------------------------------
// STEP 1: Read the Public State of the Counter Contract
// -------------------------------------------------------------------------
println!("\n[STEP 1] Reading data from public state");

// Define the Counter Contract account id from counter contract deploy
let (_network_id, address) =
    Address::from_bech32("mtst1qrhk9zc2au2vxqzaynaz5ddhs4cqqghmajy").unwrap();
let counter_contract_id = match address {
    Address::AccountId(account_id_address) => account_id_address.id(),
    _ => panic!("Expected AccountId address"),
};

client
    .import_account_by_id(counter_contract_id)
    .await
    .unwrap();

let counter_contract_details = client.get_account(counter_contract_id).await.unwrap();

let counter_contract = if let Some(account_record) = counter_contract_details {
    // Clone the account to get an owned instance
    let account = account_record.account().clone();
    println!(
        "Account details: {:?}",
        account.storage().slots().first().unwrap()
    );
    account // Now returns an owned account
} else {
    panic!("Counter contract not found!");
};
#     Ok(())
# }
```

Run the following command to execute src/main.rs:

```bash
cargo run --release
```

After the program executes, you should see the counter contract count value and nonce printed to the terminal, for example:

```text
count val: [0, 0, 0, 5]
counter nonce: 5
```

## Step 5: Importing a public account

Add the following code snippet to the end of your `src/main.rs` function:

```rust,no_run
# use std::{fs, path::Path, sync::Arc};

# use miden_assembly::{
#     ast::{Module, ModuleKind},
#     LibraryPath,
# };
# use miden_client::{
#     account::Address,
#     builder::ClientBuilder,
#     keystore::FilesystemKeyStore,
#     rpc::{Endpoint, TonicRpcClient},
#     transaction::{TransactionKernel, TransactionRequestBuilder},
#     ClientError, ScriptBuilder,
# };
# use miden_objects::{assembly::Assembler, assembly::DefaultSourceManager};

# fn create_library(
#     assembler: Assembler,
#     library_path: &str,
#     source_code: &str,
# ) -> Result<miden_assembly::Library, Box<dyn std::error::Error>> {
#     let source_manager = Arc::new(DefaultSourceManager::default());
#     let module = Module::parser(ModuleKind::Library).parse_str(
#         LibraryPath::new(library_path)?,
#         source_code,
#         &source_manager,
#     )?;
#     let library = assembler.clone().assemble_library([module])?;
#     Ok(library)
# }

# #[tokio::main]
# async fn main() -> Result<(), ClientError> {
#     // Initialize client
#     let endpoint = Endpoint::testnet();
#     let timeout_ms = 10_000;
#     let rpc_api = Arc::new(TonicRpcClient::new(&endpoint, timeout_ms));
#     let keystore = FilesystemKeyStore::new("./keystore".into()).unwrap().into();

#     let mut client = ClientBuilder::new()
#         .rpc(rpc_api)
#         .authenticator(keystore)
#         .in_debug_mode(true.into())
#         .build()
#         .await?;

#     let sync_summary = client.sync_state().await.unwrap();
#     println!("Latest block: {}", sync_summary.block_num);

#     // -------------------------------------------------------------------------
#     // STEP 1: Read the Public State of the Counter Contract
#     // -------------------------------------------------------------------------
#     println!("\n[STEP 1] Reading data from public state");

#     // Define the Counter Contract account id from counter contract deploy
#     let (_network_id, address) =
#         Address::from_bech32("mtst1qrhk9zc2au2vxqzaynaz5ddhs4cqqghmajy").unwrap();
#     let counter_contract_id = match address {
#         Address::AccountId(account_id_address) => account_id_address.id(),
#         _ => panic!("Expected AccountId address"),
#     };

#     client
#         .import_account_by_id(counter_contract_id)
#         .await
#         .unwrap();

#     let counter_contract_details = client.get_account(counter_contract_id).await.unwrap();

#     let counter_contract = if let Some(account_record) = counter_contract_details {
#         // Clone the account to get an owned instance
#         let account = account_record.account().clone();
#         println!(
#             "Account details: {:?}",
#             account.storage().slots().first().unwrap()
#         );
#         account // Now returns an owned account
#     } else {
#         panic!("Counter contract not found!");
#     };

#
// -------------------------------------------------------------------------
// STEP 2: Call the Counter Contract with a script
// -------------------------------------------------------------------------

// Load the MASM script referencing the increment procedure
let script_path = Path::new("./masm/scripts/counter_script.masm");
let script_code = fs::read_to_string(script_path).unwrap();

let counter_path = Path::new("./masm/accounts/counter.masm");
let counter_code = fs::read_to_string(counter_path).unwrap();

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
#     Ok(())
# }
```

## Summary

The final `src/main.rs` file should look like this:

```rust
use std::{fs, path::Path, sync::Arc};

use miden_assembly::{
    ast::{Module, ModuleKind},
    LibraryPath,
};
use miden_client::{
    account::Address,
    builder::ClientBuilder,
    keystore::FilesystemKeyStore,
    rpc::{Endpoint, TonicRpcClient},
    transaction::{TransactionKernel, TransactionRequestBuilder},
    ClientError, ScriptBuilder,
};
use miden_objects::{assembly::Assembler, assembly::DefaultSourceManager};

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
    // STEP 1: Read the Public State of the Counter Contract
    // -------------------------------------------------------------------------
    println!("\n[STEP 1] Reading data from public state");

    // Define the Counter Contract account id from counter contract deploy
    let (_network_id, address) =
        Address::from_bech32("mtst1qrhk9zc2au2vxqzaynaz5ddhs4cqqghmajy").unwrap();
    let counter_contract_id = match address {
        Address::AccountId(account_id_address) => account_id_address.id(),
        _ => panic!("Expected AccountId address"),
    };

    client
        .import_account_by_id(counter_contract_id)
        .await
        .unwrap();

    let counter_contract_details = client.get_account(counter_contract_id).await.unwrap();

    let counter_contract = if let Some(account_record) = counter_contract_details {
        // Clone the account to get an owned instance
        let account = account_record.account().clone();
        println!(
            "Account details: {:?}",
            account.storage().slots().first().unwrap()
        );
        account // Now returns an owned account
    } else {
        panic!("Counter contract not found!");
    };

    // -------------------------------------------------------------------------
    // STEP 2: Call the Counter Contract with a script
    // -------------------------------------------------------------------------

    // Load the MASM script referencing the increment procedure
    let script_path = Path::new("./masm/scripts/counter_script.masm");
    let script_code = fs::read_to_string(script_path).unwrap();

    let counter_path = Path::new("./masm/accounts/counter.masm");
    let counter_code = fs::read_to_string(counter_path).unwrap();

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

Run the following command to execute src/main.rs:

```bash
cargo run --release
```

The output of our program will look something like this depending on the current count value in the smart contract:

```text
Client initialized successfully.
Latest block: 242342

[STEP 1] Building counter contract from public state
count val: [0, 0, 0, 1]
counter nonce: 1

[STEP 2] Call the increment_count procedure in the counter contract
Procedure 1: "0x92495ca54d519eb5e4ba22350f837904d3895e48d74d8079450f19574bb84cb6"
Procedure 2: "0xecd7eb223a5524af0cc78580d96357b298bb0b3d33fe95aeb175d6dab9de2e54"
number of procedures: 2
Final script:
begin
    # => []
    call.0xecd7eb223a5524af0cc78580d96357b298bb0b3d33fe95aeb175d6dab9de2e54
end
Stack state before step 1812:
├──  0: 2
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

View transaction on MidenScan: https://testnet.midenscan.com/tx/0x8183aed150f20b9c26d4cb7840bfc92571ea45ece31116170b11cdff2649eb5c
counter contract storage: Ok(RpoDigest([0, 0, 0, 2]))
```

### Running the example

To run the full example, navigate to the `rust-client` directory in the [miden-tutorials](https://github.com/0xMiden/miden-tutorials/) repository and run this command:

```bash
cd rust-client
cargo run --release --bin counter_contract_increment
```

### Continue learning

Next tutorial: [Foreign Procedure Invocation](foreign_procedure_invocation_tutorial.md)
