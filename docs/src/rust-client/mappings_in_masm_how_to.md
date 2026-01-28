---
title: "How to Use Mappings in Miden Assembly"
sidebar_position: 10
---

# How to Use Mappings in Miden Assembly

_Using mappings in Miden assembly for storing key value pairs_

## Overview

In this example, we will explore how to use mappings in Miden Assembly. Mappings are essential data structures that store key-value pairs. We will demonstrate how to create an account that contains a mapping and then call a procedure in that account to update the mapping.

At a high level, this example involves:

- Setting up an account with a mapping stored in one of its storage slots.
- Writing a smart contract in Miden Assembly that includes procedures to read from and write to the mapping.
- Creating a transaction script that calls these procedures.
- Using Rust code to deploy the account and submit a transaction that updates the mapping.  
  After the Miden Assembly snippets, we explain that the transaction script calls a procedure in the account. This procedure then updates the mapping by modifying the mapping stored in the account's storage slot.

## What we'll cover

- **How to Use Mappings in Miden Assembly:** See how to create a smart contract that uses a mapping.
- **How to Link Libraries in Miden Assembly:** Demonstrate how to link procedures across Accounts, Notes, and Scripts.

## Step-by-step process

1. **Setting up an account with a mapping**  
   In this step, you create an account that has a storage slot configured as a mapping. The account smart contract code (shown below) defines procedures to write to and read from this mapping.

2. **Creating a script that calls a procedure in the account:**  
   Next, you create a transaction script that calls the procedures defined in the account. This script sends the key-value data and then invokes the account procedure, which updates the mapping.

3. **How to read and write to a mapping in MASM:**  
   Finally, we demonstrate how to use MASM instructions to interact with the mapping. The smart contract uses standard procedures to set a mapping item, retrieve a value from the mapping, and get the current mapping root.

---

### Example of smart contract that uses a mapping

```masm
use miden::protocol::active_account
use miden::protocol::native_account
use miden::core::word
use miden::core::sys

const MAP_SLOT = word("miden::tutorials::mapping::map")

# Inputs: [KEY, VALUE]
# Outputs: []
pub proc write_to_map
    # The storage map is in the mapping slot.
    push.MAP_SLOT[0..2]
    # => [slot_id_prefix, slot_id_suffix, KEY, VALUE]

    # Setting the key value pair in the map
    exec.native_account::set_map_item
    # => [OLD_VALUE]

    dropw
    # => []
end

# Inputs: [KEY]
# Outputs: [VALUE]
pub proc get_value_in_map
    # The storage map is in the mapping slot.
    push.MAP_SLOT[0..2]
    # => [slot_id_prefix, slot_id_suffix, KEY]

    exec.active_account::get_map_item
    # => [VALUE]
end

# Inputs: []
# Outputs: [CURRENT_ROOT]
pub proc get_current_map_root
    # Getting the current root from the mapping slot.
    push.MAP_SLOT[0..2] exec.active_account::get_item
    # => [CURRENT_ROOT]

    exec.sys::truncate_stack
    # => [CURRENT_ROOT]
end
```

### Explanation of the assembly code

- **write_to_map:**  
  The procedure takes a key and a value as inputs. It pushes the slot ID prefix and suffix for the mapping slot onto the stack, then calls the `set_map_item` procedure from the account library to update the mapping. After updating the map, it drops the old value.
- **get_value_in_map:**  
  This procedure takes a key as input and retrieves the corresponding value from the mapping by calling `get_map_item` after pushing the mapping slot ID.

- **get_current_map_root:**  
  This procedure retrieves the current root of the mapping by calling `get_item` with the mapping slot ID and then truncating the stack to leave only the mapping root.

**Security Note**: The procedure `write_to_map` calls the account procedure `incr_nonce`. This allows any external account to be able to write to the storage map of the account. Smart contract developers should know that procedures that call the `account::incr_nonce` procedure allow anyone to call the procedure and modify the state of the account.

### Transaction script that calls the smart contract

```masm
use miden_by_example::mapping_example_contract
use miden::core::sys

begin
    push.1.2.3.4
    push.0.0.0.0
    # => [KEY, VALUE]

    call.mapping_example_contract::write_to_map
    # => []

    push.0.0.0.0
    # => [KEY]

    call.mapping_example_contract::get_value_in_map
    # => [VALUE]

    dropw
    # => []

    call.mapping_example_contract::get_current_map_root
    # => [CURRENT_ROOT]

    exec.sys::truncate_stack
end
```

### Explanation of the transaction script

The transaction script does the following:

- It pushes a key (`[0.0.0.0]`) and a value (`[1.2.3.4]`) onto the stack.
- It calls the `write_to_map` procedure, which is defined in the account’s smart contract. This updates the mapping in the account.
- It then pushes the key again and calls `get_value_in_map` to retrieve the value associated with the key.
- Finally, it calls `get_current_map_root` to get the current state (root) of the mapping.

The script calls the `write_to_map` procedure in the account which writes the key value pair to the mapping.

---

### Rust code that sets everything up

Below is the Rust code that deploys the smart contract, creates the transaction script, and submits a transaction to update the mapping in the account:

```rust no_run
use miden_client::auth::NoAuth;
use miden_client::transaction::TransactionKernel;
use rand::RngCore;
use std::{fs, path::Path, sync::Arc};

use miden_client::{
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
        AccountBuilder, AccountComponent, AccountStorageMode, AccountType, StorageMap, StorageSlot,
        StorageSlotName,
    },
    Felt, Word,
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
    // STEP 1: Deploy a smart contract with a mapping
    // -------------------------------------------------------------------------
    println!("\n[STEP 1] Deploy a smart contract with a mapping");

    // Load the MASM file for the counter contract
    let file_path = Path::new("../masm/accounts/mapping_example_contract.masm");
    let account_code = fs::read_to_string(file_path).unwrap();

    // Prepare assembler (debug mode = true)
    let assembler: Assembler = TransactionKernel::assembler();

    // Using an empty storage value in slot 0 since this is usually reserved
    // for the account pub_key and metadata
    let empty_slot_name =
        StorageSlotName::new("miden::tutorials::mapping::value").expect("valid slot name");
    let empty_storage_slot = StorageSlot::with_value(empty_slot_name.clone(), Word::default());

    // initialize storage map
    let storage_map = StorageMap::new();
    let map_slot_name =
        StorageSlotName::new("miden::tutorials::mapping::map").expect("valid slot name");
    let storage_slot_map = StorageSlot::with_map(map_slot_name.clone(), storage_map.clone());

    // Compile the account code into `AccountComponent` with one storage slot
    let component_code = CodeBuilder::new()
        .compile_component_code("miden_by_example::mapping_example_contract", &account_code)
        .unwrap();
    let mapping_contract_component =
        AccountComponent::new(component_code, vec![empty_storage_slot, storage_slot_map])
            .unwrap()
            .with_supports_all_types();

    // Init seed for the counter contract
    let mut init_seed = [0_u8; 32];
    client.rng().fill_bytes(&mut init_seed);

    // Build the new `Account` with the component
    let mapping_example_contract = AccountBuilder::new(init_seed)
        .account_type(AccountType::RegularAccountImmutableCode)
        .storage_mode(AccountStorageMode::Public)
        .with_component(mapping_contract_component.clone())
        .with_auth_component(NoAuth)
        .build()
        .unwrap();

    client
        .add_account(&mapping_example_contract, false)
        .await
        .unwrap();

    // -------------------------------------------------------------------------
    // STEP 2: Call the Mapping Contract with a Script
    // -------------------------------------------------------------------------
    println!("\n[STEP 2] Call Mapping Contract With Script");

    let script_code =
        fs::read_to_string(Path::new("../masm/scripts/mapping_example_script.masm")).unwrap();

    // Create the library from the account source code using the helper function.
    let account_component_lib = create_library(
        assembler.clone(),
        "miden_by_example::mapping_example_contract",
        &account_code,
    )
    .unwrap();

    // Compile the transaction script with the library.
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
        .submit_new_transaction(mapping_example_contract.id(), tx_increment_request)
        .await
        .unwrap();

    println!(
        "View transaction on MidenScan: https://testnet.midenscan.com/tx/{:?}",
        tx_id
    );

    client.sync_state().await.unwrap();

    let account_record = client
        .get_account(mapping_example_contract.id())
        .await
        .unwrap()
        .expect("mapping contract not found");
    let account = match account_record.account_data() {
        AccountRecordData::Full(account) => account,
        AccountRecordData::Partial(_) => panic!("mapping contract is missing full account data"),
    };
    let key = [Felt::new(0), Felt::new(0), Felt::new(0), Felt::new(0)].into();
    println!(
        "Mapping state\n Slot: {:?}\n Key: {:?}\n Value: {:?}",
        map_slot_name,
        key,
        account.storage().get_map_item(&map_slot_name, key)
    );

    Ok(())
}
```

### What the Rust code does

- **Client Initialization:**  
  The client is initialized with a connection to the Miden Testnet and a SQLite store. This sets up the environment to deploy and interact with accounts.

- **Deploying the Smart Contract:**  
  The account containing the mapping is created by reading the MASM smart contract from a file, compiling it into an `AccountComponent`, and deploying it using an `AccountBuilder`.

- **Creating and Executing a Transaction Script:**  
  A separate MASM script is compiled into a `TransactionScript`. This script calls the smart contract's procedures to write to and then read from the mapping.

- **Displaying the Result:**  
  Finally, after the transaction is processed, the code reads the updated state of the mapping in the account.

---

### Running the example

To run the full example, navigate to the `rust-client` directory in the [miden-tutorials](https://github.com/0xMiden/miden-tutorials/) repository and run this command:

```bash
cd rust-client
cargo run --release --bin mapping_example
```

This example shows how the script calls the procedure in the account, which then updates the mapping stored within the account. The mapping update is verified by reading the mapping’s key-value pair after the transaction completes.

### Continue learning

Next tutorial: [How to Create Notes in Miden Assembly](creating_notes_in_masm_tutorial.md)
