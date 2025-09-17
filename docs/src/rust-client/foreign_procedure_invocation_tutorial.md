# Foreign Procedure Invocation Tutorial

_Using foreign procedure invocation to craft read-only cross-contract calls in the Miden VM_

## Overview

In previous tutorials we deployed a public counter contract and incremented the count from a different client instance.

In this tutorial we will cover the basics of "foreign procedure invocation" (FPI) in the Miden VM. To demonstrate FPI, we will build a "count copy" smart contract that reads the count from our previously deployed counter contract and copies the count to its own local storage.

Foreign procedure invocation (FPI) is a powerful tool for building smart contracts in the Miden VM. FPI allows one smart contract to call "read-only" procedures in other smart contracts.

The term "foreign procedure invocation" might sound a bit verbose, but it is as simple as one smart contract calling a non-state modifying procedure in another smart contract. The "EVM equivalent" of foreign procedure invocation would be a smart contract calling a read-only function in another contract.

FPI is useful for developing smart contracts that extend the functionality of existing contracts on Miden. FPI is the core primitive used by price oracles on Miden.

## What We Will Build

![count copy FPI diagram](../assets/count_copy_fpi_diagram.png)

The diagram above depicts the "count copy" smart contract using foreign procedure invocation to read the count state of the counter contract. After reading the state via FPI, the "count copy" smart contract writes the value returned from the counter contract to storage.

## What we'll cover

- Foreign Procedure Invocation (FPI)
- Building a "count copy" Smart Contract

## Prerequisites

This tutorial assumes you have a basic understanding of Miden assembly and completed the previous tutorial on deploying the counter contract. We will be working within the same `miden-counter-contract` repository that we created in the [Interacting with Public Smart Contracts](./public_account_interaction_tutorial.md) tutorial.

## Step 1: Set up your repository

We will be using the same repository used in the "Interacting with Public Smart Contracts" tutorial. To set up your repository for this tutorial, first follow up until step two [here](./public_account_interaction_tutorial.md).

## Step 2: Set up the "count reader" contract

Inside of the `masm/accounts/` directory, create the `count_reader.masm` file. This is the smart contract that will read the "count" value from the counter contract.

`masm/accounts/count_reader.masm`:

```masm
use.miden::account
use.miden::tx
use.std::sys

# => [account_id_prefix, account_id_suffix, get_count_proc_hash]
export.copy_count
    exec.tx::execute_foreign_procedure
    # => [count]

    debug.stack
    # => [count]

    push.0
    # [index, count]

    exec.account::set_item
    # => []

    exec.sys::truncate_stack
    # => []
end
```

In the count reader smart contract we have a `copy_count` procedure that uses `tx::execute_foreign_procedure` to call the `get_count` procedure in the counter contract.

To call the `get_count` procedure, we push its hash along with the counter contract's ID suffix and prefix.

This is what the stack state should look like before we call `tx::execute_foreign_procedure`:

```text
# => [account_id_prefix, account_id_suffix, GET_COUNT_HASH]
```

After calling the `get_count` procedure in the counter contract, we call `debug.stack` and then save the count of the counter contract to index 0 in storage.

**Note**: _The bracket symbols used in the count copy contract are not valid MASM syntax. These are simply placeholder elements that we will replace with the actual values before compilation._

Inside the `masm/scripts/` directory, create the `reader_script.masm` file:

```masm
use.external_contract::count_reader_contract
use.std::sys

begin
    # => []
    push.{get_count_proc_hash}

    # => [GET_COUNT_HASH]
    push.{account_id_suffix}

    # => [account_id_suffix]
    push.{account_id_prefix}

    # => []
    push.111 debug.stack drop
    call.count_reader_contract::copy_count

    exec.sys::truncate_stack
end
```

**Note**: _`push.{get_count_proc_hash}` is not valid MASM, we will format the string with the value get_count_proc_hash before passing this script code to the assembler._

### Step 3: Set up your `src/main.rs` file:

```rust,no_run
use rand::RngCore;
use std::{fs, path::Path, sync::Arc, time::Duration};
use tokio::time::sleep;

use miden_assembly::{
    ast::{Module, ModuleKind},
    LibraryPath,
};
use miden_client::{
    account::{AccountBuilder, AccountStorageMode, AccountType, StorageSlot},
    account::{AccountIdAddress, Address, AddressInterface},
    builder::ClientBuilder,
    keystore::FilesystemKeyStore,
    rpc::{domain::account::AccountStorageRequirements, Endpoint, TonicRpcClient},
    transaction::{ForeignAccount, TransactionKernel, TransactionRequestBuilder},
    ClientError, ScriptBuilder,
};
use miden_crypto::Word;
use miden_lib::account::auth::NoAuth;
use miden_objects::{
    account::{AccountComponent, NetworkId},
    assembly::{Assembler, DefaultSourceManager},
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
    // STEP 1: Create the Count Reader Contract
    // -------------------------------------------------------------------------
    println!("\n[STEP 1] Creating count reader contract.");

    // Load the MASM file for the counter contract
    let count_reader_path = Path::new("./masm/accounts/count_reader.masm");
    let count_reader_code = fs::read_to_string(count_reader_path).unwrap();

    // Prepare assembler (debug mode = true)
    let assembler: Assembler = TransactionKernel::assembler().with_debug_mode(true);

    // Compile the account code into `AccountComponent` with one storage slot
    let count_reader_component = AccountComponent::compile(
        count_reader_code.clone(),
        assembler.clone(),
        vec![StorageSlot::Value(Word::default())],
    )
    .unwrap()
    .with_supports_all_types();

    // Init seed for the counter contract
    let mut init_seed = [0_u8; 32];
    client.rng().fill_bytes(&mut init_seed);

    // Build the new `Account` with the component
    let (count_reader_contract, count_reader_seed) = AccountBuilder::new(init_seed)
        .account_type(AccountType::RegularAccountImmutableCode)
        .storage_mode(AccountStorageMode::Public)
        .with_component(count_reader_component.clone())
        .with_auth_component(NoAuth)
        .build()
        .unwrap();

    println!(
        "count_reader hash: {:?}",
        count_reader_contract.commitment()
    );
    println!(
        "contract id: {:?}",
        Address::from(AccountIdAddress::new(
            count_reader_contract.id(),
            AddressInterface::Unspecified
        ))
        .to_bech32(NetworkId::Devnet)
    );

    client
        .add_account(
            &count_reader_contract.clone(),
            Some(count_reader_seed),
            false,
        )
        .await
        .unwrap();

    Ok(())
}
```

Run the following command to execute src/main.rs:

```bash
cargo run --release
```

The output of our program will look something like this:

```text
Latest block: 226976

[STEP 1] Creating count reader contract.
count_reader hash: RpoDigest([15888177100833057221, 15548657445961063290, 5580812380698193124, 9604096693288041818])
contract id: "mtst1qp3ca3adt34euqqqqwt488x34qnnd495"
```

## Step 4: Build and read the state of the counter contract deployed on testnet

Add this snippet to the end of your file in the `main()` function that we created in the previous step:

```rust,no_run
# use rand::RngCore;
# use std::{fs, path::Path, sync::Arc, time::Duration};
# use tokio::time::sleep;
#
# use miden_assembly::{
#     ast::{Module, ModuleKind},
#     LibraryPath,
# };
# use miden_client::{
#     account::{AccountBuilder, AccountStorageMode, AccountType, StorageSlot},
#     account::{AccountIdAddress, Address, AddressInterface},
#     builder::ClientBuilder,
#     keystore::FilesystemKeyStore,
#     rpc::{domain::account::AccountStorageRequirements, Endpoint, TonicRpcClient},
#     transaction::{ForeignAccount, TransactionKernel, TransactionRequestBuilder},
#     ClientError, ScriptBuilder,
# };
# use miden_crypto::Word;
# use miden_lib::account::auth::NoAuth;
# use miden_objects::{
#     account::{AccountComponent, NetworkId},
#     assembly::{Assembler, DefaultSourceManager},
# };
#
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
#
# #[tokio::main]
# async fn main() -> Result<(), ClientError> {
#     // Initialize client
#     let endpoint = Endpoint::testnet();
#     let timeout_ms = 10_000;
#     let rpc_api = Arc::new(TonicRpcClient::new(&endpoint, timeout_ms));
#     let keystore = FilesystemKeyStore::new("./keystore".into()).unwrap().into();
#
#     let mut client = ClientBuilder::new()
#         .rpc(rpc_api)
#         .authenticator(keystore)
#         .in_debug_mode(true.into())
#         .build()
#         .await?;
#
#     let sync_summary = client.sync_state().await.unwrap();
#     println!("Latest block: {}", sync_summary.block_num);
#
#     // -------------------------------------------------------------------------
#     // STEP 1: Create the Count Reader Contract
#     // -------------------------------------------------------------------------
#     println!("\n[STEP 1] Creating count reader contract.");
#
#     // Load the MASM file for the counter contract
#     let count_reader_path = Path::new("./masm/accounts/count_reader.masm");
#     let count_reader_code = fs::read_to_string(count_reader_path).unwrap();
#
#     // Prepare assembler (debug mode = true)
#     let assembler: Assembler = TransactionKernel::assembler().with_debug_mode(true);
#
#     // Compile the account code into `AccountComponent` with one storage slot
#     let count_reader_component = AccountComponent::compile(
#         count_reader_code.clone(),
#         assembler.clone(),
#         vec![StorageSlot::Value(Word::default())],
#     )
#     .unwrap()
#     .with_supports_all_types();
#
#     // Init seed for the counter contract
#     let mut init_seed = [0_u8; 32];
#     client.rng().fill_bytes(&mut init_seed);
#
#     // Build the new `Account` with the component
#     let (count_reader_contract, count_reader_seed) = AccountBuilder::new(init_seed)
#         .account_type(AccountType::RegularAccountImmutableCode)
#         .storage_mode(AccountStorageMode::Public)
#         .with_component(count_reader_component.clone())
#         .with_auth_component(NoAuth)
#         .build()
#         .unwrap();
#
#     println!(
#         "count_reader hash: {:?}",
#         count_reader_contract.commitment()
#     );
#     println!(
#         "contract id: {:?}",
#         Address::from(AccountIdAddress::new(
#             count_reader_contract.id(),
#             AddressInterface::Unspecified
#         ))
#         .to_bech32(NetworkId::Devnet)
#     );
#
#     client
#         .add_account(
#             &count_reader_contract.clone(),
#             Some(count_reader_seed),
#             false,
#         )
#         .await
#         .unwrap();

// -------------------------------------------------------------------------
// STEP 2: Build & Get State of the Counter Contract
// -------------------------------------------------------------------------
println!("\n[STEP 2] Building counter contract from public state");

// Define the Counter Contract account id from counter contract deploy
let (_, counter_contract_address) =
    Address::from_bech32("mtst1qrhk9zc2au2vxqzaynaz5ddhs4cqqghmajy").unwrap();
let counter_contract_id = match counter_contract_address {
    Address::AccountId(account_id_address) => account_id_address.id(),
    _ => panic!("Expected AccountId address"),
};
println!("counter contract id: {:?}", counter_contract_id.to_hex());

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

This step uses the logic we explained in the [Public Account Interaction Tutorial](./public_account_interaction_tutorial.md) to read the state of the Counter contract and import it to the client locally.

## Step 5: Call the counter contract via foreign procedure invocation

Add this snippet to the end of your file in the `main()` function:

```rust,no_run
# use rand::RngCore;
# use std::{fs, path::Path, sync::Arc, time::Duration};
# use tokio::time::sleep;
#
# use miden_assembly::{
#     ast::{Module, ModuleKind},
#     LibraryPath,
# };
# use miden_client::{
#     account::{AccountBuilder, AccountStorageMode, AccountType, StorageSlot},
#     account::{AccountIdAddress, Address, AddressInterface},
#     builder::ClientBuilder,
#     keystore::FilesystemKeyStore,
#     rpc::{domain::account::AccountStorageRequirements, Endpoint, TonicRpcClient},
#     transaction::{ForeignAccount, TransactionKernel, TransactionRequestBuilder},
#     ClientError, ScriptBuilder,
# };
# use miden_crypto::Word;
# use miden_lib::account::auth::NoAuth;
# use miden_objects::{
#     account::{AccountComponent, NetworkId},
#     assembly::{Assembler, DefaultSourceManager},
# };
#
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
#
# #[tokio::main]
# async fn main() -> Result<(), ClientError> {
#     // Initialize client
#     let endpoint = Endpoint::testnet();
#     let timeout_ms = 10_000;
#     let rpc_api = Arc::new(TonicRpcClient::new(&endpoint, timeout_ms));
#     let keystore = FilesystemKeyStore::new("./keystore".into()).unwrap().into();
#
#     let mut client = ClientBuilder::new()
#         .rpc(rpc_api)
#         .authenticator(keystore)
#         .in_debug_mode(true.into())
#         .build()
#         .await?;
#
#     let sync_summary = client.sync_state().await.unwrap();
#     println!("Latest block: {}", sync_summary.block_num);
#
#     // -------------------------------------------------------------------------
#     // STEP 1: Create the Count Reader Contract
#     // -------------------------------------------------------------------------
#     println!("\n[STEP 1] Creating count reader contract.");
#
#     // Load the MASM file for the counter contract
#     let count_reader_path = Path::new("./masm/accounts/count_reader.masm");
#     let count_reader_code = fs::read_to_string(count_reader_path).unwrap();
#
#     // Prepare assembler (debug mode = true)
#     let assembler: Assembler = TransactionKernel::assembler().with_debug_mode(true);
#
#     // Compile the account code into `AccountComponent` with one storage slot
#     let count_reader_component = AccountComponent::compile(
#         count_reader_code.clone(),
#         assembler.clone(),
#         vec![StorageSlot::Value(Word::default())],
#     )
#     .unwrap()
#     .with_supports_all_types();
#
#     // Init seed for the counter contract
#     let mut init_seed = [0_u8; 32];
#     client.rng().fill_bytes(&mut init_seed);
#
#     // Build the new `Account` with the component
#     let (count_reader_contract, count_reader_seed) = AccountBuilder::new(init_seed)
#         .account_type(AccountType::RegularAccountImmutableCode)
#         .storage_mode(AccountStorageMode::Public)
#         .with_component(count_reader_component.clone())
#         .with_auth_component(NoAuth)
#         .build()
#         .unwrap();
#
#     println!(
#         "count_reader hash: {:?}",
#         count_reader_contract.commitment()
#     );
#     println!(
#         "contract id: {:?}",
#         Address::from(AccountIdAddress::new(
#             count_reader_contract.id(),
#             AddressInterface::Unspecified
#         ))
#         .to_bech32(NetworkId::Devnet)
#     );
#
#     client
#         .add_account(
#             &count_reader_contract.clone(),
#             Some(count_reader_seed),
#             false,
#         )
#         .await
#         .unwrap();
#
#     // -------------------------------------------------------------------------
#     // STEP 2: Build & Get State of the Counter Contract
#     // -------------------------------------------------------------------------
#     println!("\n[STEP 2] Building counter contract from public state");
#
#     // Define the Counter Contract account id from counter contract deploy
#     let (_, counter_contract_address) =
#         Address::from_bech32("mtst1qrhk9zc2au2vxqzaynaz5ddhs4cqqghmajy").unwrap();
#     let counter_contract_id = match counter_contract_address {
#         Address::AccountId(account_id_address) => account_id_address.id(),
#         _ => panic!("Expected AccountId address"),
#     };
#     println!("counter contract id: {:?}", counter_contract_id.to_hex());
#
#     client
#         .import_account_by_id(counter_contract_id)
#         .await
#         .unwrap();
#
#     let counter_contract_details = client.get_account(counter_contract_id).await.unwrap();
#
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
    // -------------------------------------------------------------------------
    // STEP 3: Call the Counter Contract via Foreign Procedure Invocation (FPI)
    // -------------------------------------------------------------------------
    println!("\n[STEP 3] Call counter contract with FPI from count copy contract");

    let counter_contract_path = Path::new("./masm/accounts/counter.masm");
    let counter_contract_code = fs::read_to_string(counter_contract_path).unwrap();

    let counter_contract_component =
        AccountComponent::compile(counter_contract_code, assembler.clone(), vec![])
            .unwrap()
            .with_supports_all_types();

    // Getting the hash of the `get_count` procedure
    let get_proc_export = counter_contract_component
        .library()
        .exports()
        .find(|export| export.name.name.as_str() == "get_count")
        .unwrap();

    let get_proc_mast_id = counter_contract_component
        .library()
        .get_export_node_id(&get_proc_export.name);

    let get_count_hash = counter_contract_component
        .library()
        .mast_forest()
        .get_node_by_id(get_proc_mast_id)
        .unwrap()
        .digest()
        .to_hex();

    println!("get count hash: {:?}", get_count_hash);
    println!("counter id prefix: {:?}", counter_contract.id().prefix());
    println!("suffix: {:?}", counter_contract.id().suffix());

    // Build the script that calls the count_copy_contract
    let script_path = Path::new("./masm/scripts/reader_script.masm");
    let script_code_original = fs::read_to_string(script_path).unwrap();
    let script_code = script_code_original
        .replace("{get_count_proc_hash}", &get_count_hash)
        .replace(
            "{account_id_suffix}",
            &counter_contract.id().suffix().to_string(),
        )
        .replace(
            "{account_id_prefix}",
            &counter_contract.id().prefix().to_string(),
        );

    let account_component_lib = create_library(
        assembler.clone(),
        "external_contract::count_reader_contract",
        &count_reader_code,
    )
    .unwrap();

    let tx_script = ScriptBuilder::new(true)
        .with_dynamically_linked_library(&account_component_lib)
        .unwrap()
        .compile_tx_script(script_code)
        .unwrap();

    let foreign_account =
        ForeignAccount::public(counter_contract_id, AccountStorageRequirements::default()).unwrap();

    // Build a transaction request with the custom script
    let tx_request = TransactionRequestBuilder::new()
        .foreign_accounts([foreign_account])
        .custom_script(tx_script)
        .build()
        .unwrap();

    // Execute the transaction locally
    let tx_result = client
        .new_transaction(count_reader_contract.id(), tx_request)
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

    sleep(Duration::from_secs(5)).await;

    client.sync_state().await.unwrap();

    // Retrieve updated contract data to see the incremented counter
    let account_1 = client.get_account(counter_contract.id()).await.unwrap();
    println!(
        "counter contract storage: {:?}",
        account_1.unwrap().account().storage().get_item(0)
    );

    let account_2 = client
        .get_account(count_reader_contract.id())
        .await
        .unwrap();
    println!(
        "count reader contract storage: {:?}",
        account_2.unwrap().account().storage().get_item(0)
    );
#     Ok(())
# }
```

The key here is the use of the `.foreign_accounts()` method on the `TransactionRequestBuilder`. Using this method, it is possible to create transactions with multiple foreign procedure calls.

## Summary

In this tutorial created a smart contract that calls the `get_count` procedure in the counter contract using foreign procedure invocation, and then saves the returned value to its local storage.

The final `src/main.rs` file should look like this:

```rust,no_run
use rand::RngCore;
use std::{fs, path::Path, sync::Arc, time::Duration};
use tokio::time::sleep;

use miden_assembly::{
    ast::{Module, ModuleKind},
    LibraryPath,
};
use miden_client::{
    account::{AccountBuilder, AccountStorageMode, AccountType, StorageSlot},
    account::{AccountIdAddress, Address, AddressInterface},
    builder::ClientBuilder,
    keystore::FilesystemKeyStore,
    rpc::{domain::account::AccountStorageRequirements, Endpoint, TonicRpcClient},
    transaction::{ForeignAccount, TransactionKernel, TransactionRequestBuilder},
    ClientError, ScriptBuilder,
};
use miden_crypto::Word;
use miden_lib::account::auth::NoAuth;
use miden_objects::{
    account::{AccountComponent, NetworkId},
    assembly::{Assembler, DefaultSourceManager},
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
    // STEP 1: Create the Count Reader Contract
    // -------------------------------------------------------------------------
    println!("\n[STEP 1] Creating count reader contract.");

    // Load the MASM file for the counter contract
    let count_reader_path = Path::new("./masm/accounts/count_reader.masm");
    let count_reader_code = fs::read_to_string(count_reader_path).unwrap();

    // Prepare assembler (debug mode = true)
    let assembler: Assembler = TransactionKernel::assembler().with_debug_mode(true);

    // Compile the account code into `AccountComponent` with one storage slot
    let count_reader_component = AccountComponent::compile(
        count_reader_code.clone(),
        assembler.clone(),
        vec![StorageSlot::Value(Word::default())],
    )
    .unwrap()
    .with_supports_all_types();

    // Init seed for the counter contract
    let mut init_seed = [0_u8; 32];
    client.rng().fill_bytes(&mut init_seed);

    // Build the new `Account` with the component
    let (count_reader_contract, count_reader_seed) = AccountBuilder::new(init_seed)
        .account_type(AccountType::RegularAccountImmutableCode)
        .storage_mode(AccountStorageMode::Public)
        .with_component(count_reader_component.clone())
        .with_auth_component(NoAuth)
        .build()
        .unwrap();

    println!(
        "count_reader hash: {:?}",
        count_reader_contract.commitment()
    );
    println!(
        "contract id: {:?}",
        Address::from(AccountIdAddress::new(
            count_reader_contract.id(),
            AddressInterface::Unspecified
        ))
        .to_bech32(NetworkId::Devnet)
    );

    client
        .add_account(
            &count_reader_contract.clone(),
            Some(count_reader_seed),
            false,
        )
        .await
        .unwrap();

    // -------------------------------------------------------------------------
    // STEP 2: Build & Get State of the Counter Contract
    // -------------------------------------------------------------------------
    println!("\n[STEP 2] Building counter contract from public state");

    // Define the Counter Contract account id from counter contract deploy
    let (_, counter_contract_address) =
        Address::from_bech32("mdev1qqarryhdvl3tuqp9k8gcp7r53ecqqeqtky8").unwrap();
    let counter_contract_id = match counter_contract_address {
        Address::AccountId(account_id_address) => account_id_address.id(),
        _ => panic!("Expected AccountId address"),
    };
    println!("counter contract id: {:?}", counter_contract_id.to_hex());

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
    // STEP 3: Call the Counter Contract via Foreign Procedure Invocation (FPI)
    // -------------------------------------------------------------------------
    println!("\n[STEP 3] Call counter contract with FPI from count copy contract");

    let counter_contract_path = Path::new("./masm/accounts/counter.masm");
    let counter_contract_code = fs::read_to_string(counter_contract_path).unwrap();

    let counter_contract_component =
        AccountComponent::compile(counter_contract_code, assembler.clone(), vec![])
            .unwrap()
            .with_supports_all_types();

    // Getting the hash of the `get_count` procedure
    let get_proc_export = counter_contract_component
        .library()
        .exports()
        .find(|export| export.name.name.as_str() == "get_count")
        .unwrap();

    let get_proc_mast_id = counter_contract_component
        .library()
        .get_export_node_id(&get_proc_export.name);

    let get_count_hash = counter_contract_component
        .library()
        .mast_forest()
        .get_node_by_id(get_proc_mast_id)
        .unwrap()
        .digest()
        .to_hex();

    println!("get count hash: {:?}", get_count_hash);
    println!("counter id prefix: {:?}", counter_contract.id().prefix());
    println!("suffix: {:?}", counter_contract.id().suffix());

    // Build the script that calls the count_copy_contract
    let script_path = Path::new("./masm/scripts/reader_script.masm");
    let script_code_original = fs::read_to_string(script_path).unwrap();
    let script_code = script_code_original
        .replace("{get_count_proc_hash}", &get_count_hash)
        .replace(
            "{account_id_suffix}",
            &counter_contract.id().suffix().to_string(),
        )
        .replace(
            "{account_id_prefix}",
            &counter_contract.id().prefix().to_string(),
        );

    let account_component_lib = create_library(
        assembler.clone(),
        "external_contract::count_reader_contract",
        &count_reader_code,
    )
    .unwrap();

    let tx_script = ScriptBuilder::new(true)
        .with_dynamically_linked_library(&account_component_lib)
        .unwrap()
        .compile_tx_script(script_code)
        .unwrap();

    let foreign_account =
        ForeignAccount::public(counter_contract_id, AccountStorageRequirements::default()).unwrap();

    // Build a transaction request with the custom script
    let tx_request = TransactionRequestBuilder::new()
        .foreign_accounts([foreign_account])
        .custom_script(tx_script)
        .build()
        .unwrap();

    // Execute the transaction locally
    let tx_result = client
        .new_transaction(count_reader_contract.id(), tx_request)
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

    sleep(Duration::from_secs(5)).await;

    client.sync_state().await.unwrap();

    // Retrieve updated contract data to see the incremented counter
    let account_1 = client.get_account(counter_contract.id()).await.unwrap();
    println!(
        "counter contract storage: {:?}",
        account_1.unwrap().account().storage().get_item(0)
    );

    let account_2 = client
        .get_account(count_reader_contract.id())
        .await
        .unwrap();
    println!(
        "count reader contract storage: {:?}",
        account_2.unwrap().account().storage().get_item(0)
    );

    Ok(())
}
```

The output of our program will look something like this:

```text
Latest block: 17916

[STEP 1] Creating count reader contract.
count_reader hash: RpoDigest([17106452548071357259, 1177663122773866223, 12129142941281960455, 8269441041947541276])
contract id: "0x4e79c8d2334239000000197081e311"

[STEP 2] Building counter contract from public state
Account details: Value([0, 0, 0, 2])

[STEP 3] Call Counter Contract with FPI from Count Copy Contract
get count hash: "0x92495ca54d519eb5e4ba22350f837904d3895e48d74d8079450f19574bb84cb6"
counter id prefix: V0(AccountIdPrefixV0 { prefix: 1170938688336660224 })
suffix: 204650730194688
Stack state before step 2248:
├──  0: 111
├──  1: 1170938688336660224
├──  2: 204650730194688
├──  3: 13136076846856212293
├──  4: 8755083262635706835
├──  5: 322432949672917732
├──  6: 13086986961113860498
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
├── 19: 0
├── 20: 0
├── 21: 0
└── 22: 0

Stack state before step 3224:
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
├── 19: 0
├── 20: 0
└── 21: 0

View transaction on MidenScan: https://testnet.midenscan.com/tx/0x7144cf2648a7001a9972aed73596db070a679b467fec83263846a5a4f8eb74e6
counter contract storage: Ok(RpoDigest([0, 0, 0, 2]))
count reader contract storage: Ok(RpoDigest([0, 0, 0, 2]))
```

### Running the example

To run the full example, navigate to the `rust-client` directory in the [miden-tutorials](https://github.com/0xMiden/miden-tutorials/) repository and run this command:

```bash
cd rust-client
cargo run --release --bin counter_contract_fpi
```

### Continue learning

Next tutorial: [How to Create Notes with Custom Logic](custom_note_how_to.md)
