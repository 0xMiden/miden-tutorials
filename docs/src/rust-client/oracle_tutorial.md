# Consuming On-Chain Price Data from the Pragma Oracle

_Using the Pragma oracle to get on chain price data_

## Overview

In this tutorial, we will build a simple “price reader” smart contract that will read Bitcoin price data from the on-chain Pragma oracle.

We will use a script to call the `read_price` function in our "price reader" smart contract, which, in turn, calls the Pragma oracle via foreign procedure invocation (FPI). This tutorial lays the foundation for how you can integrate on-chain price data into your DeFi applications on Miden.

## What we'll cover

- Deploying a smart contract that can read oracle price data
- Using foreign procedure invocation to get real time on-chain price data

## Prerequisites

This tutorial assumes you have a basic understanding of Miden assembly, have completed the previous tutorials on using the Rust client, and have completed the tutorial on foreign procedure invocation.

To quickly get up to speed with Miden assembly (MASM), please play around with running Miden programs in the [Miden playground](https://0xMiden.github.io/examples/).

## Step 1: Initialize your repository

Create a new Rust repository for your Miden project and navigate to it with the following command:

```bash
cargo new miden-defi-app
cd miden-defi-app
```

Add the following dependencies to your `Cargo.toml` file:

```toml
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
miden-client-tools = "0.2.0"
```

### Step 1: Set up your `src/main.rs` file

Copy and paste the following code into your `src/main.rs` file:

```rust
use rand::RngCore;
use std::{fs, path::Path};

use miden_client::{
    account::{
        component::AccountComponent, AccountBuilder, AccountId, AccountStorageMode, AccountType,
        StorageSlot,
    },
    rpc::{
        domain::account::{AccountStorageRequirements, StorageMapKey},
        Endpoint,
    },
    transaction::{
        ForeignAccount, TransactionKernel, TransactionRequestBuilder, TransactionScript,
    },
    Client, ClientError, Felt, Word, ZERO,
};

use miden_client_tools::{create_library, instantiate_client};

/// Import the oracle + its publishers and return the ForeignAccount list
/// Due to Pragma's decentralized oracle architecture, we need to get the
/// list of all data publisher accounts to read price from via a nested FPI call
pub async fn get_oracle_foreign_accounts(
    client: &mut Client,
    oracle_account_id: AccountId,
    trading_pair: u64,
) -> Result<Vec<ForeignAccount>, ClientError> {
    client.import_account_by_id(oracle_account_id).await?;

    let oracle_record = client
        .get_account(oracle_account_id)
        .await
        .expect("RPC failed")
        .expect("oracle account not found");

    let storage = oracle_record.account().storage();
    let publisher_count = storage.get_item(1).unwrap()[0].as_int();

    let publisher_ids: Vec<AccountId> = (1..publisher_count.saturating_sub(1))
        .map(|i| {
            let digest = storage.get_item(2 + i as u8).unwrap();
            let words: Word = digest.into();
            AccountId::new_unchecked([words[3], words[2]])
        })
        .collect();

    let mut foreign_accounts = Vec::with_capacity(publisher_ids.len() + 1);

    for pid in publisher_ids {
        client.import_account_by_id(pid).await?;

        foreign_accounts.push(ForeignAccount::public(
            pid,
            AccountStorageRequirements::new([(
                1u8,
                &[StorageMapKey::from([
                    ZERO,
                    ZERO,
                    ZERO,
                    Felt::new(trading_pair),
                ])],
            )]),
        )?);
    }

    foreign_accounts.push(ForeignAccount::public(
        oracle_account_id,
        AccountStorageRequirements::default(),
    )?);

    Ok(foreign_accounts)
}

#[tokio::main]
async fn main() -> Result<(), ClientError> {
    // -------------------------------------------------------------------------
    // Initialize Client
    // -------------------------------------------------------------------------
    let endpoint = Endpoint::testnet();
    let mut client = instantiate_client(endpoint, None).await.unwrap();

    println!("Latest block: {}", client.sync_state().await?.block_num);

    // -------------------------------------------------------------------------
    // Get all foreign accounts for oracle data
    // -------------------------------------------------------------------------
    let (_, oracle_account_id) =
        AccountId::from_bech32("mtst1qq0zffxzdykm7qqqqdt24cc2du5ghx99").unwrap();
    let btc_usd_pair_id = 120195681;
    let foreign_accounts: Vec<ForeignAccount> =
        get_oracle_foreign_accounts(&mut client, oracle_account_id, btc_usd_pair_id).await?;

    println!(
        "Oracle accountId prefix: {:?} suffix: {:?}",
        oracle_account_id.prefix(),
        oracle_account_id.suffix()
    );

    // -------------------------------------------------------------------------
    // Create Oracle Reader contract
    // -------------------------------------------------------------------------
    let contract_code =
        fs::read_to_string(Path::new("../masm/accounts/oracle_reader.masm")).unwrap();

    let assembler = TransactionKernel::assembler().with_debug_mode(true);

    // Load and compile the NoAuth component
    let no_auth_code = fs::read_to_string(Path::new("../masm/accounts/auth/no_auth.masm")).unwrap();
    let no_auth_component =
        AccountComponent::compile(no_auth_code, assembler.clone(), vec![StorageSlot::empty_value()])
            .unwrap()
            .with_supports_all_types();

    let contract_component = AccountComponent::compile(
        contract_code.clone(),
        assembler,
        vec![StorageSlot::empty_value()],
    )
    .unwrap()
    .with_supports_all_types();

    let mut seed = [0_u8; 32];
    client.rng().fill_bytes(&mut seed);

    let (oracle_reader_contract, seed) = AccountBuilder::new(seed)
        .account_type(AccountType::RegularAccountImmutableCode)
        .storage_mode(AccountStorageMode::Public)
        .with_component(contract_component.clone())
        .with_auth_component(no_auth_component)
        .build()
        .unwrap();

    client
        .add_account(&oracle_reader_contract.clone(), Some(seed), false)
        .await
        .unwrap();

    // -------------------------------------------------------------------------
    // Build the script that calls our `get_price` procedure
    // -------------------------------------------------------------------------
    let script_path = Path::new("../masm/scripts/oracle_reader_script.masm");
    let script_code = fs::read_to_string(script_path).unwrap();

    let assembler = TransactionKernel::assembler().with_debug_mode(true);
    let library_path = "external_contract::oracle_reader";
    let account_component_lib = create_library(contract_code, library_path).unwrap();

    let tx_script = TransactionScript::compile(
        script_code,
        [],
        assembler.with_library(&account_component_lib).unwrap(),
    )
    .unwrap();

    let tx_increment_request = TransactionRequestBuilder::new()
        .foreign_accounts(foreign_accounts)
        .custom_script(tx_script)
        .build()
        .unwrap();

    let tx_result = client
        .new_transaction(oracle_reader_contract.id(), tx_increment_request)
        .await
        .unwrap();

    let tx_id = tx_result.executed_transaction().id();
    println!(
        "View transaction on MidenScan: https://testnet.midenscan.com/tx/{:?}",
        tx_id
    );
    // -------------------------------------------------------------------------
    //  Submit transaction to the network
    // -------------------------------------------------------------------------
    let _ = client.submit_transaction(tx_result).await;

    client.sync_state().await.unwrap();

    Ok(())
}
```

_Don't run this code just yet, we still need to create our smart contract that queries the oracle_

In the code above, we specified the Pragma oracle account id `0x4f67e78643022e00000220d8997e33` and the BTC/USD pair `120195681`. The `get_oracle_foreign_accounts` function returns all of the `ForeignAccounts` that you will need to execute the transaction to get the price data from the oracle. Since Pragma's oracle depends on multiple publishers, this function queries all of the publisher account ids required to make a successful FPI call.

To learn more about Pragma's oracle architecture, you can look at the source code here: https://github.com/astraly-labs/pragma-miden

## Step 2: Build the price reader smart contract and script

Just like in previous tutorials, for better code organization we will separate the Miden assembly code from our Rust code.

Create a directory named `masm` at the **root** of your `miden-counter-contract` directory. This will contain our contract and script masm code.

Initialize the `masm` directory:

```bash
mkdir -p masm/accounts masm/scripts
```

This will create:

```
masm/
├── accounts/
└── scripts/
```

### Oracle price reader smart contract

Below is our oracle price reader contract. It has a a single exported procedure: `get_price`

The import `miden::tx` contains the `tx::execute_foreign_procedure` which we will use to read the price from the oracle contract.

#### Here's a breakdown of what the `get_price` procedure does:

1. Pushes `0.0.0.120195681` onto the stack, representing the BTC/USD pair in the Pragma oracle.
2. Pushes `0xb86237a8c9cd35acfef457e47282cc4da43df676df410c988eab93095d8fb3b9` onto the stack which is the procedure root of the `get_median` procedure in the oracle.
3. Pushes `599064613630720.5721796415433354752` onto the stack which is the oracle id prefix and suffix.
4. Calls `tx::execute_foreign_procedure` which calls the `get_median` procedure via foreign procedure invocation.

Inside of the `masm/accounts/` directory, create the `oracle_reader.masm` file:

```masm
use.miden::tx

# Fetches the current price from the `get_median`
# procedure from the Pragma oracle
# => []
export.get_price
    push.0.0.0.120195681
    # => [PAIR]

    # This is the procedure root of the `get_median` procedure
    push.0xb86237a8c9cd35acfef457e47282cc4da43df676df410c988eab93095d8fb3b9
    # => [GET_MEDIAN_HASH, PAIR]

    push.939716883672832.2172042075194638080
    # => [oracle_id_prefix, oracle_id_suffix, GET_MEDIAN_HASH, PAIR]

    exec.tx::execute_foreign_procedure
    # => [price]

    debug.stack
    # => [price]

    dropw dropw
end
```

**Note**: _It's a good habit to add comments above each line of MASM code with the expected stack state. This improves readability and helps with debugging._

### Create the script which calls the `get_price` procedure

This is a Miden assembly script that will call the `get_price` procedure during the transaction.

Inside of the `masm/scripts/` directory, create the `oracle_reader_script.masm` file:

```masm
use.external_contract::oracle_reader

begin
    exec.oracle_reader::get_price
end
```

## Step 3: Run the program

Run the following command to execute src/main.rs:

```
cargo run --release
```

The output of our program will look something like this:

```
cleared sqlite store: ./store.sqlite3
Latest block: 648397
Oracle accountId prefix: V0(AccountIdPrefixV0 { prefix: 5721796415433354752 }) suffix: 599064613630720
Stack state before step 8766:
├──  0: 82655190335
├──  1: 0
├──  2: 0
├──  3: 0
├──  4: 0
├──  5: 0
├──  6: 0
├──  7: 0
├──  8: 0
├──  9: 0
├── 10: 0
├── 11: 0
├── 12: 0
├── 13: 0
├── 14: 0
├── 15: 0
├── 16: 0
├── 17: 0
├── 18: 0
└── 19: 0

View transaction on MidenScan: https://testnet.midenscan.com/tx/0xc8951190564d5c3ac59fe99d8911f8c17f5b59ba542e2eb860413898902f3722
```

As you can see, at the top of the stack is the price returned from the Pragma oracle. The price is returned with 6 decimal places. Currently Pragma only publishes the `BTC/USD` price feed on testnet.

### Running the example

To run the full example, navigate to the `rust-client` directory in the [miden-tutorials](https://github.com/0xMiden/miden-tutorials/) repository and run this command:

```bash
cd rust-client
cargo run --release --bin oracle_data_query
```

### Continue learning

Next tutorial: [How to Use Unauthenticated Notes](./unauthenticated_note_how_to.md)
