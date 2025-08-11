# Network Transactions on Miden

_Using the Miden client in Rust to deploy and interact with smart contracts using network transactions_

## Overview

In this tutorial, we will explore Network Transactions (NTXs) on Miden - a powerful feature that enables autonomous smart contract execution and public shared state management. Unlike local transactions that require users to execute and prove, network transactions are executed and proven by the Miden operator.

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
miden-client = { version = "0.10.0", features = ["testing", "tonic", "sqlite"] }
miden-lib = { version = "0.10.0", default-features = false }
miden-objects = { version = "0.10.0", default-features = false }
miden-crypto = { version = "0.15.0", features = ["executable"] }
miden-assembly = "0.15.0"
rand = { version = "0.9" }
serde = { version = "1", features = ["derive"] }
serde_json = { version = "1.0", features = ["raw_value"] }
tokio = { version = "1.40", features = ["rt-multi-thread", "net", "macros"] }
rand_chacha = "0.9.0"
dotenv = "0.15"
```

## Step 2: Set up MASM files

Create the directory structure:

```bash
mkdir -p masm/accounts masm/scripts masm/notes masm/accounts/auth
```

### Counter Contract

We'll use the same counter contract MASM code as the regular counter tutorial. The key difference is in the Rust configuration, not the MASM code.

Create `masm/accounts/counter.masm`:

```masm
use.miden::account
use.std::sys

# => []
export.get_count
    push.0
    # => [index]

    exec.account::get_item
    # => [count]

    exec.sys::truncate_stack
    # => [count]
end

# => []
export.increment
    push.0
    # => [index]

    exec.account::get_item
    # => [count]

    push.1 add
    # => [count+1]

    # debug statement with client
    push.111 debug.stack drop

    push.0
    # [index, count+1]

    exec.account::set_item
    # => []

    exec.sys::truncate_stack
    # => []
end
```

### Transaction Script for Deployment

Create `masm/scripts/network_increment_script.masm`:

```masm
use.external_contract::network_counter_contract

begin
    call.network_counter_contract::increment
end
```

This script will be used to deploy the network account and ensure it's registered on-chain.

### Network Note for User Interaction

Create `masm/notes/network_increment_note.masm`:

```masm
use.external_contract::network_counter_contract

begin
    call.network_counter_contract::increment
end
```

After deployment, users will interact with the contract through these network notes.

### Authentication Component

Create `masm/accounts/auth/no_auth.masm`:

```masm
use.miden::account

export.auth__basic
    push.1 exec.account::incr_nonce
end
```

## Step 3: Implement the Network Transaction Example

Now let's walk through each step of the network transaction implementation:

### Step 1: Initialize Client and Create User Account

```rust,no_run
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    delete_keystore_and_store(None).await;

    let endpoint = Endpoint::testnet();
    let mut client = instantiate_client(endpoint.clone(), None).await.unwrap();
    let keystore = FilesystemKeyStore::new("./keystore".into()).unwrap();

    let sync_summary = client.sync_state().await.unwrap();
    println!("Latest block: {}", sync_summary.block_num);

    // Create a basic user account (Alice)
    let (alice_account, _) = create_basic_account(&mut client, keystore.clone())
        .await
        .unwrap();
    println!(
        "alice account id: {:?}",
        alice_account.id().to_bech32(NetworkId::Testnet)
    );
```

This step initializes the Miden client and creates a basic user account that will interact with our network contract.

### Step 2: Create Network Counter Smart Contract

```rust,no_run
    // Load the same counter MASM code
    let counter_code = fs::read_to_string(Path::new("../masm/accounts/counter.masm")).unwrap();

    // Create network account - key difference is storage mode
    let (counter_contract, counter_seed) = create_network_account(&mut client, &counter_code)
        .await
        .unwrap();
    println!(
        "contract id: {:?}",
        counter_contract.id().to_bech32(NetworkId::Testnet)
    );

    // Save contract ID for later use
    let env_content = format!("NETWORK_COUNTER_CONTRACT_ID={}", counter_contract.id().to_hex());
    fs::write(".env", env_content).expect("Failed to write .env file");
    println!("Network counter contract ID saved to .env file");

    client
        .add_account(&counter_contract, Some(counter_seed), false)
        .await
        .unwrap();
```

The key difference from a regular counter is in the `create_network_account` function:

```rust,no_run
async fn create_network_account(
    client: &mut Client,
    account_code: &str,
) -> Result<(Account, Word), ClientError> {
    // ... component compilation ...
    
    let (counter_contract, counter_seed) = AccountBuilder::new(init_seed)
        .account_type(AccountType::RegularAccountImmutableCode)
        .storage_mode(AccountStorageMode::Network)  // Network storage!
        .with_auth_component(no_auth_component)
        .with_component(counter_component)
        .build()
        .unwrap();

    Ok((counter_contract, counter_seed))
}
```

**Key Point**: We use `AccountStorageMode::Network` instead of `AccountStorageMode::Public`. This enables the contract to be executed by the network operator.

### Step 3: Deploy Network Account with Transaction Script

```rust,no_run
    // Load the increment script
    let script_code =
        fs::read_to_string(Path::new("../masm/scripts/network_increment_script.masm")).unwrap();

    let account_code = fs::read_to_string(Path::new("../masm/accounts/network_counter.masm")).unwrap();
    let library_path = "external_contract::network_counter_contract";

    // Create library and transaction script
    let library = create_library(account_code, library_path).unwrap();
    
    // Use ScriptBuilder to compile transaction script with library
    let tx_script = ScriptBuilder::default()
        .with_dynamically_linked_library(&library)?
        .compile_tx_script(script_code)?;

    // Execute transaction script to deploy and initialize the contract
    let tx_increment_request = TransactionRequestBuilder::new()
        .custom_script(tx_script)
        .build()
        .unwrap();

    let tx_result = client
        .new_transaction(counter_contract.id(), tx_increment_request)
        .await
        .unwrap();

    let _ = client.submit_transaction(tx_result.clone()).await;

    let tx_id = tx_result.executed_transaction().id();
    println!(
        "View transaction on MidenScan: https://testnet.midenscan.com/tx/{:?}",
        tx_id
    );

    // Wait for the transaction to be committed
    wait_for_tx(&mut client, tx_id).await.unwrap();
```

This step uses a transaction script to deploy the network account and ensure it's properly registered on-chain. The script calls the `increment` function, which initializes the counter to 1.

### Step 4: Create Network Note for User Interaction

```rust,no_run
    // Load the increment note code
    let note_code = fs::read_to_string(Path::new("../masm/notes/network_increment_note.masm")).unwrap();
    let account_code = fs::read_to_string(Path::new("../masm/accounts/network_counter.masm")).unwrap();

    let library_path = "external_contract::network_counter_contract";
    let library = create_library(account_code, library_path).unwrap();

    // Create network note
    let (_increment_note, note_tx_id) = create_network_note(
        &mut client,
        note_code,
        library,
        alice_account.clone(),
        counter_contract.id(),
    )
    .await
    .unwrap();

    println!("increment note created, waiting for onchain commitment");

    // Wait for the note transaction to be committed
    wait_for_tx(&mut client, note_tx_id).await.unwrap();
```

The `create_network_note` function creates a public note that can be consumed by the network operator:

```rust,no_run
async fn create_network_note(
    client: &mut Client,
    note_code: String,
    account_library: Library,
    creator_account: Account,
    counter_contract_id: AccountId,
) -> Result<(Note, TransactionId), Box<dyn std::error::Error>> {
    let rng = client.rng();
    let serial_num = rng.inner_mut().draw_word();

    // Use ScriptBuilder to compile note script with library
    let note_script = ScriptBuilder::default()
        .with_dynamically_linked_library(&account_library)?
        .compile_note_script(note_code)?;
    let note_inputs = NoteInputs::new([].to_vec())?;
    let recipient = NoteRecipient::new(serial_num, note_script, note_inputs.clone());

    let tag = NoteTag::from_account_id(counter_contract_id);
    let metadata = NoteMetadata::new(
        creator_account.id(),
        NoteType::Public,  // Public note for network execution
        tag,
        NoteExecutionHint::none(),
        Felt::new(0),
    )?;

    let note = Note::new(NoteAssets::default(), metadata, recipient);
    
    // Submit note transaction
    let note_req = TransactionRequestBuilder::new()
        .own_output_notes(vec![OutputNote::Full(note.clone())])
        .build()?;
    let tx_result = client
        .new_transaction(creator_account.id(), note_req)
        .await?;

    let _ = client.submit_transaction(tx_result.clone()).await;
    // ...
}
```

This creates a public note that the network operator can consume to execute the increment function.

### Step 5: Validate Updated State

```rust,no_run
    // Clean up and create fresh client to validate results
    delete_keystore_and_store(None).await;
    let mut client = instantiate_client(endpoint, None).await.unwrap();

    // Import the contract and check final state
    client
        .import_account_by_id(counter_contract.id())
        .await
        .unwrap();

    let new_account_state = client.get_account(counter_contract.id()).await.unwrap();

    if let Some(account) = new_account_state.as_ref() {
        let count: Word = account.account().storage().get_item(0).unwrap().into();
        let val = count.get(3).unwrap().as_int();
        assert_eq!(val, 2);  // 1 from script + 1 from note = 2
        println!("🔢 Final counter value: {}", val);
    }

    Ok(())
}
```

This final step validates that both the transaction script (which incremented to 1) and the network note (which incremented to 2) were successfully executed by the network operator.

## Step 4: Running the Example

To run the complete network transaction example:

```bash
cd rust-client
cargo run --release --bin network_notes_counter_contract
```

Expected output:
```text
Latest block: 226717
alice account id: "mtst1qql030hpsp0yyqra494lcwazxsym7add"
contract id: "mtst1qpj0g3ke67tg5qqqqd2z4ffm9g8ezpf6"
Network counter contract ID saved to .env file
View transaction on MidenScan: https://testnet.midenscan.com/tx/0x...
✅ transaction committed
increment note created, waiting for onchain commitment
✅ transaction committed
🔢 Final counter value: 2
```

## Summary

Network transactions on Miden enable powerful use cases by allowing the operator to execute transactions on behalf of users. The key steps are:

1. **Create network account**: Use `AccountStorageMode::Network` instead of `Public`
2. **Deploy with transaction script**: Ensures the contract is registered on-chain
3. **Interact with network notes**: Users create public notes that the operator executes
4. **Autonomous execution**: The operator handles proof generation and execution

The same MASM code works for both regular and network contracts - the difference is purely in the Rust configuration. This makes network transactions a powerful tool for building applications like AMMs where multiple users need to interact with shared state efficiently.

### Continue learning

Next tutorial: [Foreign Procedure Invocation](foreign_procedure_invocation_tutorial.md)