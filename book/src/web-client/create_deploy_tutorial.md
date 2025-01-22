# Creating Accounts and Deploying Faucets 

*Using the Miden web client in TypeScript to create accounts and deploy faucets*

## Overview
In this tutorial, we will create a basic web application that interacts with Miden using the Miden web client. 

Our web application will create a Miden account for *Alice* and then deploy a fungible faucet. In the next section we will mint tokens from the faucet to fund her account, and then send the tokens from Alice's account to other Miden accounts.

## What we'll cover
* Understanding the difference between public and private accounts & notes
* Instantiating the Miden client
* Creating new accounts (public or private)
* Deploying a faucet to fund an account

## Prerequisites
To begin, make sure you have a miden-node running locally in a separate terminal window. To get the Miden node running locally, you can follow the instructions on the [Miden Node Setup](./miden_node_setup_tutorial.md) page.

## Public vs. private accounts & notes
Before we dive into the coding, let's clarify the concepts of public and private accounts and notes on Miden:

* Public accounts: The account's data and code are stored on-chain and are openly visible, including its assets.
* Private accounts: The account's state and logic are off-chain, only known to its owner.
* Public notes: The note's state is visible to anyone - perfect for scenarios where transparency is desired.
* Private notes: The note's state is stored off-chain, you will need to share the note data with the relevant parties (via email or Telegram) for them to be able to consume the note.

Note: *The term "account" can be used interchangeably with the term "smart contract" since account abstraction on Miden is handled natively.*

*It is useful to think of notes on Miden as "cryptographic cashier's checks" that allow users to send tokens. If the note is private, the note transfer is only known to the sender and receiver.*

## Step 1: Initialize your repository
Create a new React TypeScript repository for your Miden web application, navigate to it, and install the Miden web client using this command:
```bash
pnpm create vite miden-app --template react-ts

cd miden-app
pnpm install

pnpm i @demox-labs/miden-sdk@0.6.1
```

Save this as your `vite.config.ts` file:
```ts
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  build: {
    target: 'esnext',
  },
  optimizeDeps: {
    exclude: ['@demox-labs/miden-sdk'], // Exclude the SDK from optimization
  },
});
```

Note: ensure you are using Node version `v20.12.0`

## Step 2: Initialize the client
Before we can interact with the Miden network, we need to instantiate the web client. In this step, we specify two parameters:

* **RPC endpoint** - The URL of the Miden node to which we connect.
* **Delegated Prover Endpoint (optional)** – The URL of the delegated prover which the client can connect to.

### Create a `webClient.ts` file:
To instantiate the web client, pass in the endpoint of the Miden node. You can also instantiate the client with a delegated prover to speed up the proof generation time, however, in this example we will be instantiating the web client only with the endpoint of the Miden node since we will be handling proof generation locally within the browser.

Since we will be handling proof generation in the computationally constrained environment of the browser, it will be slower than proof generation handled by the Rust client. Currently, the Miden web client is thread-blocking when not used within a web worker.

Example of instantiating the web client with a delegated prover:
```ts
const nodeEndpoint = "http://localhost:57291";
const delegatedProver = 'http://18.118.151.210:8082'

let client = new WebClient();
await client.create_client(nodeEndpoint, delegatedProver);
```

In the `src/` directory create a file named `webClient.ts` and paste the following into it:
```ts
// src/webClient.ts
import { WebClient } from "@demox-labs/miden-sdk";

const nodeEndpoint = "http://localhost:57291";

export async function webClient(): Promise<void> {
  try {
    let client = new WebClient();
    await client.create_client(nodeEndpoint);

    let state = await client.sync_state();
    console.log("Latest block number: ", state.block_num());
  } catch (error) {
    console.error("Error", error);
    throw error;
  }
}
```

### Edit your `App.tsx` file:
Set this as your `App.tsx` file.

```ts
// src/App
import React, { useState } from "react";
import "./App.css";
import { webClient } from "./webClient";

function App() {
  const [clientStarted, setClientStarted] = useState(false);

  const handleClick = () => {
    webClient();
    setClientStarted(true);
  };

  return (
    <div className="App">
      <h1>Miden Web App</h1>

      <p>Open the console to view logs</p>

      {!clientStarted && (
        <button onClick={handleClick}>Start Web Client</button>
      )}
    </div>
  );
}

export default App;
```

### Starting the frontend:
```
pnpm run dev
```

Open the frontend at: 
```
http://localhost:5173/
```

Now open the browser console. In the console, you should see something like:
```
Latest block number: 123
```

## Step 3: Creating a wallet
Now that we've initialized the web client, we can create a wallet for Alice.

To create a wallet for Alice using the Miden web client, we specify the account type by specifying if the account code is mutable or immutable and whether the account is public or private. A mutable wallet means you can change the account code after deployment.

 A wallet on Miden is simply an account with standardized code.

In the example below we create a mutable public account for Alice. 

Add this snippet to the `webClient()` function:
```ts
const aliceAccount = await client.new_wallet(
  AccountStorageMode.public(), // account type
  true                         // mutability
);
const aliceIdHex = aliceAccount.id().to_string();
console.log("Alice's account ID:", aliceIdHex);

await client.sync_state();
```

## Step 4: Deploying a fungible faucet
For Alice to receive testnet assets, we first need to deploy a faucet. A faucet account on Miden mints fungible tokens.

We'll create a public faucet with a token symbol, decimals, and a max supply. We will use this faucet to mint tokens to Alice's account in the next section.

Add this snippet to the end of the `webClient()` function:
```ts
const faucetAccount = await client.new_faucet(
  AccountStorageMode.public(), // account type
  false,                       // is fungible
  "MID",                       // symbol
  8,                           // decimals
  BigInt(1_000_000)            // max supply
);
const faucetIdHex = faucetAccount.id().to_string();
console.log("Faucet account ID:", faucetIdHex);

await client.sync_state();
```

*When tokens are minted from this faucet, each token batch is represented as a "note" (UTXO). You can think of a Miden Note as a cryptographic cashier's check that has certain spend conditions attached to it.*

## Summary
Our new `src/webClient.ts` file should look something like this:

```ts
import { WebClient, AccountStorageMode } from "@demox-labs/miden-sdk";

const nodeEndpoint = "http://localhost:57291";

export async function webClient(): Promise<void> {
  try {
    // 1. Create client
    const client = new WebClient();
    await client.create_client(nodeEndpoint);

    // 2. Sync and log block
    const state = await client.sync_state();
    console.log("Latest block number:", state.block_num());

    // 3. Create Alice account (public, updatable)
    const aliceAccount = await client.new_wallet(
      AccountStorageMode.public(),  // account ty[e]
      true                          // mutability
    );
    const aliceIdHex = aliceAccount.id().to_string();
    console.log("Alice's account ID:", aliceIdHex);

    // 4. Create faucet
    console.log("Creating faucet...");
    const faucetAccount = await client.new_faucet(
      AccountStorageMode.public(), // account type
      false,                       // is fungible
      "MID",                       // symbol
      8,                           // decimals
      BigInt(1_000_000)            // max supply
    );
    const faucetIdHex = faucetAccount.id().to_string();
    console.log("Faucet account ID:", faucetIdHex);

    await client.sync_state();
  } catch (error) {
    console.error("Error", error);
    throw error;
  }
}
```

Let's run the `src/main.rs` program again:
```bash
pnpm run dev
```

The output will look like this:
```bash
Latest block number: 607494
Alice's account id:  0x157d84660075ffcf
Faucet account id: 0x2d7969e6125856d0
```

In this section, we explained how to instantiate the Miden client, create a wallet, and deploy a faucet.

In the next section we will cover how to mint tokens from the faucet, consume notes, and send tokens to other accounts. 

### Running the Example
To run a full working example navigate to the `web-client` directory in the [miden-tutorials](https://github.com/0xPolygonMiden/miden-tutorials/) repository and run the web application example:

```bash
cd web-client
pnpm i
pnpm run dev
```