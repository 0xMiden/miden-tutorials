---
title: "How To Create Notes with Custom Logic"
sidebar_position: 7
---

# How to Create a Custom Note

_Creating notes with custom logic_

## Overview

In this guide, we will create a custom note on Miden that can only be consumed by someone who knows the preimage of the hash stored in the note. This approach securely embeds assets into the note and restricts spending to those who possess the correct secret number.

By following the steps below and using the Miden Assembly code and Rust example, you will learn how to:

- Create a note with custom logic.
- Leverage Miden’s privacy features to keep certain transaction details private.

Unlike Ethereum, where all pending transactions are publicly visible in the mempool, Miden enables you to partially or completely hide transaction details.

## What we'll cover

- Writing Miden assembly for a note
- Consuming notes

## Step-by-step process

### 1. Creating two accounts: Alice & Bob

First, we create two basic accounts for the two users:

- **Alice:** The account that creates and funds the custom note.
- **Bob:** The account that will consume the note if they know the correct secret.

### 2. Hashing the secret number

The security of the custom note hinges on a secret number. Here, we will:

- Choose a secret number (for example, an array of four integers).
- For simplicity, we're only hashing 4 elements. Therefore, we prepend an empty word—consisting of 4 zero integers—as a placeholder. This is required by the RPO hashing algorithm to ensure the input has the correct structure and length for proper processing.
- Compute the hash of the secret. The resulting hash will serve as the note’s input, meaning that the note can only be consumed if the secret number’s hash preimage is provided during consumption.

### 3. Creating the custom note

Now, combine the minted asset and the secret hash to build the custom note. The note is created using the following key steps:

1. **Note Inputs:**
   - The note is set up with the asset and the hash of the secret number as its input.
2. **Miden Assembly Code:**
   - The Miden assembly note script ensures that the note can only be consumed if the provided secret, when hashed, matches the hash stored in the note input.

Below is the Miden Assembly code for the note:

```masm
use miden::protocol::active_note
use miden::standards::wallets::basic->wallet

# CONSTANTS
# =================================================================================================

const EXPECTED_DIGEST_PTR=0
const ASSET_PTR=100

# ERRORS
# =================================================================================================

const ERROR_DIGEST_MISMATCH="Expected digest does not match computed digest"

#! Inputs (arguments):  [HASH_PREIMAGE_SECRET]
#! Outputs: []
#!
#! Note inputs are assumed to be as follows:
#!  => EXPECTED_DIGEST
begin
    # => HASH_PREIMAGE_SECRET
    # Hashing the secret number
    hash
    # => [DIGEST]

    # Writing the note inputs to memory
    push.EXPECTED_DIGEST_PTR exec.active_note::get_inputs drop drop

    # Pad stack and load expected digest from memory
    padw push.EXPECTED_DIGEST_PTR mem_loadw_be
    # => [EXPECTED_DIGEST, DIGEST]

    # Assert that the note input matches the digest
    # Will fail if the two hashes do not match
    assert_eqw.err=ERROR_DIGEST_MISMATCH
    # => []

    # ---------------------------------------------------------------------------------------------
    # If the check is successful, we allow for the asset to be consumed
    # ---------------------------------------------------------------------------------------------

    # Write the asset in note to memory address ASSET_PTR
    push.ASSET_PTR exec.active_note::get_assets
    # => [num_assets, dest_ptr]

    drop
    # => [dest_ptr]

    # Load asset from memory
    mem_loadw_be
    # => [ASSET]

    # Call receive asset in wallet
    call.wallet::receive_asset
    # => []
end
```

### How the assembly code works:

1. **Constants and Error Handling:**  
   The code defines memory pointers (`EXPECTED_DIGEST_PTR` and `ASSET_PTR`) for better code organization and an error message for digest mismatches.
2. **Passing the Secret:**  
   The secret number is passed as `Note Arguments` into the note.
3. **Hashing the Secret:**  
   The `hash` instruction applies a hash permutation to the secret number, resulting in a digest that takes up four stack elements.
4. **Digest Comparison:**  
   The assembly code loads the expected digest from the note inputs stored in memory and compares it with the computed hash. If they don't match, the transaction fails with a clear error message.
5. **Asset Transfer:**  
   If the hash of the number passed in as `Note Arguments` matches the hash stored in the note inputs, the script continues, and the asset stored in the note is loaded from memory and passed to Bob's wallet via the `wallet::receive_asset` function.

### 5. Consuming the note

With the note created, Bob can now consume it—but only if he provides the correct secret. When Bob initiates the transaction to consume the note, he must supply the same secret number used when Alice created the note. The custom note’s logic will hash the secret and compare it with its stored hash. If they match, Bob’s wallet receives the asset.

---

## Full Rust code example

The following Rust code demonstrates how to implement the steps outlined above using the Miden client library:

```rust no_run
use miden_client::auth::AuthFalcon512Rpo;
use rand::RngCore;
use std::{fs, path::Path, sync::Arc};
use tokio::time::{sleep, Duration};

use miden_client::{
    account::{
        component::{BasicFungibleFaucet, BasicWallet},
        Account,
    },
    address::NetworkId,
    auth::AuthSecretKey,
    builder::ClientBuilder,
    crypto::FeltRng,
    keystore::FilesystemKeyStore,
    note::{
        Note, NoteAssets, NoteInputs, NoteMetadata, NoteRecipient, NoteTag, NoteType,
    },
    rpc::{Endpoint, GrpcClient},
    store::TransactionFilter,
    transaction::{OutputNote, TransactionId, TransactionRequestBuilder, TransactionStatus},
    Client, ClientError, Felt,
};
use miden_client_sqlite_store::ClientBuilderSqliteExt;
use miden_client::{
    account::{AccountBuilder, AccountStorageMode, AccountType},
    asset::{FungibleAsset, TokenSymbol},
};
use miden_protocol::Hasher;

// Helper to create a basic account
async fn create_basic_account(
    client: &mut Client<FilesystemKeyStore>,
    keystore: &Arc<FilesystemKeyStore>,
) -> Result<Account, ClientError> {
    let mut init_seed = [0_u8; 32];
    client.rng().fill_bytes(&mut init_seed);

    let key_pair = AuthSecretKey::new_falcon512_rpo();

    let account = AccountBuilder::new(init_seed)
        .account_type(AccountType::RegularAccountUpdatableCode)
        .storage_mode(AccountStorageMode::Public)
        .with_auth_component(AuthFalcon512Rpo::new(key_pair.public_key().to_commitment()))
        .with_component(BasicWallet)
        .build()
        .unwrap();

    client.add_account(&account, false).await?;
    keystore.add_key(&key_pair).unwrap();

    Ok(account)
}

async fn create_basic_faucet(
    client: &mut Client<FilesystemKeyStore>,
    keystore: &Arc<FilesystemKeyStore>,
) -> Result<Account, ClientError> {
    let mut init_seed = [0u8; 32];
    client.rng().fill_bytes(&mut init_seed);

    let key_pair = AuthSecretKey::new_falcon512_rpo();
    let symbol = TokenSymbol::new("MID").unwrap();
    let decimals = 8;
    let max_supply = Felt::new(1_000_000);

    let account = AccountBuilder::new(init_seed)
        .account_type(AccountType::FungibleFaucet)
        .storage_mode(AccountStorageMode::Public)
        .with_auth_component(AuthFalcon512Rpo::new(key_pair.public_key().to_commitment()))
        .with_component(BasicFungibleFaucet::new(symbol, decimals, max_supply).unwrap())
        .build()
        .unwrap();

    client.add_account(&account, false).await?;
    keystore.add_key(&key_pair).unwrap();

    Ok(account)
}

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
    // STEP 1: Create accounts and deploy faucet
    // -------------------------------------------------------------------------
    println!("\n[STEP 1] Creating new accounts");
    let alice_account = create_basic_account(&mut client, &keystore).await?;
    println!(
        "Alice's account ID: {:?}",
        alice_account.id().to_bech32(NetworkId::Testnet)
    );
    let bob_account = create_basic_account(&mut client, &keystore).await?;
    println!(
        "Bob's account ID: {:?}",
        bob_account.id().to_bech32(NetworkId::Testnet)
    );

    println!("\nDeploying a new fungible faucet.");
    let faucet = create_basic_faucet(&mut client, &keystore).await?;
    println!(
        "Faucet account ID: {:?}",
        faucet.id().to_bech32(NetworkId::Testnet)
    );
    client.sync_state().await?;

    // -------------------------------------------------------------------------
    // STEP 2: Mint tokens with P2ID
    // -------------------------------------------------------------------------
    println!("\n[STEP 2] Mint tokens with P2ID");
    let faucet_id = faucet.id();
    let amount: u64 = 100;
    let mint_amount = FungibleAsset::new(faucet_id, amount).unwrap();
    let tx_request = TransactionRequestBuilder::new()
        .build_mint_fungible_asset(
            mint_amount,
            alice_account.id(),
            NoteType::Public,
            client.rng(),
        )
        .unwrap();

    let tx_id = client
        .submit_new_transaction(faucet.id(), tx_request)
        .await?;
    println!("Minted tokens. TX: {:?}", tx_id);

    // Wait for the note to be available
    client.sync_state().await?;
    wait_for_tx(&mut client, tx_id).await?;

    // Consume the minted note
    let consumable_notes = client
        .get_consumable_notes(Some(alice_account.id()))
        .await?;

    if let Some((note_record, _)) = consumable_notes.first() {
        let note: Note = note_record.clone().try_into()?;
        let consume_request = TransactionRequestBuilder::new()
            .build_consume_notes(vec![note])
            .unwrap();

        let tx_id = client
            .submit_new_transaction(alice_account.id(), consume_request)
            .await?;
        println!("Consumed minted note. TX: {:?}", tx_id);
    }

    client.sync_state().await?;

    // -------------------------------------------------------------------------
    // STEP 3: Create custom note
    // -------------------------------------------------------------------------
    println!("\n[STEP 3] Create custom note");
    let secret_vals = vec![Felt::new(1), Felt::new(2), Felt::new(3), Felt::new(4)];
    let digest = Hasher::hash_elements(&secret_vals);
    println!("digest: {:?}", digest);

    let code = fs::read_to_string(Path::new("../masm/notes/hash_preimage_note.masm")).unwrap();
    let serial_num = client.rng().draw_word();

    let note_script = client.code_builder().compile_note_script(code).unwrap();
    let note_inputs = NoteInputs::new(digest.to_vec()).unwrap();
    let recipient = NoteRecipient::new(serial_num, note_script, note_inputs);
    let tag = NoteTag::new(0);
    let metadata = NoteMetadata::new(alice_account.id(), NoteType::Public, tag);
    let vault = NoteAssets::new(vec![mint_amount.into()])?;
    let custom_note = Note::new(vault, metadata, recipient);
    println!("note hash: {:?}", custom_note.id().to_hex());

    let note_request = TransactionRequestBuilder::new()
        .own_output_notes(vec![OutputNote::Full(custom_note.clone())])
        .build()
        .unwrap();

    let tx_id = client
        .submit_new_transaction(alice_account.id(), note_request)
        .await?;
    println!(
        "View transaction on MidenScan: https://testnet.midenscan.com/tx/{:?}",
        tx_id
    );

    client.sync_state().await?;

    // -------------------------------------------------------------------------
    // STEP 4: Consume the Custom Note
    // -------------------------------------------------------------------------
    println!("\n[STEP 4] Bob consumes the Custom Note with Correct Secret");

    let secret = [Felt::new(1), Felt::new(2), Felt::new(3), Felt::new(4)];
    let consume_custom_request = TransactionRequestBuilder::new()
        .input_notes([(custom_note, Some(secret.into()))])
        .build()
        .unwrap();

    let tx_id = client
        .submit_new_transaction(bob_account.id(), consume_custom_request)
        .await?;
    println!(
        "Consumed Note Tx on MidenScan: https://testnet.midenscan.com/tx/{:?} \n",
        tx_id
    );

    Ok(())
}
```

The output of our program will look something like this:

```text
Latest block: 226943

[STEP 1] Creating new accounts
Alice's account ID: "mtst1qqufkq3xr0rr5yqqqwgrc20ctythccy6"
Bob's account ID: "mtst1qz76c9fvhvms2yqqqvvw8tf6m5h86y2h"

Deploying a new fungible faucet.
Faucet account ID: "mtst1qpwsgjstpwvykgqqqwwzgz3u5vwuuywe"

[STEP 2] Mint tokens with P2ID
Note 0x88d8c4a50c0e6342e58026b051fb6038867de21d3bd3963aec67fd6c45861faf not found. Waiting...
Note 0x88d8c4a50c0e6342e58026b051fb6038867de21d3bd3963aec67fd6c45861faf not found. Waiting...
✅ note found 0x88d8c4a50c0e6342e58026b051fb6038867de21d3bd3963aec67fd6c45861faf

[STEP 3] Create custom note
digest: RpoDigest([14371582251229115050, 1386930022051078873, 17689831064175867466, 9632123050519021080])
note hash: "0x14c66143377223e090e5b4da0d1e5ce6c6521622ad5b92161a704a25c915769b"
View transaction on MidenScan: https://testnet.midenscan.com/tx/0xffbee228a2c6283efe958c6b3cd31af88018c029221b413b0f23fcfacb2cb611

[STEP 4] Bob consumes the Custom Note with Correct Secret
Consumed Note Tx on MidenScan: https://testnet.midenscan.com/tx/0xe6c8bb7b469e03dcacd8f1f400011a781e96ad4266ede11af8e711379e85b929

account delta: AccountVaultDelta { fungible: FungibleAssetDelta({V0(AccountIdV0 { prefix: 6702563556733766432, suffix: 1016103534633728 }): 100}), non_fungible: NonFungibleAssetDelta({}) }
```

## Conclusion

You have now seen how to create a custom note on Miden that requires a secret preimage to be consumed. We covered:

1. Creating and funding accounts (Alice and Bob)
2. Hashing a secret number
3. Building a note with custom logic in Miden Assembly
4. Consuming the note by providing the correct secret

By leveraging Miden’s privacy features, you can create customized logic for secure asset transfers that depend on keeping parts of the transaction private.

### Running the example

To run the custom note example, navigate to the `rust-client` directory in the [miden-tutorials](https://github.com/0xMiden/miden-tutorials/) repository and run this command:

```bash
cd rust-client
cargo run --release --bin hash_preimage_note
```

### Continue learning

Next tutorial: [How to Use Unauthenticated Notes](unauthenticated_note_how_to.md)
