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
    println!("Faucet account ID: {}", faucet_account.id().to_hex());

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
        println!("account id {:?}: {}", i, account.id().to_hex());
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
    let transaction_request = TransactionRequestBuilder::mint_fungible_asset(
        fungible_asset_mint_amount,
        alice.id(),
        NoteType::Public,
        client.rng(),
    )
    .unwrap()
    .build()
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
        println!("sender: {}", accounts[i].id().to_hex());
        println!("target: {}", accounts[i + 1].id().to_hex());

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
        println!("Account: {} balance: {}", account.id().to_hex(), balance);
    }

    Ok(())
}
```

The output of our program will look something like this:

```
Latest block: 17854

[STEP 1] Deploying a new fungible faucet.
Faucet account ID: 0x7a3c3e3e03fe43200000449196ce1f

[STEP 2] Creating new accounts
account id 0: 0x44d89b438d298e1000003636aa7a58
account id 1: 0xf275e0bcd03fd110000002b1cd6b60
account id 2: 0xf18208694c7926100000d1946f306e
account id 3: 0xc028077080d628100000f47f698791
account id 4: 0x16c973d5b5cb96100000674ca476f9
account id 5: 0x53ce6afddd744f100000b5d39c64bd
account id 6: 0x3b8ed3bfa7c9f9100000dfd8a12b9c
account id 7: 0x94117096753d06100000470857d9d2
account id 8: 0xa8dd5dc6d59e89100000e620b17531
account id 9: 0x3d0bdd225de2be1000004ffa75a2c1

[STEP 3] Mint tokens
Minting tokens for Alice...

[STEP 4] Create unauthenticated note tx chain

unauthenticated tx 1
sender: 0x44d89b438d298e1000003636aa7a58
target: 0xf275e0bcd03fd110000002b1cd6b60
Consumed Note Tx on MidenScan: https://testnet.midenscan.com/tx/0x2eb9c92e928595a55c8d98027cb8f434dcaef15a6ce9478518ba8083f80d7928
Total time for loop iteration 0: 3.126228875s

unauthenticated tx 2
sender: 0xf275e0bcd03fd110000002b1cd6b60
target: 0xf18208694c7926100000d1946f306e
Consumed Note Tx on MidenScan: https://testnet.midenscan.com/tx/0x8e780ab4715d1473e7babcf05bef6550b9bbaca8dc9460cee3dd3e25bd1f097d
Total time for loop iteration 1: 2.969214834s

unauthenticated tx 3
sender: 0xf18208694c7926100000d1946f306e
target: 0xc028077080d628100000f47f698791
Consumed Note Tx on MidenScan: https://testnet.midenscan.com/tx/0xdccd832ed22c9cd9054559d7746b5fe4cc9e78da50638e4a30973f4f0ea74e63
Total time for loop iteration 2: 2.967574333s

unauthenticated tx 4
sender: 0xc028077080d628100000f47f698791
target: 0x16c973d5b5cb96100000674ca476f9
Consumed Note Tx on MidenScan: https://testnet.midenscan.com/tx/0xc8fe1dd90b9008861e4ea3583195edadefbb9443f657ae296dfba0bf4ac56519
Total time for loop iteration 3: 2.86498225s

unauthenticated tx 5
sender: 0x16c973d5b5cb96100000674ca476f9
target: 0x53ce6afddd744f100000b5d39c64bd
Consumed Note Tx on MidenScan: https://testnet.midenscan.com/tx/0xaf4450834db4397de1832ea826c1fcecdf7fee0d8498110f857761e5f4c05bb6
Total time for loop iteration 4: 2.879300125s

unauthenticated tx 6
sender: 0x53ce6afddd744f100000b5d39c64bd
target: 0x3b8ed3bfa7c9f9100000dfd8a12b9c
Consumed Note Tx on MidenScan: https://testnet.midenscan.com/tx/0xb5f3b92272ffde9a5e9634fe8e5ef9d0dc2dc6e1695f09685f5a80f143fef421
Total time for loop iteration 5: 2.829184834s

unauthenticated tx 7
sender: 0x3b8ed3bfa7c9f9100000dfd8a12b9c
target: 0x94117096753d06100000470857d9d2
Consumed Note Tx on MidenScan: https://testnet.midenscan.com/tx/0xed5eaba98132dd7bce4421da72cac330758ca41bd8818edb07526f7b662d8827
Total time for loop iteration 6: 2.897448917s

unauthenticated tx 8
sender: 0x94117096753d06100000470857d9d2
target: 0xa8dd5dc6d59e89100000e620b17531
Consumed Note Tx on MidenScan: https://testnet.midenscan.com/tx/0xdd9cc26dbdbb542ef835780f9881708b9867190c353810ebce501955c5dad139
Total time for loop iteration 7: 2.864668333s

unauthenticated tx 9
sender: 0xa8dd5dc6d59e89100000e620b17531
target: 0x3d0bdd225de2be1000004ffa75a2c1
Consumed Note Tx on MidenScan: https://testnet.midenscan.com/tx/0xdd6e35b59032c0a22ce1e8f27c43ee7935ec1c2077ff1c37444ded09b879c330
Total time for loop iteration 8: 3.070943167s

Total execution time for unauthenticated note txs: 26.46999025s
blocks: [BlockNumber(17859), BlockNumber(17859), BlockNumber(17859), BlockNumber(17859), BlockNumber(17859), BlockNumber(17859), BlockNumber(17859), BlockNumber(17859), BlockNumber(17859)]
Account: 0x44d89b438d298e1000003636aa7a58 balance: 80
Account: 0xf275e0bcd03fd110000002b1cd6b60 balance: 0
Account: 0xf18208694c7926100000d1946f306e balance: 0
Account: 0xc028077080d628100000f47f698791 balance: 0
Account: 0x16c973d5b5cb96100000674ca476f9 balance: 0
Account: 0x53ce6afddd744f100000b5d39c64bd balance: 0
Account: 0x3b8ed3bfa7c9f9100000dfd8a12b9c balance: 0
Account: 0x94117096753d06100000470857d9d2 balance: 0
Account: 0xa8dd5dc6d59e89100000e620b17531 balance: 0
Account: 0x3d0bdd225de2be1000004ffa75a2c1 balance: 20
```

## Conclusion

Unauthenticated notes on Miden offer a powerful mechanism for achieving faster asset settlements by allowing notes to be both created and consumed within the same block. In this guide, we walked through:

- **Minting and Transacting with Unauthenticated Notes:** Building, serializing, and consuming notes quickly using the Miden client's "unauthenticated note" method.
- **Performance Observations:** Measuring and demonstrating how unauthenticated notes enable assets to be sent faster than the blocktime.

By following this guide, you should now have a clear understanding of how to build and deploy high-performance transactions using unauthenticated notes on Miden. Unauthenticated notes are the ideal approach for applications like central limit order books (CLOBs) or other DeFi platforms where transaction speed is critical.

### Running the example

To run the unauthenticated note transfer example, navigate to the `rust-client` directory in the [miden-tutorials](https://github.com/0xPolygonMiden/miden-tutorials/) repository and run this command:

```bash
cd rust-client
cargo run --release --bin unauthenticated_note_transfer
```

### Continue learning

Next tutorial: [How to Use Mappings in Miden Assembly](mappings_in_masm_how_to.md)
