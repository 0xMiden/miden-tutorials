---
title: 'Creating Accounts and Deploying Faucets'
sidebar_position: 2
---

_Using the Miden WebClient in TypeScript to create accounts and deploy faucets_

## Overview

In this tutorial, we'll build a simple Next.js application that demonstrates the fundamentals of interacting with the Miden blockchain using the WebClient SDK. We'll walk through creating a Miden account for Alice and deploying a fungible faucet contract that can mint tokens. This sets the foundation for more complex operations like issuing assets and transferring them between accounts.

## What we'll cover

- Understanding the difference between public and private accounts & notes
- Instantiating the Miden client
- Creating new accounts (public or private)
- Deploying a faucet to fund an account

## Prerequisites

- Node `v20` or greater
- Familiarity with TypeScript
- `yarn`

## Public vs. private accounts & notes

Before we dive into code, a quick refresher:

- **Public accounts**: The account's data and code are stored on-chain and are openly visible, including its assets.
- **Private accounts**: The account's state and logic are off-chain, only known to its owner.
- **Public notes**: The note's state is visible to anyone - perfect for scenarios where transparency is desired.
- **Private notes**: The note's state is stored off-chain, you will need to share the note data with the relevant parties (via email or Telegram) for them to be able to consume the note.

> **Important**: In Miden, "accounts" and "smart contracts" can be used interchangeably due to native account abstraction. Every account is programmable and can contain custom logic.

It is useful to think of notes on Miden as "cryptographic cashier's checks" that allow users to send tokens. If the note is private, the note transfer is only known to the sender and receiver.

## Step 1: Initialize your Next.js project

1. Create a new Next.js app with TypeScript:

   ```bash
   yarn create next-app@latest miden-web-app --typescript
   ```

   Hit enter for all terminal prompts.

2. Change into the project directory:

   ```bash
   cd miden-web-app
   ```

3. Install the Miden WebClient SDK:
   ```bash
   yarn add @miden-sdk/miden-sdk@0.13.0
   ```

**NOTE!**: Be sure to add the `--webpack` command to your `package.json` when running the `dev script`. The dev script should look like this:

`package.json`

```json
  "scripts": {
    "dev": "next dev --webpack",
    ...
  }
```

## Step 2: Set up the WebClient

The WebClient is your gateway to interact with the Miden blockchain. It handles state synchronization, transaction creation, and proof generation. Let's set it up.

### Create `lib/createMintConsume.ts`

First, we'll create a separate file for our blockchain logic. In the project root, create a folder `lib/` and inside it `createMintConsume.ts`:

```bash
mkdir -p lib
touch lib/createMintConsume.ts
```

```ts
// lib/createMintConsume.ts
export async function createMintConsume(): Promise<void> {
  if (typeof window === 'undefined') {
    console.warn('webClient() can only run in the browser');
    return;
  }

  // dynamic import → only in the browser, so WASM is loaded client‑side
  const { WebClient, AccountStorageMode, AccountId, NoteType } =
    await import('@miden-sdk/miden-sdk');

  // Connect to Miden testnet RPC endpoint
  const nodeEndpoint = 'https://rpc.testnet.miden.io';
  const client = await WebClient.createClient(nodeEndpoint);

  // 1. Sync with the latest blockchain state
  // This fetches the latest block header and state commitments
  const state = await client.syncState();
  console.log('Latest block number:', state.blockNum());

  // At this point, your client is connected and synchronized
  // Ready to create accounts and deploy contracts!
}
```

> Since we will be handling proof generation in the browser, it will be slower than proof generation handled by the Rust client. Check out the [tutorial on delegated proving](./creating_multiple_notes_tutorial.md#what-is-delegated-proving) to speed up proof generation in the browser.

## Step 3: Create the User Interface

Now let's create a simple UI that will trigger our blockchain interactions. We'll replace the default Next.js page with a button that calls our `createMintConsume()` function.

Edit `app/page.tsx` to call `createMintConsume()` on a button click:

```tsx
'use client';
import { useState } from 'react';
import { createMintConsume } from '../lib/createMintConsume';

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
              ? 'Working...'
              : 'Tutorial #1: Create a wallet and deploy a faucet'}
          </button>
        </div>
      </div>
    </main>
  );
}
```

## Step 4: Create Alice's Wallet Account

Now we'll create Alice's account. Let's create a **public** account so we can easily track her transactions.

Back in `lib/createMintConsume.ts`, extend the `createMintConsume()` function:

<!-- prettier-ignore-start -->
```ts
// lib/createMintConsume.ts
export async function createMintConsume(): Promise<void> {
  if (typeof window === 'undefined') {
    console.warn('webClient() can only run in the browser');
    return;
  }

  const { WebClient, AccountStorageMode, AuthScheme } = await import(
    "@miden-sdk/miden-sdk"
  );

  const nodeEndpoint = 'https://rpc.testnet.miden.io';
  const client = await WebClient.createClient(nodeEndpoint);

  // 1. Sync with the latest blockchain state
  const state = await client.syncState();
  console.log('Latest block number:', state.blockNum());

  // 2. Create Alice's account
  console.log('Creating account for Alice…');
  const alice = await client.newWallet(
    AccountStorageMode.public(),  // Public: account state is visible on-chain
    true,                         // Mutable: account code can be upgraded later
    AuthScheme.AuthRpoFalcon512   // Auth Scheme: RPO Falcon 512
  );
  console.log('Alice ID:', alice.id().toString());
}
```
<!-- prettier-ignore-end -->

## Step 5: Deploy a Fungible Faucet

A faucet in Miden is a special type of account that can mint new tokens. Think of it as your own token factory. Let's deploy one that will create our custom "MID" tokens.

Add this code after creating Alice's account:

<!-- prettier-ignore-start -->
```ts
// 3. Deploy a fungible faucet
// A faucet is an account that can mint new tokens
console.log('Creating faucet…');
const faucetAccount = await client.newFaucet(
  AccountStorageMode.public(),  // Public: faucet operations are transparent
  false,                        // Immutable: faucet rules cannot be changed
  "MID",                        // Token symbol (like ETH, BTC, etc.)
  8,                            // Decimals (8 means 1 MID = 100,000,000 base units)
  BigInt(1_000_000),            // Max supply: total tokens that can ever be minted
  AuthScheme.AuthRpoFalcon512   // Auth Scheme: RPO Falcon 512
);
console.log('Faucet account ID:', faucetAccount.id().toString());

console.log('Setup complete.');
```
<!-- prettier-ignore-end -->

### Understanding Faucet Parameters:

- **Storage Mode**: We use `public()` so anyone can verify the faucet's minting operations
- **Mutability**: Set to `false` to ensure the faucet rules can't be changed after deployment
- **Token Symbol**: A short identifier for your token (e.g., "MID", "USDC", "DAI")
- **Decimals**: Determines the smallest unit of your token. With 8 decimals, 1 MID = 10^8 base units
- **Max Supply**: The maximum number of tokens that can ever exist

> **Note**: When tokens are minted from a faucet, they're created as "notes" - Miden's version of UTXOs. Each note contains tokens and can have specific spending conditions.

## Summary

In this tutorial, we've successfully:

1. Set up a Next.js application with the Miden WebClient SDK
2. Connected to the Miden testnet
3. Created a wallet account for Alice
4. Deployed a fungible faucet that can mint custom tokens

Your final `lib/createMintConsume.ts` should look like:

```ts
// lib/createMintConsume.ts
export async function createMintConsume(): Promise<void> {
  if (typeof window === 'undefined') {
    console.warn('webClient() can only run in the browser');
    return;
  }

  // dynamic import → only in the browser, so WASM is loaded client‑side
  const { WebClient, AccountStorageMode, AuthScheme, NoteType, Address } =
    await import('@miden-sdk/miden-sdk');

  const nodeEndpoint = 'https://rpc.testnet.miden.io';
  const client = await WebClient.createClient(nodeEndpoint);

  // 1. Sync with the latest blockchain state
  const state = await client.syncState();
  console.log('Latest block number:', state.blockNum());

  // 2. Create Alice's account
  console.log('Creating account for Alice…');
  const alice = await client.newWallet(
    AccountStorageMode.public(),
    true,
    AuthScheme.AuthRpoFalcon512,
  );
  console.log('Alice ID:', alice.id().toString());

  // 3. Deploy a fungible faucet
  console.log('Creating faucet…');
  const faucet = await client.newFaucet(
    AccountStorageMode.public(),
    false,
    'MID',
    8,
    BigInt(1_000_000),
    AuthScheme.AuthRpoFalcon512,
  );
  console.log('Faucet ID:', faucet.id().toString());

  console.log('Setup complete.');
}
```

### Running the example

```bash
cd miden-web-app
yarn install
yarn dev
```

Open [http://localhost:3000](http://localhost:3000) in your browser, click **Tutorial #1: Create a wallet and deploy a faucet**, and check the browser console (F12 or right-click → Inspect → Console):

```
Latest block: 2247
Creating account for Alice…
Alice ID: 0xd70b2072c6495d100000869a8bacf2
Creating faucet…
Faucet ID: 0x2d7e506fb88dde200000a1386efec8
Setup complete.
```

## What's Next?

Now that you have:

- A wallet account for Alice that can hold tokens
- A faucet that can mint new MID tokens

In the next tutorial, we'll:

1. Mint tokens from the faucet to Alice's account
2. Consume notes
3. Transfer tokens between accounts
