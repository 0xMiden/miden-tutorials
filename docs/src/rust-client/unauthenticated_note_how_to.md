# How to Use Unauthenticated Notes

_Using unauthenticated notes for optimistic note consumption_

## Overview

In this guide, we will explore how to leverage unauthenticated notes on Miden to settle transactions faster than the blocktime. Unauthenticated notes are essentially UTXOs that have not yet been fully committed into a block. This feature allows the notes to be created and consumed within the same block.

We construct a chain of transactions using the unauthenticated notes method on the transaction builder. Unauthenticated notes are also referred to as "unauthenticated notes" or "erasable notes". We also demonstrate how a note can be serialized and deserialized, highlighting the ability to transfer notes between client instances for asset transfers that can be settled between parties faster than the blocktime.

For example, our demo creates a chain of unauthenticated note transactions:

```markdown
Alice ➡ Bob ➡ Charlie ➡ Dave ➡ Eve ➡ Frank ➡ ...
```

## What we'll cover

- **Introduction to Unauthenticated Notes:** Understand what unauthenticated notes are and how they differ from standard notes.
- **Serialization Example:** See how to serialize and deserialize a note to demonstrate how notes can be propagated to client instances faster than the blocktime.
- **Performance Insights:** Observe how unauthenticated notes can reduce transaction times dramatically.

## Step-by-step process

1. **Client Initialization:**
   - Set up an RPC client to connect with the Miden testnet.
   - Initialize a random coin generator and a store for persisting account data.

2. **Deploying a Fungible Faucet:**
   - Use a random seed to deploy a fungible faucet.
   - Configure the faucet parameters (symbol, decimals, and max supply) and add it to the client.

3. **Creating Wallet Accounts:**
   - Build multiple wallet accounts using a secure key generation process.
   - Add these accounts to the client, making them ready for transactions.

4. **Minting and Transacting with Unauthenticated Notes:**
   - Mint tokens for one of the accounts (Alice) from the deployed faucet.
   - Create a note representing the minted tokens.
   - Build and submit a transaction that uses the unauthenticated note via the "unauthenticated" method.
   - Serialize the note to demonstrate how it could be transferred to another client instance.
   - Consume the note in a subsequent transaction, effectively creating a chain of unauthenticated transactions.

5. **Performance Timing and Syncing:**
   - Measure the time taken for each transaction iteration.
   - Sync the client state and print account balances to verify the transactions.

## Full Rust code example

```rust
use rand::RngCore;
use std::sync::Arc;
use tokio::time::{Duration, Instant};

use miden_client::{
    account::{
        component::{BasicFungibleFaucet, BasicWallet, RpoFalcon512},
        AccountBuilder, AccountStorageMode, AccountType,
    },
    asset::{FungibleAsset, TokenSymbol},
    auth::AuthSecretKey,
    builder::ClientBuilder,
    crypto::SecretKey,
    keystore::FilesystemKeyStore,
    note::{create_p2id_note, Note, NoteType},
    rpc::{Endpoint, TonicRpcClient},
    transaction::{OutputNote, TransactionRequestBuilder},
    utils::{Deserializable, Serializable},
    ClientError, Felt,
};

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

    //------------------------------------------------------------
    // STEP 1: Deploy a fungible faucet
    //------------------------------------------------------------
    println!("\n[STEP 1] Deploying a new fungible faucet.");

    // Faucet seed
    let mut init_seed = [0_u8; 32];
    client.rng().fill_bytes(&mut init_seed);

    let key_pair = SecretKey::with_rng(client.rng());

    // Anchor block
    let anchor_block = client.get_latest_epoch_block().await.unwrap();

    // Faucet parameters
    let symbol = TokenSymbol::new("MID").unwrap();
    let decimals = 8;
    let max_supply = Felt::new(1_000_000);

    // Generate key pair

    // Build the account
    let builder = AccountBuilder::new(init_seed)
        .anchor((&anchor_block).try_into().unwrap())
        .account_type(AccountType::FungibleFaucet)
        .storage_mode(AccountStorageMode::Public)
        .with_component(RpoFalcon512::new(key_pair.public_key()))
        .with_component(BasicFungibleFaucet::new(symbol, decimals, max_supply).unwrap());

    let (faucet_account, seed) = builder.build().unwrap();

    // Add the faucet to the client
    client
        .add_account(&faucet_account, Some(seed), false)
        .await?;
    println!("Faucet account ID: {}", faucet_account.id().to_bech32(NetworkId::Testnet));

    // Add the key pair to the keystore
    keystore
        .add_key(&AuthSecretKey::RpoFalcon512(key_pair))
        .unwrap();

    // Resync to show newly deployed faucet
    tokio::time::sleep(Duration::from_secs(2)).await;
    client.sync_state().await?;

    //------------------------------------------------------------
    // STEP 2: Create basic wallet accounts
    //------------------------------------------------------------
    println!("\n[STEP 2] Creating new accounts");

    let mut accounts = vec![];
    let number_of_accounts = 10;

    for i in 0..number_of_accounts {
        let mut init_seed = [0_u8; 32];
        client.rng().fill_bytes(&mut init_seed);
        let key_pair = SecretKey::with_rng(client.rng());
        let builder = AccountBuilder::new(init_seed)
            .anchor((&anchor_block).try_into().unwrap())
            .account_type(AccountType::RegularAccountUpdatableCode)
            .storage_mode(AccountStorageMode::Public)
            .with_component(RpoFalcon512::new(key_pair.public_key()))
            .with_component(BasicWallet);

        let (account, seed) = builder.build().unwrap();
        accounts.push(account.clone());
        println!("account id {:?}: {}", i, account.id().to_bech32(NetworkId::Testnet));
        client.add_account(&account, Some(seed), true).await?;

        // Add the key pair to the keystore
        keystore
            .add_key(&AuthSecretKey::RpoFalcon512(key_pair))
            .unwrap();
    }

    // For demo purposes, Alice is the first account.
    let alice = &accounts[0];

    //------------------------------------------------------------
    // STEP 3: Mint and consume tokens for Alice
    //------------------------------------------------------------
    println!("\n[STEP 3] Mint tokens");
    println!("Minting tokens for Alice...");
    let amount: u64 = 100;
    let fungible_asset_mint_amount = FungibleAsset::new(faucet_account.id(), amount).unwrap();
    let transaction_request = TransactionRequestBuilder::new()
        .build_mint_fungible_asset(
            fungible_asset_mint_amount,
            alice.id(),
            NoteType::Public,
            client.rng(),
        )
        .unwrap();

    let tx_execution_result = client
        .new_transaction(faucet_account.id(), transaction_request)
        .await?;
    client
        .submit_transaction(tx_execution_result.clone())
        .await?;

    // The minted fungible asset is public so output is a `Full` note type
    let p2id_note: Note =
        if let OutputNote::Full(note) = tx_execution_result.created_notes().get_note(0) {
            note.clone()
        } else {
            panic!("Expected Full note type");
        };

    let transaction_request = TransactionRequestBuilder::new()
        .with_unauthenticated_input_notes([(p2id_note, None)])
        .build()
        .unwrap();
    let tx_execution_result = client
        .new_transaction(alice.id(), transaction_request)
        .await?;
    client.submit_transaction(tx_execution_result).await?;
    client.sync_state().await?;

    //------------------------------------------------------------
    // STEP 4: Create unauthenticated note tx chain
    //------------------------------------------------------------
    println!("\n[STEP 4] Create unauthenticated note tx chain");
    let mut landed_blocks = vec![];
    let start = Instant::now();

    for i in 0..number_of_accounts - 1 {
        let loop_start = Instant::now();
        println!("\nunauthenticated tx {:?}", i + 1);
        println!("sender: {}", accounts[i].id().to_bech32(NetworkId::Testnet));
        println!("target: {}", accounts[i + 1].id().to_bech32(NetworkId::Testnet));

        // Time the creation of the p2id note
        let send_amount = 20;
        let fungible_asset_send_amount =
            FungibleAsset::new(faucet_account.id(), send_amount).unwrap();

        // for demo purposes, unauthenticated notes can be public or private
        let note_type = if i % 2 == 0 {
            NoteType::Private
        } else {
            NoteType::Public
        };

        let p2id_note = create_p2id_note(
            accounts[i].id(),
            accounts[i + 1].id(),
            vec![fungible_asset_send_amount.into()],
            note_type,
            Felt::new(0),
            client.rng(),
        )
        .unwrap();

        let output_note = OutputNote::Full(p2id_note.clone());

        // Time transaction request building
        let transaction_request = TransactionRequestBuilder::new()
            .with_own_output_notes(vec![output_note])
            .build()
            .unwrap();
        let tx_execution_result = client
            .new_transaction(accounts[i].id(), transaction_request)
            .await?;
        client.submit_transaction(tx_execution_result).await?;

        // Note serialization/deserialization
        // This demonstrates how you could send the serialized note to another client instance
        let serialized = p2id_note.to_bytes();
        let deserialized_p2id_note = Note::read_from_bytes(&serialized).unwrap();

        // Time consume note request building
        let consume_note_request =
            TransactionRequestBuilder::consume_notes(vec![deserialized_p2id_note.id()])
                .with_unauthenticated_input_notes([(deserialized_p2id_note, None)])
                .build()
                .unwrap();
        let tx_execution_result = client
            .new_transaction(accounts[i + 1].id(), consume_note_request)
            .await?;
        landed_blocks.push(tx_execution_result.block_num());
        client
            .submit_transaction(tx_execution_result.clone())
            .await?;

        println!(
            "Consumed Note Tx on MidenScan: https://testnet.midenscan.com/tx/{:?}",
            tx_execution_result.executed_transaction().id()
        );
        println!(
            "Total time for loop iteration {}: {:?}",
            i,
            loop_start.elapsed()
        );
    }

    println!(
        "\nTotal execution time for unauthenticated note txs: {:?}",
        start.elapsed()
    );
    println!("blocks: {:?}", landed_blocks);

    // Final resync and display account balances
    tokio::time::sleep(Duration::from_secs(3)).await;
    client.sync_state().await?;
    for account in accounts.clone() {
        let new_account = client.get_account(account.id()).await.unwrap().unwrap();
        let balance = new_account
            .account()
            .vault()
            .get_balance(faucet_account.id())
            .unwrap();
        println!("Account: {} balance: {}", account.id().to_bech32(NetworkId::Testnet), balance);
    }

    Ok(())
}
```

The output of our program will look something like this:

```
Latest block: 227040

[STEP 1] Deploying a new fungible faucet.
Faucet account ID: mtst1qqvhzywfzy4xugqqq0yqj28jxy3kr5hy

[STEP 2] Creating new accounts
account id 0: mtst1qrdwf5hv6wnqzyqqq06hyfvqryn2nam3
account id 1: mtst1qz7lwv4wh27xyyqqq026adcyc54ueccz
account id 2: mtst1qzzmpa7f3tcnkyqqqdgj4dan2q8r0s6c
account id 3: mtst1qrdclj0zp3v7qyqqqdn92ad87cl0rctl
account id 4: mtst1qre79420whvn2yqqq0udf4z8d5c3xwfj
account id 5: mtst1qpmfryrdjfwazyqqqdslm7gdhur80xhk
account id 6: mtst1qr0n4cxfddn2wyqqqv2vsc9mnqh0dtyj
account id 7: mtst1qrfmw4297mchwyqqqdfzq8dl2uu89uhq
account id 8: mtst1qpevlxpnuetesyqqqdwmsgd4zua84nda
account id 9: mtst1qre7lqnwt03zwyqqqvjdlj2w6yc87u4w

[STEP 3] Mint tokens
Minting tokens for Alice...

[STEP 4] Create unauthenticated note tx chain

unauthenticated tx 1
sender: mtst1qrdwf5hv6wnqzyqqq06hyfvqryn2nam3
target: mtst1qz7lwv4wh27xyyqqq026adcyc54ueccz
Consumed Note Tx on MidenScan: https://testnet.midenscan.com/tx/0x31f48117c645c5b4ccff78ef356bad764798d4f207925e492ebbae1b86ef4f55
Total time for loop iteration 0: 1.952243542s

unauthenticated tx 2
sender: mtst1qz7lwv4wh27xyyqqq026adcyc54ueccz
target: mtst1qzzmpa7f3tcnkyqqqdgj4dan2q8r0s6c
Consumed Note Tx on MidenScan: https://testnet.midenscan.com/tx/0x45b4c62c6e8e79a1c7200d1c84dc6304a88debd37b20b069dd739498827354c1
Total time for loop iteration 1: 2.091625458s

unauthenticated tx 3
sender: mtst1qzzmpa7f3tcnkyqqqdgj4dan2q8r0s6c
target: mtst1qrdclj0zp3v7qyqqqdn92ad87cl0rctl
Consumed Note Tx on MidenScan: https://testnet.midenscan.com/tx/0xb2241e10df8f6f891b910975a3b4f4fd47657c47de164138300d683cfca5dd61
Total time for loop iteration 2: 1.846021291s

unauthenticated tx 4
sender: mtst1qrdclj0zp3v7qyqqqdn92ad87cl0rctl
target: mtst1qre79420whvn2yqqq0udf4z8d5c3xwfj
Consumed Note Tx on MidenScan: https://testnet.midenscan.com/tx/0xd3ea6fa1da6c317f055ac4b069388d93b88d526039e01531879e75598e0f8cff
Total time for loop iteration 3: 1.877627958s

unauthenticated tx 5
sender: mtst1qre79420whvn2yqqq0udf4z8d5c3xwfj
target: mtst1qpmfryrdjfwazyqqqdslm7gdhur80xhk
Consumed Note Tx on MidenScan: https://testnet.midenscan.com/tx/0x6098638ec0ff7331432c037331ee7372977abe20af5c56315985fd314e21548d
Total time for loop iteration 4: 1.884586875s

unauthenticated tx 6
sender: mtst1qpmfryrdjfwazyqqqdslm7gdhur80xhk
target: mtst1qr0n4cxfddn2wyqqqv2vsc9mnqh0dtyj
Consumed Note Tx on MidenScan: https://testnet.midenscan.com/tx/0x8258292e49e0cfdd96603450c2de6738afecb1e7482ede0fb68ea375e884e1d8
Total time for loop iteration 5: 1.886505875s

unauthenticated tx 7
sender: mtst1qr0n4cxfddn2wyqqqv2vsc9mnqh0dtyj
target: mtst1qrfmw4297mchwyqqqdfzq8dl2uu89uhq
Consumed Note Tx on MidenScan: https://testnet.midenscan.com/tx/0x9e0f84e00a9393bf6e5f224d55ccdf8bd0ef32ee20c3299e2dfccf1771001dfd
Total time for loop iteration 6: 2.095149458s

unauthenticated tx 8
sender: mtst1qrfmw4297mchwyqqqdfzq8dl2uu89uhq
target: mtst1qpevlxpnuetesyqqqdwmsgd4zua84nda
Consumed Note Tx on MidenScan: https://testnet.midenscan.com/tx/0xa9db6445dfaa44ccf9dd52bf4cd8d9057946571ccb5299a7a56c59faf2ed2093
Total time for loop iteration 7: 1.935587291s

unauthenticated tx 9
sender: mtst1qpevlxpnuetesyqqqdwmsgd4zua84nda
target: mtst1qre7lqnwt03zwyqqqvjdlj2w6yc87u4w
Consumed Note Tx on MidenScan: https://testnet.midenscan.com/tx/0xba4bb4ae3c7aaf949cdd3be8c9ea52169f958e7dca8e9d4541fd5ac939393e41
Total time for loop iteration 8: 1.964682833s

Total execution time for unauthenticated note txs: 17.534611542s
blocks: [BlockNumber(227047), BlockNumber(227047), BlockNumber(227047), BlockNumber(227047), BlockNumber(227047), BlockNumber(227047), BlockNumber(227047), BlockNumber(227047), BlockNumber(227047)]
Account: mtst1qrdwf5hv6wnqzyqqq06hyfvqryn2nam3 balance: 80
Account: mtst1qz7lwv4wh27xyyqqq026adcyc54ueccz balance: 0
Account: mtst1qzzmpa7f3tcnkyqqqdgj4dan2q8r0s6c balance: 0
Account: mtst1qrdclj0zp3v7qyqqqdn92ad87cl0rctl balance: 0
Account: mtst1qre79420whvn2yqqq0udf4z8d5c3xwfj balance: 0
Account: mtst1qpmfryrdjfwazyqqqdslm7gdhur80xhk balance: 0
Account: mtst1qr0n4cxfddn2wyqqqv2vsc9mnqh0dtyj balance: 0
Account: mtst1qrfmw4297mchwyqqqdfzq8dl2uu89uhq balance: 0
Account: mtst1qpevlxpnuetesyqqqdwmsgd4zua84nda balance: 0
Account: mtst1qre7lqnwt03zwyqqqvjdlj2w6yc87u4w balance: 20
```

## Conclusion

Unauthenticated notes on Miden offer a powerful mechanism for achieving faster asset settlements by allowing notes to be both created and consumed within the same block. In this guide, we walked through:

- **Minting and Transacting with Unauthenticated Notes:** Building, serializing, and consuming notes quickly using the Miden client's "unauthenticated note" method.
- **Performance Observations:** Measuring and demonstrating how unauthenticated notes enable assets to be sent faster than the blocktime.

By following this guide, you should now have a clear understanding of how to build and deploy high-performance transactions using unauthenticated notes on Miden. Unauthenticated notes are the ideal approach for applications like central limit order books (CLOBs) or other DeFi platforms where transaction speed is critical.

### Running the example

To run the unauthenticated note transfer example, navigate to the `rust-client` directory in the [miden-tutorials](https://github.com/0xMiden/miden-tutorials/) repository and run this command:

```bash
cd rust-client
cargo run --release --bin unauthenticated_note_transfer
```

### Continue learning

Next tutorial: [How to Use Mappings in Miden Assembly](mappings_in_masm_how_to.md)
