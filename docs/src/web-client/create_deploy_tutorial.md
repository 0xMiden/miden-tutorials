## Creating Accounts and Deploying Faucets

_Using the Miden WebClient in TypeScript to create accounts and deploy faucets_

## Overview

In this tutorial, we’ll build a simple Next.js application that interacts with Miden using the Miden WebClient. Our web application will create a Miden account for Alice and then deploy a fungible faucet. In the next section we will mint tokens from the faucet to fund her account, and then send the tokens from Alice's account to other Miden accounts.

## What we'll cover

- Understanding the difference between public and private accounts & notes
- Instantiating the Miden client
- Creating new accounts (public or private)
- Deploying a faucet to fund an account

## Prerequisites

- Node `v20` or greater
- Familiarity with TypeScript
- `pnpm`

## Public vs. private accounts & notes

Before we dive into code, a quick refresher:

- **Public accounts**: The account's data and code are stored on-chain and are openly visible, including its assets.
- **Private accounts**: The account's state and logic are off-chain, only known to its owner.
- **Public notes**: The note's state is visible to anyone - perfect for scenarios where transparency is desired.
- **Private notes**: The note's state is stored off-chain, you will need to share the note data with the relevant parties (via email or Telegram) for them to be able to consume the note.

Note: The term "account" can be used interchangeably with the term "smart contract" since account abstraction on Miden is handled natively.

It is useful to think of notes on Miden as "cryptographic cashier's checks" that allow users to send tokens. If the note is private, the note transfer is only known to the sender and receiver.

## Step 1: Initialize your Next.js project

1. Create a new Next.js app with TypeScript:

   ```bash
   npx create-next-app@latest miden-web-app --typescript
   ```

   Hit enter for all terminal prompts.

2. Change into the project directory:

   ```bash
   cd miden-web-app
   ```

3. Install the Miden WebClient SDK:
   ```bash
   pnpm install @demox-labs/miden-sdk@0.9.2
   ```

**NOTE!**: Be sure to remove the `--turbopack` command from your `package.json` when running the `dev script`. The dev script should look like this:

`package.json`

```json
  "scripts": {
    "dev": "next dev",
    ...
  }
```

## Step 2: Instantiate the WebClient

### Create `lib/createMintConsume.ts`

In the project root, create a folder `lib/` and inside it `createMintConsume.ts`:

```bash
mkdir -p lib
touch lib/createMintConsume.ts
```

```ts
// lib/createMintConsume.ts
export async function createMintConsume(): Promise<void> {
  if (typeof window === "undefined") {
    console.warn("webClient() can only run in the browser");
    return;
  }

  // dynamic import → only in the browser, so WASM is loaded client‑side
  const { WebClient, AccountStorageMode, AccountId, NoteType } = await import(
    "@demox-labs/miden-sdk"
  );

  const nodeEndpoint = "https://rpc.testnet.miden.io:443";
  const client = await WebClient.createClient(nodeEndpoint);

  // 1. Sync and log block
  const state = await client.syncState();
  console.log("Latest block number:", state.blockNum());
}
```

> To instantiate the WebClient, pass in the endpoint of the Miden node.

> Since we will be handling proof generation in the browser, it will be slower than proof generation handled by the Rust client. Check out the tutorial on how to use delegated proving in the browser to speed up proof generation.

## Step 3: Edit the `app/page.tsx` file:

Edit `app/page.tsx` to call `webClient()` on a button click:

```tsx
"use client";
import { useState } from "react";
import { createMintConsume } from "../lib/createMintConsume";

export default function Home() {
  const [isCreatingNotes, setIsCreatingNotes] = useState(false);

  const handleCreateMintConsume = async () => {
    setIsCreatingNotes(true);
    await createMintConsume();
    setIsCreatingNotes(false);
  };

  return (
    <main className="min-h-screen flex items-center justify-center bg-gradient-to-br from-gray-900 via-gray-800 to-black text-slate-800 dark:text-slate-100">
      <div className="text-center">
        <h1 className="text-4xl font-semibold mb-4">Miden Web App</h1>
        <p className="mb-6">Open your browser console to see WebClient logs.</p>

        <div className="max-w-sm w-full bg-gray-800/20 border border-gray-600 rounded-2xl p-6 mx-auto flex flex-col gap-4">
          <button
            onClick={handleCreateMintConsume}
            className="w-full px-6 py-3 text-lg cursor-pointer bg-transparent border-2 border-orange-600 text-white rounded-lg transition-all hover:bg-orange-600 hover:text-white"
          >
            {isCreatingNotes
              ? "Working..."
              : "Tutorial #1: Create, Mint, Consume Notes"}
          </button>
        </div>
      </div>
    </main>
  );
}
```

## Step 4: Create a wallet for Alice

Back in `lib/createMintConsume.ts`, extend `createMintConsume()`:

```ts
// lib/createMintConsume.ts
export async function createMintConsume(): Promise<void> {
  if (typeof window === "undefined") {
    console.warn("webClient() can only run in the browser");
    return;
  }

  // dynamic import → only in the browser, so WASM is loaded client‑side
  const { WebClient, AccountStorageMode } = await import(
    "@demox-labs/miden-sdk"
  );

  const nodeEndpoint = "https://rpc.testnet.miden.io:443";
  const client = await WebClient.createClient(nodeEndpoint);

  // 1. Sync and log block
  const state = await client.syncState();
  console.log("Latest block number:", state.blockNum());

  // 2. Create Alice’s account
  console.log("Creating account for Alice…");
  const alice = await client.newWallet(AccountStorageMode.public(), true);
  console.log("Alice ID:", alice.id().toString());

  await client.syncState();
}
```

## Step 5: Deploy a fungible faucet

Append this to the end of `webClient()`:

```ts
// 4. Deploy faucet
console.log("Creating faucet…");
const faucetAccount = await client.newFaucet(
  AccountStorageMode.public(), // public faucet
  false, // immutable
  "MID", // token symbol
  8, // decimals
  BigInt(1_000_000), // max supply
);
console.log("Faucet account ID:", faucetAccount.id().toString());

await client.syncState();
console.log("Setup complete.");
```

> Every batch minted is a “note”—think of it as a UTXO with spend conditions.

## Summary

Your final `lib/createMintConsume.ts` should look like:

```ts
// lib/createMintConsume.ts
export async function createMintConsume(): Promise<void> {
  if (typeof window === "undefined") {
    console.warn("webClient() can only run in the browser");
    return;
  }

  // dynamic import → only in the browser, so WASM is loaded client‑side
  const { WebClient, AccountStorageMode } = await import(
    "@demox-labs/miden-sdk"
  );

  const nodeEndpoint = "https://rpc.testnet.miden.io:443";
  const client = await WebClient.createClient(nodeEndpoint);

  // 1. Sync and log block
  const state = await client.syncState();
  console.log("Latest block number:", state.blockNum());

  // 2. Create Alice’s account
  console.log("Creating account for Alice…");
  const alice = await client.newWallet(AccountStorageMode.public(), true);
  console.log("Alice ID:", alice.id().toString());

  // 3. Deploy faucet
  console.log("Creating faucet…");
  const faucet = await client.newFaucet(
    AccountStorageMode.public(),
    false,
    "MID",
    8,
    BigInt(1_000_000),
  );
  console.log("Faucet ID:", faucet.id().toString());

  await client.syncState();
  console.log("Setup complete.");
}
```

### Running the example

```bash
cd miden-web-app
npm i
npm run dev
```

Open [http://localhost:3000](http://localhost:3000) in your browser, click **Start WebClient**, and check the console:

```
Latest block: 2247
Alice ID: 0xd70b2072c6495d100000869a8bacf2
Faucet ID: 0x2d7e506fb88dde200000a1386efec8
Setup complete.
```
