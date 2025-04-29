# Miden Node Setup Tutorial

To run the Miden tutorial examples, you will need to set up a test environment and connect to a Miden node.

There are two ways to connect to a Miden node:

1. Run the Miden node locally
2. Connect to the Miden testnet

## Running the Miden node locally

### Step 1: Clone the miden-node repository

In a terminal window, clone the miden-tutorials repository and navigate to the root of the repository using this command:

```bash
git clone git@github.com:0xPolygonMiden/miden-node.git
cd miden-node
```

### Step 2: Install the Miden node

Next, install the miden-node crate using this command:

```bash
make install-node
```

### Step 3: Initializing the node

To start the node, we first need to generate the genesis file. To do so, navigate to the `/node` directory and create the genesis file using this command:

```bash
miden-node store dump-genesis > genesis.toml
mkdir -p data accounts
miden-node bundled bootstrap \
  --data-directory data \
  --accounts-directory accounts \
  --config genesis.toml
```

Expected output:

```
2025-04-16T18:05:30.049129Z  INFO miden_node::commands::store: bin/node/src/commands/store.rs:145: Generating account, index: 0, total: 1
```

### Step 4: Starting the node

To start the node run this command:

```bash
miden-node bundled start \
  --data-directory data \
  --rpc.url http://0.0.0.0:57123
```

Expected output:

```
2025-01-17T12:14:55.432445Z  INFO try_build_batches: miden-block-producer: /Users/username/.cargo/registry/src/index.crates.io-6f17d22bba15001f/miden-node-block-producer-0.6.0/src/txqueue/mod.rs:85: close, time.busy: 8.88µs, time.idle: 103µs
2025-01-17T12:14:57.433162Z  INFO try_build_batches: miden-block-producer: /Users/username/.cargo/registry/src/index.crates.io-6f17d22bba15001f/miden-node-block-producer-0.6.0/src/txqueue/mod.rs:85: new
2025-01-17T12:14:57.433256Z  INFO try_build_batches: miden-block-producer: /Users/username/.cargo/registry/src/index.crates.io-6f17d22bba15001f/miden-node-block-producer-0.6.0/src/txqueue/mod.rs:85: close, time.busy: 6.46µs, time.idle: 94.0µs
```

Congratulations, you now have a Miden node running locally. Now we can start creating a testing environment for building applications on Miden!

The endpoint of the Miden node running locally is:

```
http://localhost:57123
```

### Resetting the node

_If you need to reset the local state of the node run this command:_

```bash
rm -r data
```

After resetting the state of the node, follow steps 3 and 4 again.

## Connecting to the Miden testnet

To run the tutorial examples using the Miden testnet, use this endpoint:

```bash
https://rpc.testnet.miden.io:443
```
