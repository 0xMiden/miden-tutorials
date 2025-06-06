# Delegated Proving

_Using delegated proving to minimize transaction proving times on computationally constrained devices_

## Overview

In this tutorial we will cover how to use delegated proving with the Miden Rust client to minimize the time it takes to generate a valid transaction proof. In the code below, we will create an account, mint tokens from a faucet, then send the tokens to another account using delegated proving.

## Prerequisites

This tutorial assumes you have basic familiarity with the Miden Rust client.

## What we'll cover

- Explaining what "delegated proving" is and its pros and cons
- How to use delegated proving with the Rust client

## What is Delegated Proving?

Before diving into our code example, let's clarify what "delegated proving" means.

Delegated proving is the process of outsourcing the ZK proof generation of your transaction to a third party. For certain computationally constrained devices such as mobile phones and web browser environments, generating ZK proofs might take too long to ensure an acceptable user experience. Devices that do not have the computational resources to generate Miden proofs in under 1-2 seconds can use delegated proving to provide a more responsive user experience.

_How does it work?_ When a user choses to use delegated proving, they send off their locally executed transaction to a dedicated server. This dedicated server generates the ZK proof for the executed transaction and sends the proof back to the user. Proving a transaction with delegated proving is trustless, meaning if the delegated prover is malicious, they could not compromise the security of the account that is submitting a transaction to be processed by the delegated prover.

The only downside of using delegated proving is that it reduces the privacy of the account that uses delegated proving, because the delegated prover would have knowledge of the inputs to the transaction that is being proven. For example, it would not be advisable to use delegated proving in the case of our "How to Create a Custom Note" tutorial, since the note we create requires knowledge of a hash preimage to redeem the assets in the note. Using delegated proving would reveal the hash preimage to the server running the delegated proving service.

Anyone can run their own delegated prover server. If you are building a product on Miden, it may make sense to run your own delegated prover server for your users. To run your own delegated proving server, follow the instructions here: https://crates.io/crates/miden-proving-service

## Step 1: Initialize your repository

Create a new Rust repository for your Miden project and navigate to it with the following command:

```bash
cargo new miden-delegated-proving-app
cd miden-delegated-proving-app
```

Add the following dependencies to your `Cargo.toml` file:

```toml
[dependencies]
miden-client = { version = "0.9.0", features = ["testing", "concurrent", "tonic", "sqlite"] }
miden-lib = { version = "0.9", default-features = false }
miden-objects = { version = "0.9", default-features = false }
miden-crypto = { version = "0.14.1", features = ["executable"] }
miden-assembly = "0.14.0"
rand = { version = "0.9" }
serde = { version = "1", features = ["derive"] }
serde_json = { version = "1.0", features = ["raw_value"] }
tokio = { version = "1.40", features = ["rt-multi-thread", "net", "macros"] }
rand_chacha = "0.9.0"
miden-client-tools = "0.2.0"
```

## Step 2: Initialize the client and delegated prover endpoint and construct transactions

Similarly to previous tutorials, we must instantiate the client.
We construct a `RemoteTransactionProver` that points to our delegated-proving service running at https://tx-prover.testnet.miden.io.

```rust
use std::sync::Arc;

use miden_client::account::AccountId;
use miden_client::crypto::FeltRng;
use miden_client::{
    asset::FungibleAsset,
    keystore::FilesystemKeyStore,
    note::NoteType,
    rpc::Endpoint,
    transaction::{OutputNote, TransactionProver, TransactionRequestBuilder},
    ClientError, Felt, RemoteTransactionProver,
};
use miden_client_tools::{
    create_basic_account, create_exact_p2id_note, instantiate_client, mint_from_faucet_for_account,
};

#[tokio::main]
async fn main() -> Result<(), ClientError> {
    // Initialize client, keystore, & delegated prover endpoint
    let endpoint = Endpoint::testnet();
    let mut client = instantiate_client(endpoint, None).await.unwrap();

    let keystore: FilesystemKeyStore<rand::prelude::StdRng> =
        FilesystemKeyStore::new("./keystore".into()).unwrap();

    let remote_tx_prover: RemoteTransactionProver =
        RemoteTransactionProver::new("https://tx-prover.testnet.miden.io");
    let tx_prover: Arc<dyn TransactionProver + 'static> = Arc::new(remote_tx_prover);

    let sync_summary = client.sync_state().await.unwrap();
    println!("Latest block: {}", sync_summary.block_num);

    let (alice_account, _) = create_basic_account(&mut client, keystore.clone())
        .await
        .unwrap();

    let (bob_account, _) = create_basic_account(&mut client, keystore.clone())
        .await
        .unwrap();

    // import public faucet id
    let faucet_id = AccountId::from_hex("0x9526e379bc3ad4200000b201b1f0f3").unwrap();
    client.import_account_by_id(faucet_id).await.unwrap();
    let binding = client.get_account(faucet_id).await.unwrap().unwrap();
    let faucet = binding.account();

    let _ = mint_from_faucet_for_account(&mut client, &alice_account, &faucet, 1000, None)
        .await
        .unwrap();

    let account = client
        .get_account(alice_account.id())
        .await
        .unwrap()
        .unwrap();

    println!(
        "Alice initial account balance: {:?}",
        account.account().vault().get_balance(faucet.id())
    );

    // Creating 10 separate P2ID notes with 10 tokens each to send to Bob
    let send_amount = 10;
    let fungible_asset = FungibleAsset::new(faucet.id(), send_amount).unwrap();
    let mut p2id_notes = vec![];
    for _ in 0..=9 {
        let p2id_note = create_exact_p2id_note(
            alice_account.id(),
            bob_account.id(),
            vec![fungible_asset.into()],
            NoteType::Public,
            Felt::new(0),
            client.rng().draw_word(),
        )?;
        p2id_notes.push(p2id_note);
    }

    // Specifying output notes and creating a tx request to create them
    let output_notes: Vec<OutputNote> = p2id_notes.into_iter().map(OutputNote::Full).collect();
    let transaction_request = TransactionRequestBuilder::new()
        .with_own_output_notes(output_notes)
        .build()
        .unwrap();
    let tx_execution_result = client
        .new_transaction(alice_account.id(), transaction_request)
        .await?;

    // Using the `submit_transaction_with_prover` function
    // to offload proof generation to the delegated prover
    client
        .submit_transaction_with_prover(tx_execution_result, tx_prover.clone())
        .await
        .unwrap();

    client.sync_state().await.unwrap();

    let account = client
        .get_account(alice_account.id())
        .await
        .unwrap()
        .unwrap();

    println!(
        "Alice final account balance: {:?}",
        account.account().vault().get_balance(faucet.id())
    );

    Ok(())
}
```

Now let's run the `src/main.rs` program:

```bash
cargo run --release
```

The output will look like this:

```
Latest block: 751265
0 consumable notes found for account 0x33959f3ba0998010000ba4a311179b. Waiting...
0 consumable notes found for account 0x33959f3ba0998010000ba4a311179b. Waiting...
Alice Account balance: Ok(1000)
Alice Account balance: Ok(900)
```

### Running the example

To run a full working example navigate to the `rust-client` directory in the [miden-tutorials](https://github.com/0xPolygonMiden/miden-tutorials/) repository and run this command:

```bash
cd rust-client
cargo run --release --bin delegated_prover
```

### Continue learning

Next tutorial: [Deploying a Counter Contract](counter_contract_tutorial.md)
