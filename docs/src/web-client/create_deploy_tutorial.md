# Creating Accounts and Deploying Faucets

_Using the Miden WebClient in TypeScript to create accounts and deploy faucets_

## Overview

In this tutorial, we will create a basic web application that interacts with Miden using the Miden WebClient.

Our web application will create a Miden account for _Alice_ and then deploy a fungible faucet. In the next section we will mint tokens from the faucet to fund her account, and then send the tokens from Alice's account to other Miden accounts.

## What we'll cover

- Understanding the difference between public and private accounts & notes
- Instantiating the Miden client
- Creating new accounts (public or private)
- Deploying a faucet to fund an account

## Prerequisites

In this tutorial we use [pnpm](https://pnpm.io/installation) which is a drop in replacement for npm.

## Public vs. private accounts & notes

Before we dive into the coding, let's clarify the concepts of public and private accounts and notes on Miden:

- Public accounts: The account's data and code are stored on-chain and are openly visible, including its assets.
- Private accounts: The account's state and logic are off-chain, only known to its owner.
- Public notes: The note's state is visible to anyone - perfect for scenarios where transparency is desired.
- Private notes: The note's state is stored off-chain, you will need to share the note data with the relevant parties (via email or Telegram) for them to be able to consume the note.

Note: _The term "account" can be used interchangeably with the term "smart contract" since account abstraction on Miden is handled natively._

_It is useful to think of notes on Miden as "cryptographic cashier's checks" that allow users to send tokens. If the note is private, the note transfer is only known to the sender and receiver._

## Step 1: Initialize your repository

Create a new React TypeScript repository for your Miden web application, navigate to it, and install the Miden WebClient using this command:

```bash
pnpm create vite miden-webapp --template react-ts
```

Navigate to the new repository:

```bash
cd miden-webapp
```

Install dependencies:

```bash
pnpm install
```

Install the Miden WebClient SDK:

```bash
pnpm i @demox-labs/miden-sdk@0.8.1
```

Save this as your `vite.config.ts` file:

```ts
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  build: {
    target: "esnext",
    commonjsOptions: {
      include: [/node_modules/],
      transformMixedEsModules: true,
    },
    rollupOptions: {
      external: ["@demox-labs/miden-sdk"],
      output: {
        format: "es",
      },
    },
  },
  optimizeDeps: {
    include: ["@demox-labs/miden-sdk"],
    esbuildOptions: {
      target: "esnext",
      supported: {
        "top-level-await": true,
      },
    },
  },
});
```

**Note**: _ensure you are using Node version `v20.12.0`_

## Step 2: Initialize the client

Before we can interact with the Miden network, we need to instantiate the WebClient. In this step, we specify two parameters:

- **RPC endpoint** - The URL of the Miden node to which we connect.
- **Delegated Prover Endpoint (optional)** – The URL of the delegated prover which the client can connect to.

### Create a `webClient.ts` file:

To instantiate the WebClient, pass in the endpoint of the Miden node. You can also instantiate the client with a delegated prover to speed up the proof generation time, however, in this example we will be instantiating the WebClient only with the endpoint of the Miden node since we will be handling proof generation locally within the browser.

Since we will be handling proof generation in the computationally constrained environment of the browser, it will be slower than proof generation handled by the Rust client. Currently, the Miden WebClient is thread-blocking when not used within a web worker.

Example of instantiating the WebClient:

```ts
const nodeEndpoint = "https://rpc.testnet.miden.io:443";
const client = await WebClient.create_client(nodeEndpoint);
```

In the `src/` directory create a file named `webClient.ts` and paste the following into it:

```ts
// src/webClient.ts
import {
  WebClient,
  AccountStorageMode,
  AccountId,
  NoteType,
} from "@demox-labs/miden-sdk";

const nodeEndpoint = "https://rpc.testnet.miden.io:443";

export async function webClient(): Promise<void> {
  try {
    // 1. Create client
    const client = await WebClient.createClient(nodeEndpoint);

    // 2. Sync and log block
    const state = await client.syncState();
    console.log("Latest block number:", state.blockNum());
  } catch (error) {
    console.error("Error", error);
    throw error;
  }
}
```

### Edit your `App.tsx` file:

Set this as your `App.tsx` file.

```ts
// src/App.tsx
import { useState } from "react";
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

      {!clientStarted && <button onClick={handleClick}>Start WebClient</button>}
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

Now open the browser console. Click the "Start the WebClient" button. Then in the console, you should see something like:

```
Latest block number: 123
```

## Step 3: Creating a wallet

Now that we've initialized the WebClient, we can create a wallet for Alice.

To create a wallet for Alice using the Miden WebClient, we specify the account type by specifying if the account code is mutable or immutable and whether the account is public or private. A mutable wallet means you can change the account code after deployment.

A wallet on Miden is simply an account with standardized code.

In the example below we create a mutable public account for Alice.

Our `src/webClient.ts` file should now look something like this:

```ts
// src/webClient.ts
import {
  WebClient,
  AccountStorageMode,
  AccountId,
  NoteType,
} from "@demox-labs/miden-sdk";

const nodeEndpoint = "https://rpc.testnet.miden.io:443";

export async function webClient(): Promise<void> {
  try {
    // 1. Create client
    const client = await WebClient.createClient(nodeEndpoint);

    // 2. Sync and log block
    const state = await client.syncState();
    console.log("Latest block number:", state.blockNum());

    // 3. Create Alice account (public, updatable)
    console.log("Creating account for Alice");
    const aliceAccount = await client.newWallet(
      AccountStorageMode.public(), // account type
      true, // mutability
    );
    const aliceIdHex = aliceAccount.id().toString();
    console.log("Alice's account ID:", aliceIdHex);

    await client.syncState();
  } catch (error) {
    console.error("Error:", error);
    throw error;
  }
}
```

## Step 4: Deploying a fungible faucet

For Alice to receive testnet assets, we first need to deploy a faucet. A faucet account on Miden mints fungible tokens.

We'll create a public faucet with a token symbol, decimals, and a max supply. We will use this faucet to mint tokens to Alice's account in the next section.

Add this snippet to the end of the `webClient()` function:

```ts
// 4. Create faucet
console.log("Creating faucet...");
const faucetAccount = await client.newFaucet(
  AccountStorageMode.public(),
  false,
  "MID",
  8,
  BigInt(1_000_000),
);
const faucetIdHex = faucetAccount.id().toString();
console.log("Faucet account ID:", faucetIdHex);
```

_When tokens are minted from this faucet, each token batch is represented as a "note" (UTXO). You can think of a Miden Note as a cryptographic cashier's check that has certain spend conditions attached to it._

## Summary

Our new `src/webClient.ts` file should look something like this:

```ts
// src/webClient.ts
import {
  WebClient,
  AccountStorageMode,
  AccountId,
  NoteType,
} from "@demox-labs/miden-sdk";

const nodeEndpoint = "https://rpc.testnet.miden.io:443";

export async function webClient(): Promise<void> {
  try {
    // 1. Create client
    const client = await WebClient.createClient(nodeEndpoint);

    // 2. Sync and log block
    const state = await client.syncState();
    console.log("Latest block number:", state.blockNum());

    // 3. Create Alice account (public, updatable)
    console.log("Creating account for Alice");
    const aliceAccount = await client.newWallet(
      AccountStorageMode.public(),
      true,
    );
    const aliceIdHex = aliceAccount.id().toString();
    console.log("Alice's account ID:", aliceIdHex);

    // 4. Create faucet
    console.log("Creating faucet...");
    const faucetAccount = await client.newFaucet(
      AccountStorageMode.public(),
      false,
      "MID",
      8,
      BigInt(1_000_000),
    );
    const faucetIdHex = faucetAccount.id().toString();
    console.log("Faucet account ID:", faucetIdHex);

    await client.syncState();
    console.log("Tokens sent.");
  } catch (error) {
    console.error("Error:", error);
    throw error;
  }
}
```

Let's run the `src/main.rs` program again:

```bash
pnpm run dev
```

The output will look like this:

```
Latest block number: 2247
Alice's account ID: 0xd70b2072c6495d100000869a8bacf2
Faucet account ID: 0x2d7e506fb88dde200000a1386efec8
```

In this section, we explained how to instantiate the Miden client, create a wallet, and deploy a faucet.

In the next section we will cover how to mint tokens from the faucet, consume notes, and send tokens to other accounts.

### Running the example

To run a full working example navigate to the `web-client` directory in the [miden-tutorials](https://github.com/0xPolygonMiden/miden-tutorials/) repository and run the web application example:

```bash
cd web-client
pnpm i
pnpm run dev
```
