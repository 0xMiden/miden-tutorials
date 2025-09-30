---
title: "Delegated Proving"
sidebar_position: 12
---

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

Anyone can run their own delegated prover server. If you are building a product on Miden, it may make sense to run your own delegated prover server for your users. To run your own delegated proving server, follow the instructions here: https://crates.io/crates/miden-remote-prover.

## Step 1: Initialize your repository

Create a new Rust repository for your Miden project and navigate to it with the following command:

```bash
cargo new miden-delegated-proving-app
cd miden-delegated-proving-app
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

## Step 2: Initialize the client and delegated prover endpoint and construct transactions

Similarly to previous tutorials, we must instantiate the client.
We construct a `RemoteTransactionProver` that points to our delegated-proving service running at https://tx-prover.testnet.miden.io.

```rust
use miden_client::auth::AuthSecretKey;
use std::sync::Arc;

use miden_client::account::{AccountStorageMode, AccountType};
use miden_client::builder::ClientBuilder;
use miden_client::crypto::SecretKey;
use miden_client::rpc::TonicRpcClient;
use miden_client::{
    keystore::FilesystemKeyStore,
    rpc::Endpoint,
    transaction::{TransactionProver, TransactionRequestBuilder},
    ClientError, RemoteTransactionProver, ScriptBuilder,
};
use miden_lib::account::wallets::create_basic_wallet;
use miden_lib::AuthScheme;

#[tokio::main]
async fn main() -> Result<(), ClientError> {
    // Initialize client & keystore
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

    let keystore: FilesystemKeyStore<rand::prelude::StdRng> =
        FilesystemKeyStore::new("./keystore".into()).unwrap();

    let key_pair = SecretKey::with_rng(client.rng());

    let sync_summary = client.sync_state().await.unwrap();
    println!("Latest block: {}", sync_summary.block_num);

    let (alice_account, seed) = create_basic_wallet(
        [0; 32],
        AuthScheme::RpoFalcon512 {
            pub_key: key_pair.public_key(),
        },
        AccountType::RegularAccountImmutableCode,
        AccountStorageMode::Private,
    )
    .unwrap();

    client
        .add_account(&alice_account, Some(seed), false)
        .await?;
    keystore
        .add_key(&AuthSecretKey::RpoFalcon512(key_pair))
        .unwrap();

    // -------------------------------------------------------------------------
    // Setup the remote tx prover
    // -------------------------------------------------------------------------
    let remote_tx_prover: RemoteTransactionProver =
        RemoteTransactionProver::new("https://tx-prover.testnet.miden.io");
    let tx_prover: Arc<dyn TransactionProver + 'static> = Arc::new(remote_tx_prover);

    // We use a dummy transaction request to showcase delegated proving.
    // The only effect of this tx should be increasing Alice's nonce.
    println!("Alice nonce initial: {:?}", alice_account.nonce());
    let script_code = "begin push.1 drop end";
    let tx_script = ScriptBuilder::new(true)
        .compile_tx_script(script_code)
        .unwrap();

    let transaction_request = TransactionRequestBuilder::new()
        .custom_script(tx_script)
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

    println!("Alice nonce has increased: {:?}", account.account().nonce());

    Ok(())
}
```

Now let's run the `src/main.rs` program:

```bash
cargo run --release
```

The output will look like this:

```text
Latest block: 226954
Alice initial account balance: Ok(1000)
Alice final account balance: Ok(900)
```

### Running the example

To run a full working example navigate to the `rust-client` directory in the [miden-tutorials](https://github.com/0xMiden/miden-tutorials/) repository and run this command:

```bash
cd rust-client
cargo run --release --bin delegated_prover
```

### Continue learning

Next tutorial: [Consuming On-Chain Price Data from the Pragma Oracle](oracle_tutorial.md)
