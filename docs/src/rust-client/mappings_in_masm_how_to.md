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
use.miden::account
use.std::sys

# Inputs: [KEY, VALUE]
# Outputs: []
export.write_to_map
  # The storage map is in storage slot 1
  push.1
  # => [index, KEY, VALUE]

  # Setting the key value pair in the map
  exec.account::set_map_item
  # => [OLD_MAP_ROOT, OLD_MAP_VALUE]

  dropw dropw dropw dropw
  # => []

  # Incrementing the nonce by 1
  push.1 exec.account::incr_nonce
  # => []
end

# Inputs: [KEY]
# Outputs: [VALUE]
export.get_value_in_map
  # The storage map is in storage slot 1
  push.1
  # => [index]

  exec.account::get_map_item
  # => [VALUE]
end

# Inputs: []
# Outputs: [CURRENT_ROOT]
export.get_current_map_root
  # Getting the current root from slot 1
  push.1 exec.account::get_item
  # => [CURRENT_ROOT]

  exec.sys::truncate_stack
  # => [CURRENT_ROOT]
end
```

### Explanation of the assembly code

- **write_to_map:**  
  The procedure takes a key and a value as inputs. It pushes the storage index (`0` for our mapping) onto the stack, then calls the `set_map_item` procedure from the account library to update the mapping. After updating the map, it drops any unused outputs and increments the nonce.
- **get_value_in_map:**  
  This procedure takes a key as input and retrieves the corresponding value from the mapping by calling `get_map_item` after pushing the mapping index.

- **get_current_map_root:**  
  This procedure retrieves the current root of the mapping (stored at index `0`) by calling `get_item` and then truncating the stack to leave only the mapping root.

**Security Note**: The procedure `write_to_map` calls the account procedure `incr_nonce`. This allows any external account to be able to write to the storage map of the account. Smart contract developers should know that procedures that call the `account::incr_nonce` procedure allow anyone to call the procedure and modify the state of the account.

### Transaction script that calls the smart contract

```masm
use.miden_by_example::mapping_example_contract
use.std::sys

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

```rust
use rand::RngCore;
use std::{fs, path::Path, sync::Arc};

use miden_assembly::{
    ast::{Module, ModuleKind},
    LibraryPath,
};
use miden_client::{
    account::{AccountBuilder, AccountStorageMode, AccountType, StorageSlot},
    builder::ClientBuilder,
    rpc::{Endpoint, TonicRpcClient},
    transaction::{TransactionKernel, TransactionRequestBuilder, TransactionScript},
    ClientError, Felt,
};
use miden_objects::{
    account::{AccountComponent, StorageMap},
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

    let mut client = ClientBuilder::new()
        .with_rpc(rpc_api)
        .with_filesystem_keystore("./keystore")
        .in_debug_mode(true)
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
    let assembler: Assembler = TransactionKernel::assembler().with_debug_mode(true);

    // Using an empty storage value in slot 0 since this is usually resurved
    // for the account pub_key and metadata
    let empty_storage_slot = StorageSlot::empty_value();

    // initialize storage map
    let storage_map = StorageMap::new();
    let storage_slot_map = StorageSlot::Map(storage_map.clone());

    // Compile the account code into `AccountComponent` with one storage slot
    let mapping_contract_component = AccountComponent::compile(
        account_code.clone(),
        assembler.clone(),
        vec![empty_storage_slot, storage_slot_map],
    )
    .unwrap()
    .with_supports_all_types();

    // Init seed for the counter contract
    let mut init_seed = [0_u8; 32];
    client.rng().fill_bytes(&mut init_seed);

    // Anchor block of the account
    let anchor_block = client.get_latest_epoch_block().await.unwrap();

    // Build the new `Account` with the component
    let (mapping_example_contract, seed) = AccountBuilder::new(init_seed)
        .anchor((&anchor_block).try_into().unwrap())
        .account_type(AccountType::RegularAccountImmutableCode)
        .storage_mode(AccountStorageMode::Public)
        .with_component(mapping_contract_component.clone())
        .build()
        .unwrap();

    client
        .add_account(&mapping_example_contract.clone(), Some(seed), false)
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
    let tx_script = TransactionScript::compile(
        script_code,
        [],
        assembler.with_library(&account_component_lib).unwrap(),
    )
    .unwrap();

    // Build a transaction request with the custom script
    let tx_increment_request = TransactionRequestBuilder::new()
        .with_custom_script(tx_script)
        .build()
        .unwrap();

    // Execute the transaction locally
    let tx_result = client
        .new_transaction(mapping_example_contract.id(), tx_increment_request)
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

    let account = client
        .get_account(mapping_example_contract.id())
        .await
        .unwrap();
    let index = 1;
    let key = [Felt::new(0), Felt::new(0), Felt::new(0), Felt::new(0)];
    println!(
        "Mapping state\n Index: {:?}\n Key: {:?}\n Value: {:?}",
        index,
        key,
        account
            .unwrap()
            .account()
            .storage()
            .get_map_item(index, key)
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

To run the full example, navigate to the `rust-client` directory in the [miden-tutorials](https://github.com/0xPolygonMiden/miden-tutorials/) repository and run this command:

```bash
cd rust-client
cargo run --release --bin mapping_example
```

This example shows how the script calls the procedure in the account, which then updates the mapping stored within the account. The mapping update is verified by reading the mapping’s key-value pair after the transaction completes.
