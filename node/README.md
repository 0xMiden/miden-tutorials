# Running the Miden node locally

### Setup

1. **Install Miden node:**

```
cargo install miden-node --locked
```

2. **In the root of the miden-tutorials directory, run the following:**

```
miden-node make-genesis \
  --inputs-path  node/config/genesis.toml \
  --output-path node/storage/genesis.dat

cd node/storage
miden-node start \
--config node/config/miden-node.toml \
node
```

### Reset

Use the following command to reset the Miden node:

```
rm -rf rust-client/store.sqlite3
rm -rf node/storage/accounts
rm -rf node/storage/blocks
```
