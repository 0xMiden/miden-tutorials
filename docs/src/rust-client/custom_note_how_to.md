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
use.miden::note
use.miden::contracts::wallets::basic->wallet

# => [HASH_PREIMAGE_SECRET]
begin

    # Hashing the secret number
    hperm
    # => [F,E,D]
    # E is digest

    dropw swapw dropw
    # => [DIGEST]

    # Writing the note inputs to memory
    push.0 exec.note::get_inputs drop drop
    # => [DIGEST]

    # Pad stack and load note inputs from memory
    padw push.0 mem_loadw
    # => [INPUTS, DIGEST]

    # Assert that the note input matches the digest
    # Will fail if the two hashes do not match
    assert_eqw
    # => []

    # Write the asset in note to memory address 0
    push.0 exec.note::get_assets
    # => [num_assets, dest_ptr]

    drop
    # => [dest_ptr]

    # Load asset from memory
    mem_loadw
    # => [ASSET]

    # Call receive asset in wallet
    call.wallet::receive_asset
    # => []
end
```

### How the assembly code works:

1. **Passing the Secret:**  
   The secret number is passed as `Note Arguments` into the note.
2. **Hashing the Secret:**  
   The `hperm` instruction applies a hash permutation to the secret number, resulting in a hash that takes up four stack elements.
3. **Stack Cleanup and Comparison:**  
   The assembly code extracts the digest, loads the note inputs from memory and checks if the computed hash matches the note’s stored hash.
4. **Asset Transfer:**  
   If the hash of the number passed in as `Note Arguments` matches the hash stored in the note inputs, the script continues, and the asset stored in the note is loaded from memory and passed to Bob’s wallet via the `wallet::receive_asset` function.

### 5. Consuming the note

With the note created, Bob can now consume it—but only if he provides the correct secret. When Bob initiates the transaction to consume the note, he must supply the same secret number used when Alice created the note. The custom note’s logic will hash the secret and compare it with its stored hash. If they match, Bob’s wallet receives the asset.

---

## Full Rust code example

The following Rust code demonstrates how to implement the steps outlined above using the Miden client library:

```rust
use rand::{rngs::StdRng, RngCore};
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
    Client, ClientError, Felt, Word,
};
use miden_objects::Hasher;

// Helper to create a basic account
async fn create_basic_account(
    client: &mut Client,
    keystore: FilesystemKeyStore<StdRng>,
) -> Result<miden_client::account::Account, ClientError> {
    let mut init_seed = [0_u8; 32];
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

    let keystore = FilesystemKeyStore::new("./keystore".into()).unwrap();

    // -------------------------------------------------------------------------
    // STEP 1: Create accounts and deploy faucet
    // -------------------------------------------------------------------------
    println!("\n[STEP 1] Creating new accounts");
    let alice_account = create_basic_account(&mut client, keystore.clone()).await?;
    println!("Alice's account ID: {:?}", alice_account.id().to_hex());
    let bob_account = create_basic_account(&mut client, keystore.clone()).await?;
    println!("Bob's account ID: {:?}", bob_account.id().to_hex());

    println!("\nDeploying a new fungible faucet.");
    let faucet = create_basic_faucet(&mut client, keystore).await?;
    println!("Faucet account ID: {:?}", faucet.id().to_hex());
    client.sync_state().await?;

    // -------------------------------------------------------------------------
    // STEP 2: Mint tokens with P2ID
    // -------------------------------------------------------------------------
    println!("\n[STEP 2] Mint tokens with P2ID");
    let faucet_id = faucet.id();
    let amount: u64 = 100;
    let mint_amount = FungibleAsset::new(faucet_id, amount).unwrap();
    let tx_request = TransactionRequestBuilder::mint_fungible_asset(
        mint_amount,
        alice_account.id(),
        NoteType::Public,
        client.rng(),
    )
    .unwrap()
    .build()
    .unwrap();
    let tx_exec = client.new_transaction(faucet.id(), tx_request).await?;
    client.submit_transaction(tx_exec.clone()).await?;

    let p2id_note = if let OutputNote::Full(note) = tx_exec.created_notes().get_note(0) {
        note.clone()
    } else {
        panic!("Expected OutputNote::Full");
    };

    sleep(Duration::from_secs(3)).await;
    wait_for_notes(&mut client, &alice_account, 1).await?;

    let consume_request = TransactionRequestBuilder::new()
        .with_authenticated_input_notes([(p2id_note.id(), None)])
        .build()
        .unwrap();
    let tx_exec = client
        .new_transaction(alice_account.id(), consume_request)
        .await?;
    client.submit_transaction(tx_exec).await?;
    client.sync_state().await?;

    // -------------------------------------------------------------------------
    // STEP 3: Create custom note
    // -------------------------------------------------------------------------
    println!("\n[STEP 3] Create custom note");
    let mut secret_vals = vec![Felt::new(1), Felt::new(2), Felt::new(3), Felt::new(4)];
    secret_vals.splice(0..0, Word::default().iter().cloned());
    let digest = Hasher::hash_elements(&secret_vals);
    println!("digest: {:?}", digest);

    let assembler = TransactionKernel::assembler().with_debug_mode(true);
    let code = fs::read_to_string(Path::new("../masm/notes/hash_preimage_note.masm")).unwrap();
    let serial_num = client.rng().draw_word();
    let note_script = NoteScript::compile(code, assembler).unwrap();
    let note_inputs = NoteInputs::new(digest.to_vec()).unwrap();
    let recipient = NoteRecipient::new(serial_num, note_script, note_inputs);
    let tag = NoteTag::for_public_use_case(0, 0, NoteExecutionMode::Local).unwrap();
    let metadata = NoteMetadata::new(
        alice_account.id(),
        NoteType::Public,
        tag,
        NoteExecutionHint::always(),
        Felt::new(0),
    )?;
    let vault = NoteAssets::new(vec![mint_amount.into()])?;
    let custom_note = Note::new(vault, metadata, recipient);
    println!("note hash: {:?}", custom_note.commitment());

    let note_request = TransactionRequestBuilder::new()
        .with_own_output_notes(vec![OutputNote::Full(custom_note.clone())])
        .build()
        .unwrap();
    let tx_result = client
        .new_transaction(alice_account.id(), note_request)
        .await
        .unwrap();
    println!(
        "View transaction on MidenScan: https://testnet.midenscan.com/tx/{:?}",
        tx_result.executed_transaction().id()
    );
    let _ = client.submit_transaction(tx_result).await;
    client.sync_state().await?;

    // -------------------------------------------------------------------------
    // STEP 4: Consume the Custom Note
    // -------------------------------------------------------------------------
    wait_for_notes(&mut client, &bob_account, 1).await?;
    println!("\n[STEP 4] Bob consumes the Custom Note with Correct Secret");
    let secret = [Felt::new(1), Felt::new(2), Felt::new(3), Felt::new(4)];
    let consume_custom_request = TransactionRequestBuilder::new()
        .with_authenticated_input_notes([(custom_note.id(), Some(secret))])
        .build()
        .unwrap();
    let tx_result = client
        .new_transaction(bob_account.id(), consume_custom_request)
        .await
        .unwrap();
    println!(
        "Consumed Note Tx on MidenScan: https://testnet.midenscan.com/tx/{:?} \n",
        tx_result.executed_transaction().id()
    );
    println!("account delta: {:?}", tx_result.account_delta().vault());
    let _ = client.submit_transaction(tx_result).await;

    Ok(())
}
```

The output of our program will look something like this:

```
Latest block: 17829

[STEP 1] Creating new accounts
Alice's account ID: "0x667909fa154e6d100000b61462a7ac"
Bob's account ID: "0xc05d939c75628210000029f9172d7d"

Deploying a new fungible faucet.
Faucet account ID: "0x0065844c736713200000b88008ea44"

[STEP 2] Mint tokens with P2ID
0 consumable notes found for account 0x667909fa154e6d100000b61462a7ac. Waiting...

[STEP 3] Create custom note
digest: RpoDigest([14371582251229115050, 1386930022051078873, 17689831064175867466, 9632123050519021080])
note hash: RpoDigest([11506510464714929313, 9338809316352932727, 13027456524296482454, 16157106531420254030])
View transaction on MidenScan: https://testnet.midenscan.com/tx/0xb3f2fb0f83c262ab64c48d8c3c37e802c88abd69c0a0774d92e84781691bbf4c
0 consumable notes found for account 0xc05d939c75628210000029f9172d7d. Waiting...

[STEP 4] Bob consumes the Custom Note with Correct Secret
Consumed Note Tx on MidenScan: https://testnet.midenscan.com/tx/0x63e3726201897f355c8322a7cf086738b3493daed42c145389acdf3618014d56

account delta: AccountVaultDelta { fungible: FungibleAssetDelta({V0(AccountIdV0 { prefix: 28574436536292128, suffix: 202860044895232 }): 100}), non_fungible: NonFungibleAssetDelta({}) }
```

## Conclusion

You have now seen how to create a custom note on Miden that requires a secret preimage to be consumed. We covered:

1. Creating and funding accounts (Alice and Bob)
2. Hashing a secret number
3. Building a note with custom logic in Miden Assembly
4. Consuming the note by providing the correct secret

By leveraging Miden’s privacy features, you can create customized logic for secure asset transfers that depend on keeping parts of the transaction private.

### Running the example

To run the custom note example, navigate to the `rust-client` directory in the [miden-tutorials](https://github.com/0xPolygonMiden/miden-tutorials/) repository and run this command:

```bash
cd rust-client
cargo run --release --bin hash_preimage_note
```

### Continue learning

Next tutorial: [Foreign Procedure Invocation](foreign_procedure_invocation_tutorial.md)
