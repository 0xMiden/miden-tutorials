# Creating Notes in Miden Assembly

_Creating notes inside the MidenVM using Miden assembly_

## Overview

In this tutorial, we will create a custom note that generates a copy of itself when it is consumed by an account. The purpose of this tutorial is to demonstrate how to create notes inside the MidenVM using Miden assembly (MASM). By the end of this tutorial, you will understand how to write MASM code that creates notes.

## What We'll Cover

- Computing the note inputs commitment in MASM
- Creating notes in MASM

## Prerequisites

This tutorial assumes you have a basic understanding of Miden assembly and that you have completed the tutorial on [creating a custom note](./custom_note_how_to.md).

## Why Creating Notes in MASM Is Useful

Being able to create a note in MASM enables you to build various types of applications. Creating a note during the consumption of another note or from an account allows you to develop complex DeFi applications.

Here are some tangible examples of when creating a note in MASM is useful in a DeFi context:

- Creating snapshots of an account's state at a specific point in time (not possible in an EVM context)
- Representing partially fillable buy/sell orders as notes (SWAPP)
- Handling withdrawals from a smart contract

## What We Will Be Building

![Iterative Note Creation](../assets/note_creation_masm.png)

In the diagram above, note A is consumed by an account, and during the transaction, note A' is created.

In this tutorial, we will create a note that contains an asset. When consumed, it outputs a copy of itself and allows the consuming account to take half of the asset. Although this type of note would not be used in a real-world context, it demonstrates several key concepts for writing MASM code that can create notes.

## Step 1: Initialize Your Repository

Create a new Rust repository for your Miden project and navigate to it with the following command:

```bash
cargo new miden-project
cd miden-project
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

## Step 2: Write the Note Script

For better code organization, we will separate the Miden assembly code from our Rust code.

Create a directory named `masm` at the **root** of your `miden-project` directory. This directory will contain our contract and MASM script code.

Initialize the `masm` directory:

```bash
mkdir masm/notes
```

This will create:

```
masm/
└── notes/
```

Inside the `masm/notes/` directory, create the file `iterative_output_note.masm`:

```masm
use.miden::note
use.miden::tx
use.std::sys
use.std::crypto::hashes::rpo
use.miden::contracts::wallets::basic->wallet

# Memory Addresses
const.ASSET=0
const.ASSET_HALF=4
const.ACCOUNT_ID_PREFIX=8
const.ACCOUNT_ID_SUFFIX=9
const.TAG=10

# => []
begin
    # Drop word if user accidentally pushes note_args
    dropw
    # => []

    # Get note inputs
    push.ACCOUNT_ID_PREFIX exec.note::get_inputs drop drop
    # => []

    # Get asset contained in note
    push.ASSET exec.note::get_assets drop drop
    # => []

    mem_loadw.ASSET
    # => [ASSET]

    # Compute half amount of asset
    swap.3 push.2 div swap.3
    # => [ASSET_HALF]

    mem_storew.ASSET_HALF dropw
    # => []

    mem_loadw.ASSET
    # => [ASSET]

    # Receive the entire asset amount to the wallet
    call.wallet::receive_asset
    # => []

    # Get note inputs commitment
    push.8.ACCOUNT_ID_PREFIX
    # => [memory_address_pointer, number_of_inputs]

    # Note: Must pad with 0s to nearest multiple of 8
    exec.rpo::hash_memory
    # => [INPUTS_COMMITMENT]

    # Push script hash
    exec.note::get_script_root
    # => [SCRIPT_HASH, INPUTS_COMMITMENT]

    # Get the current note serial number
    exec.note::get_serial_number
    # => [SERIAL_NUM, SCRIPT_HASH, INPUTS_COMMITMENT]

    # Increment serial number by 1
    push.1 add
    # => [SERIAL_NUM+1, SCRIPT_HASH, INPUTS_COMMITMENT]

    exec.tx::build_recipient_hash
    # => [RECIPIENT]

    # Push hint, note type, and aux to stack
    push.1.1.0
    # => [aux, public_note, execution_hint_always, RECIPIENT]

    # Load tag from memory
    mem_load.TAG
    # => [tag, aux, note_type, execution_hint, RECIPIENT]

    call.wallet::create_note
    # => [note_idx, pad(15) ...]

    padw mem_loadw.ASSET_HALF
    # => [ASSET / 2, note_idx]

    call.wallet::move_asset_to_note
    # => [ASSET, note_idx, pad(11)]

    dropw drop
    # => []

    exec.sys::truncate_stack
    # => []
end
```

### How the Assembly Code Works:

1. **Reads note inputs:**  
   The note begins by writing the note inputs to memory by calling the `note::get_inputs` procedure. It writes the note inputs starting at memory address 8, which is defined as the constant `ACCOUNT_ID_PREFIX`.
2. **Retrieving the asset:**  
   The note then calls `note::get_assets` to write the asset contained in the note to memory address 0, defined as `ASSET`. It computes half of the asset and stores the value at memory address 4, defined as `ASSET_HALF`. Finally, the note calls the `wallet::receive_asset` procedure to move the asset contained in the note to the consuming account.
3. **Computing note inputs hash in MASM:**  
   The script calls the `note::compute_inputs_hash` procedure with the number of inputs and the memory address where the inputs begin. This procedure returns the note inputs commitment.
4. **Getting the script hash:**  
   Next, the note script calls the `note::get_script_hash` procedure, which returns the note's script hash.
5. **Getting the serial number for the future note:**  
   Although not strictly necessary in this scenario, preventing two identical notes from having the same serial number is important. If an account creates two identical notes with the same serial number, recipient, and asset vault, one of the notes may not be consumed. Therefore, the MASM code increments the serial number of the current note by 1.
6. **Computing the `RECIPIENT` hash:**  
   The `RECIPIENT` hash is defined as:  
   `hash(hash(hash(serial_num, [0; 4]), script_root), input_commitment)`  
   To compute it in MASM, the script calls the `tx::build_recipient_hash` procedure with the serial number, script hash, and inputs commitment on the stack.
7. **Creating the note:**  
   To create the note, the script pushes the execution hint, note type, aux value, and tag onto the stack, then calls the `wallet::create_note` procedure, which returns a pointer to the note.
8. **Moving assets to the note:**  
   After the note is created, the script loads the half asset value computed in step 2 onto the stack and calls the `wallet::move_asset_to_note` procedure.
9. **Stack cleanup:**  
   Finally, the script cleans up the stack by calling `sys::truncate_stack` after creating the note and adding the assets.

## Step 3: Rust Program

With the Miden assembly note script written, we can move on to writing the Rust script to create and consume the note.

Copy and paste the following code into your `src/main.rs` file.

```rust
use rand::{prelude::StdRng, RngCore};
use std::{fs, path::Path, sync::Arc};
use tokio::time::{sleep, Duration};

use miden_client::{
    account::{
        component::{BasicFungibleFaucet, BasicWallet, RpoFalcon512},
        AccountBuilder, AccountStorageMode, AccountType,
    },
    asset::{FungibleAsset, TokenSymbol},
    auth::AuthSecretKey,
    builder::ClientBuilder,
    crypto::{FeltRng, SecretKey},
    keystore::FilesystemKeyStore,
    note::{
        Note, NoteAssets, NoteExecutionHint, NoteExecutionMode, NoteInputs, NoteMetadata,
        NoteRecipient, NoteScript, NoteTag, NoteType,
    },
    rpc::{Endpoint, TonicRpcClient},
    transaction::{OutputNote, TransactionKernel, TransactionRequestBuilder},
    Client, ClientError, Felt,
};

// Helper to create a basic account
async fn create_basic_account(
    client: &mut Client,
    keystore: FilesystemKeyStore<StdRng>,
) -> Result<miden_client::account::Account, ClientError> {
    let mut init_seed = [0u8; 32];
    client.rng().fill_bytes(&mut init_seed);
    let key_pair = SecretKey::with_rng(client.rng());
    let anchor_block = client.get_latest_epoch_block().await.unwrap();
    let builder = AccountBuilder::new(init_seed)
        .anchor((&anchor_block).try_into().unwrap())
        .account_type(AccountType::RegularAccountUpdatableCode)
        .storage_mode(AccountStorageMode::Public)
        .with_component(RpoFalcon512::new(key_pair.public_key()))
        .with_component(BasicWallet);
    let (account, seed) = builder.build().unwrap();
    client.add_account(&account, Some(seed), false).await?;
    keystore
        .add_key(&AuthSecretKey::RpoFalcon512(key_pair))
        .unwrap();
    Ok(account)
}

async fn create_basic_faucet(
    client: &mut Client,
    keystore: FilesystemKeyStore<StdRng>,
) -> Result<miden_client::account::Account, ClientError> {
    let mut init_seed = [0u8; 32];
    client.rng().fill_bytes(&mut init_seed);
    let key_pair = SecretKey::with_rng(client.rng());
    let anchor_block = client.get_latest_epoch_block().await.unwrap();
    let symbol = TokenSymbol::new("MID").unwrap();
    let decimals = 8;
    let max_supply = Felt::new(1_000_000);
    let builder = AccountBuilder::new(init_seed)
        .anchor((&anchor_block).try_into().unwrap())
        .account_type(AccountType::FungibleFaucet)
        .storage_mode(AccountStorageMode::Public)
        .with_component(RpoFalcon512::new(key_pair.public_key()))
        .with_component(BasicFungibleFaucet::new(symbol, decimals, max_supply).unwrap());
    let (account, seed) = builder.build().unwrap();
    client.add_account(&account, Some(seed), false).await?;
    keystore
        .add_key(&AuthSecretKey::RpoFalcon512(key_pair))
        .unwrap();
    Ok(account)
}

// Helper to wait until an account has the expected number of consumable notes
async fn wait_for_notes(
    client: &mut Client,
    account_id: &miden_client::account::Account,
    expected: usize,
) -> Result<(), ClientError> {
    loop {
        client.sync_state().await?;
        let notes = client.get_consumable_notes(Some(account_id.id())).await?;
        if notes.len() >= expected {
            break;
        }
        println!(
            "{} consumable notes found for account {}. Waiting...",
            notes.len(),
            account_id.id().to_hex()
        );
        sleep(Duration::from_secs(3)).await;
    }
    Ok(())
}
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

    // -------------------------------------------------------------------------
    // STEP 1: Create accounts and deploy faucet
    // -------------------------------------------------------------------------
    println!("\n[STEP 1] Creating new accounts");
    let alice_account = create_basic_account(&mut client, keystore.clone()).await?;
    println!("Alice's account ID: {:?}", alice_account.id().to_hex());
    let bob_account = create_basic_account(&mut client, keystore.clone()).await?;
    println!("Bob's account ID: {:?}", bob_account.id().to_hex());

    println!("\nDeploying a new fungible faucet.");
    let faucet = create_basic_faucet(&mut client, keystore.clone()).await?;
    println!("Faucet account ID: {:?}", faucet.id().to_hex());
    client.sync_state().await?;

    // -------------------------------------------------------------------------
    // STEP 2: Mint tokens with P2ID
    // -------------------------------------------------------------------------
    println!("\n[STEP 2] Mint tokens with P2ID");
    let faucet_id = faucet.id();
    let amount: u64 = 100;
    let mint_amount = FungibleAsset::new(faucet_id, amount).unwrap();

    let tx_req = TransactionRequestBuilder::mint_fungible_asset(
        mint_amount,
        alice_account.id(),
        NoteType::Public,
        client.rng(),
    )
    .unwrap()
    .build()
    .unwrap();

    let tx_exec = client.new_transaction(faucet.id(), tx_req).await?;
    client.submit_transaction(tx_exec.clone()).await?;

    let p2id_note = if let OutputNote::Full(note) = tx_exec.created_notes().get_note(0) {
        note.clone()
    } else {
        panic!("Expected OutputNote::Full");
    };

    wait_for_notes(&mut client, &alice_account, 1).await?;

    let consume_req = TransactionRequestBuilder::new()
        .with_authenticated_input_notes([(p2id_note.id(), None)])
        .build()
        .unwrap();
    let tx_exec = client
        .new_transaction(alice_account.id(), consume_req)
        .await?;
    client.submit_transaction(tx_exec).await?;
    client.sync_state().await?;

    // -------------------------------------------------------------------------
    // STEP 3: Create iterative output note
    // -------------------------------------------------------------------------
    println!("\n[STEP 3] Create iterative output note");

    let assembler = TransactionKernel::assembler().with_debug_mode(true);
    let code = fs::read_to_string(Path::new("../masm/notes/iterative_output_note.masm")).unwrap();
    let rng = client.rng();
    let serial_num = rng.draw_word();

    // Create note metadata and tag
    let tag = NoteTag::for_public_use_case(0, 0, NoteExecutionMode::Local).unwrap();
    let metadata = NoteMetadata::new(
        alice_account.id(),
        NoteType::Public,
        tag,
        NoteExecutionHint::always(),
        Felt::new(0),
    )?;
    let note_script = NoteScript::compile(code, assembler.clone()).unwrap();
    let note_inputs = NoteInputs::new(vec![
        alice_account.id().prefix().as_felt(),
        alice_account.id().suffix(),
        tag.into(),
        Felt::new(0),
    ])
    .unwrap();

    let recipient = NoteRecipient::new(serial_num, note_script.clone(), note_inputs.clone());
    let vault = NoteAssets::new(vec![mint_amount.into()])?;
    let custom_note = Note::new(vault, metadata, recipient);

    let note_req = TransactionRequestBuilder::new()
        .with_own_output_notes(vec![OutputNote::Full(custom_note.clone())])
        .build()
        .unwrap();
    let tx_result = client
        .new_transaction(alice_account.id(), note_req)
        .await
        .unwrap();
    println!(
        "View transaction on MidenScan: https://testnet.midenscan.com/tx/{:?}",
        tx_result.executed_transaction().id()
    );
    let _ = client.submit_transaction(tx_result).await;
    client.sync_state().await?;

    // -------------------------------------------------------------------------
    // STEP 4: Consume the iterative output note
    // -------------------------------------------------------------------------
    println!("\n[STEP 4] Bob consumes the note and creates a copy");

    // Increment the serial number for the new note
    let serial_num_1 = [
        serial_num[0],
        serial_num[1],
        serial_num[2],
        Felt::new(serial_num[3].as_int() + 1),
    ];

    // Reuse the note_script and note_inputs
    let recipient = NoteRecipient::new(serial_num_1, note_script, note_inputs);

    // Note: Change metadata to include Bob's account as the creator
    let metadata = NoteMetadata::new(
        bob_account.id(),
        NoteType::Public,
        tag,
        NoteExecutionHint::always(),
        Felt::new(0),
    )?;

    let asset_amount_1 = FungibleAsset::new(faucet_id, 50).unwrap();
    let vault = NoteAssets::new(vec![asset_amount_1.into()])?;
    let output_note = Note::new(vault, metadata, recipient);

    let consume_custom_req = TransactionRequestBuilder::new()
        .with_unauthenticated_input_notes([(custom_note, None)])
        .with_expected_output_notes(vec![output_note])
        .build()
        .unwrap();
    let tx_result = client
        .new_transaction(bob_account.id(), consume_custom_req)
        .await
        .unwrap();
    println!(
        "Consumed Note Tx on MidenScan: https://testnet.midenscan.com/tx/{:?}",
        tx_result.executed_transaction().id()
    );
    println!("Account delta: {:?}", tx_result.account_delta().vault());
    let _ = client.submit_transaction(tx_result).await;

    Ok(())
}
```

Run the following command to execute `src/main.rs`:

```bash
cargo run --release
```

The output will look something like this:

```
Latest block: 18392

[STEP 1] Creating new accounts
Alice's account ID: "0xb23fa56edfb652100000354f9ad0f3"
Bob's account ID: "0xe4b869133a460d100000e036fe951e"

Deploying a new fungible faucet.
Faucet account ID: "0x0cc82fb7d6d5ba200000accff80c0c"

[STEP 2] Mint tokens with P2ID
0 consumable notes found for account 0xb23fa56edfb652100000354f9ad0f3. Waiting...

[STEP 3] Create iterative output note
View transaction on MidenScan: https://testnet.midenscan.com/tx/0x0c67c2de1b028bcb495f60f8ad81168f99cffeded00e293344dac6f45f702433

[STEP 4] Bob consumes the note and creates a copy
Consumed Note Tx on MidenScan: https://testnet.midenscan.com/tx/0x717e3fc01da330e25acef2d302691f3a9593ea80bb4668d6fdb12ca35a137119
Account delta: AccountVaultDelta { fungible: FungibleAssetDelta({V0(AccountIdV0 { prefix: 921038590427118112, suffix: 190009219746816 }): 50}), non_fungible: NonFungibleAssetDelta({}) }
```

---

### Running the example

To run the full example, navigate to the `rust-client` directory in the [miden-tutorials](https://github.com/0xPolygonMiden/miden-tutorials/) repository and run this command:

```bash
cd rust-client
cargo run --release --bin note_creation_in_masm
```

### Continue learning

Next tutorial: [How to Use Mappings in Miden Assembly](./mappings_in_masm_how_to.md)
