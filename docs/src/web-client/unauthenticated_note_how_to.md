---
title: "How to Use Unauthenticated Notes"
sidebar_position: 6
---

# How to Use Unauthenticated Notes

_Using unauthenticated notes for optimistic note consumption with the Miden WebClient_

## Overview

In this tutorial, we will explore how to leverage unauthenticated notes on Miden to settle transactions faster than the blocktime using the Miden WebClient. Unauthenticated notes are essentially UTXOs that have not yet been fully committed into a block. This feature allows the notes to be created and consumed within the same batch during [batch production](https://0xmiden.github.io/miden-docs/imported/miden-base/src/blockchain.html#batch-production).

When using unauthenticated notes, both the creation and consumption of notes can happen within the same batch, enabling faster-than-blocktime settlement. This is particularly powerful for applications requiring high-frequency transactions or optimistic settlement patterns.

We construct a chain of transactions using the unauthenticated notes method on the transaction builder. Unauthenticated notes are also referred to as "erasable notes". We also demonstrate how a note can be created and consumed, highlighting the ability to transfer notes between client instances for asset transfers that can be settled between parties faster than the blocktime.

For example, our demo creates a chain of unauthenticated note transactions:

```markdown
Alice ➡ Wallet 1 ➡ Wallet 2 ➡ Wallet 3 ➡ Wallet 4 ➡ Wallet 5
```

## What we'll cover

- **Introduction to Unauthenticated Notes:** Understand what unauthenticated notes are and how they differ from standard notes.
- **WebClient Setup:** Configure the Miden WebClient for browser-based transactions.
- **P2ID Note Creation:** Learn how to create Pay-to-ID notes for targeted transfers.
- **Performance Insights:** Observe how unauthenticated notes can reduce transaction times dramatically.

## Prerequisites

- Node `v20` or greater
- Familiarity with TypeScript
- `pnpm`

This tutorial assumes you have a basic understanding of Miden assembly. To quickly get up to speed with Miden assembly (MASM), please play around with running basic Miden assembly programs in the [Miden playground](https://0xmiden.github.io/examples/).

## Step-by-step process

1. **Next.js Project Setup:**
   - Create a new Next.js application with TypeScript.
   - Install the Miden WebClient SDK.

2. **WebClient Initialization:**
   - Set up the WebClient to connect with the Miden testnet.
   - Configure a delegated prover for improved performance.

3. **Account Creation:**
   - Create wallet accounts for Alice and multiple transfer recipients.
   - Deploy a fungible faucet for token minting.

4. **Initial Token Setup:**
   - Mint tokens from the faucet to Alice's account.
   - Consume the minted tokens to prepare for transfers.

5. **Unauthenticated Note Transfer Chain:**
   - Create P2ID (Pay-to-ID) notes for each transfer in the chain.
   - Use unauthenticated input notes to consume notes faster than blocktime.
   - Measure and observe the performance benefits.

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
   pnpm install @demox-labs/miden-sdk@0.11.1
   ```

**NOTE!**: Be sure to add the `--webpack` command to your `package.json` when running the `dev script`. The dev script should look like this:

`package.json`

```json
  "scripts": {
    "dev": "next dev --webpack",
    ...
  }
```

## Step 2: Edit the `app/page.tsx` file

Add the following code to the `app/page.tsx` file. This code defines the main page of our web application:

```tsx
"use client";
import { useState } from "react";
import { unauthenticatedNoteTransfer } from "../lib/unauthenticatedNoteTransfer";

export default function Home() {
  const [isTransferring, setIsTransferring] = useState(false);

  const handleUnauthenticatedNoteTransfer = async () => {
    setIsTransferring(true);
    await unauthenticatedNoteTransfer();
    setIsTransferring(false);
  };

  return (
    <main className="min-h-screen flex items-center justify-center bg-gradient-to-br from-gray-900 via-gray-800 to-black text-slate-800 dark:text-slate-100">
      <div className="text-center">
        <h1 className="text-4xl font-semibold mb-4">Miden Web App</h1>
        <p className="mb-6">Open your browser console to see WebClient logs.</p>

        <div className="max-w-sm w-full bg-gray-800/20 border border-gray-600 rounded-2xl p-6 mx-auto flex flex-col gap-4">
          <button
            onClick={handleUnauthenticatedNoteTransfer}
            className="w-full px-6 py-3 text-lg cursor-pointer bg-transparent border-2 border-orange-600 text-white rounded-lg transition-all hover:bg-orange-600 hover:text-white"
          >
            {isTransferring
              ? "Working..."
              : "Tutorial #4: Unauthenticated Note Transfer"}
          </button>
        </div>
      </div>
    </main>
  );
}
```

## Step 3: Create the Unauthenticated Note Transfer Implementation

Create the file `lib/unauthenticatedNoteTransfer.ts` and add the following code:

```bash
mkdir -p lib
touch lib/unauthenticatedNoteTransfer.ts
```

Copy and paste the following code into the `lib/unauthenticatedNoteTransfer.ts` file:

```ts
/**
 * Demonstrates unauthenticated note transfer chain using a delegated prover on the Miden Network
 * Creates a chain of P2ID (Pay to ID) notes: Alice → wallet 1 → wallet 2 → wallet 3 → wallet 4
 *
 * @throws {Error} If the function cannot be executed in a browser environment
 */
export async function unauthenticatedNoteTransfer(): Promise<void> {
  // Ensure this runs only in a browser context
  if (typeof window === "undefined") return console.warn("Run in browser");

  const {
    WebClient,
    AccountStorageMode,
    AccountId,
    NoteType,
    TransactionProver,
    NoteInputs,
    Note,
    NoteAssets,
    NoteRecipient,
    Word,
    OutputNotesArray,
    NoteExecutionHint,
    NoteTag,
    NoteExecutionMode,
    NoteMetadata,
    FeltArray,
    Felt,
    FungibleAsset,
    NoteAndArgsArray,
    NoteAndArgs,
    TransactionRequestBuilder,
    OutputNote,
  } = await import("@demox-labs/miden-sdk");

  const client = await WebClient.createClient("https://rpc.testnet.miden.io");
  const prover = TransactionProver.newRemoteProver(
    "https://tx-prover.testnet.miden.io",
  );

  console.log("Latest block:", (await client.syncState()).blockNum());

  // ── Creating new account ──────────────────────────────────────────────────────
  console.log("Creating accounts");

  console.log("Creating account for Alice…");
  const alice = await client.newWallet(AccountStorageMode.public(), true);
  console.log("Alice accout ID:", alice.id().toString());

  let wallets = [];
  for (let i = 0; i < 5; i++) {
    let wallet = await client.newWallet(AccountStorageMode.public(), true);
    wallets.push(wallet);
    console.log("wallet ", i.toString(), wallet.id().toString());
  }

  // ── Creating new faucet ──────────────────────────────────────────────────────
  const faucet = await client.newFaucet(
    AccountStorageMode.public(),
    false,
    "MID",
    8,
    BigInt(1_000_000),
  );
  console.log("Faucet ID:", faucet.id().toString());

  // ── mint 10 000 MID to Alice ──────────────────────────────────────────────────────
  await client.submitTransaction(
    await client.newTransaction(
      faucet.id(),
      client.newMintTransactionRequest(
        alice.id(),
        faucet.id(),
        NoteType.Public,
        BigInt(10_000),
      ),
    ),
    prover,
  );

  console.log("Waiting for settlement");
  await new Promise((r) => setTimeout(r, 7_000));
  await client.syncState();

  // ── Consume the freshly minted note ──────────────────────────────────────────────
  const noteIds = (await client.getConsumableNotes(alice.id())).map((rec) =>
    rec.inputNoteRecord().id().toString(),
  );

  await client.submitTransaction(
    await client.newTransaction(
      alice.id(),
      client.newConsumeTransactionRequest(noteIds),
    ),
    prover,
  );
  await client.syncState();

  // ── Create unauthenticated note transfer chain ─────────────────────────────────────────────
  // Alice → wallet 1 → wallet 2 → wallet 3 → wallet 4
  for (let i = 0; i < wallets.length; i++) {
    console.log(`\nUnauthenticated tx ${i + 1}`);

    // Determine sender and receiver for this iteration
    const sender = i === 0 ? alice : wallets[i - 1];
    const receiver = wallets[i];

    console.log("Sender:", sender.id().toString());
    console.log("Receiver:", receiver.id().toString());

    const assets = new NoteAssets([new FungibleAsset(faucet.id(), BigInt(50))]);
    const receiverAccountId = AccountId.fromHex(receiver.id().toString());

    let p2idNote = Note.createP2IDNote(
      sender.id(),
      receiverAccountId,
      assets,
      NoteType.Public,
      new Felt(BigInt(0)), // aux value
    );

    let outputP2ID = OutputNote.full(p2idNote);

    console.log("Creating P2ID note...");
    let transaction = await client.newTransaction(
      sender.id(),
      new TransactionRequestBuilder()
        .withOwnOutputNotes(new OutputNotesArray([outputP2ID]))
        .build(),
    );
    await client.submitTransaction(transaction, prover);

    console.log("Consuming P2ID note...");

    let noteIdAndArgs = new NoteAndArgs(p2idNote, null);

    let consumeRequest = new TransactionRequestBuilder()
      .withUnauthenticatedInputNotes(new NoteAndArgsArray([noteIdAndArgs]))
      .build();

    let txExecutionResult = await client.newTransaction(
      receiver.id(),
      consumeRequest,
    );

    await client.submitTransaction(txExecutionResult, prover);

    const txId = txExecutionResult
      .executedTransaction()
      .id()
      .toHex()
      .toString();

    console.log(
      `Consumed Note Tx on MidenScan: https://testnet.midenscan.com/tx/${txId}`,
    );
  }

  console.log("Asset transfer chain completed ✅");
}
```

## Key Concepts: Unauthenticated Notes

### What are Unauthenticated Notes?

Unauthenticated notes are a powerful feature that allows notes to be:

- **Created and consumed in the same block**
- **Transferred faster than blocktime**
- **Used for optimistic transactions**

### Performance Benefits

By using unauthenticated notes, we can:

- Skip waiting for block confirmation between note creation and consumption
- Create transaction chains that execute within a single block
- Achieve sub-blocktime settlement for certain use cases

### Use Cases

Unauthenticated notes are ideal for:

- **High-frequency trading applications**
- **Payment channels**
- **Micropayment systems**
- **Any scenario requiring fast settlement**

## Running the Example

To run the unauthenticated note transfer example:

```bash
cd miden-web-app
pnpm install
pnpm run dev
```

Open [http://localhost:3000](http://localhost:3000) in your browser, click the **"Tutorial #4: Unauthenticated Note Transfer"** button, and check the browser console for detailed logs.

### Expected Output

You should see output similar to this in the browser console:

```
🚀 Starting unauthenticated note transfer demo
Latest block: 2247

[STEP 1] Creating wallet accounts
Creating account for Alice…
Alice account ID: 0xd70b2072c6495d100000869a8bacf2
Wallet 1 ID: 0x2d7e506fb88dde200000a1386efec8
Wallet 2 ID: 0x1a8c3f4e2b9d5a600000c7e9b2f4d8
...

[STEP 2] Deploying a fungible faucet
Faucet ID: 0x8f2a1b7c3e5d9f800000d4a6c8e2b5

[STEP 3] Minting tokens to Alice
Waiting for settlement...

[STEP 4] Consuming minted tokens

[STEP 5] Creating unauthenticated note transfer chain
Transfer chain: Alice → Wallet 1 → Wallet 2 → Wallet 3 → Wallet 4 → Wallet 5

--- Unauthenticated transfer 1 ---
Sender: 0xd70b2072c6495d100000869a8bacf2
Receiver: 0x2d7e506fb88dde200000a1386efec8
Creating P2ID note...
Consuming P2ID note with unauthenticated input...
✅ Consumed Note Tx on MidenScan: https://testnet.midenscan.com/tx/0x1234...
⏱️  Iteration 1 completed in: 2341ms

...

🏁 Total execution time for unauthenticated note transfers: 11847ms
✅ Asset transfer chain completed successfully!

[FINAL BALANCES]
Alice balance: 9750 MID
Wallet 1 balance: 0 MID
Wallet 2 balance: 0 MID
Wallet 3 balance: 0 MID
Wallet 4 balance: 0 MID
Wallet 5 balance: 50 MID
```

## Conclusion

Unauthenticated notes on Miden offer a powerful mechanism for achieving faster asset settlements by allowing notes to be both created and consumed within the same block. In this guide, we walked through:

- **Setting up the Miden WebClient** with delegated proving for optimal performance
- **Creating P2ID Notes** for targeted asset transfers between specific accounts
- **Building Transaction Chains** using unauthenticated input notes for sub-blocktime settlement
- **Performance Observations** demonstrating how unauthenticated notes enable faster-than-blocktime transfers

By following this guide, you should now have a clear understanding of how to build and deploy high-performance transactions using unauthenticated notes on Miden with the WebClient. Unauthenticated notes are the ideal approach for applications like central limit order books (CLOBs) or other DeFi platforms where transaction speed is critical.

### Resetting the `MidenClientDB`

The Miden webclient stores account and note data in the browser. If you get errors such as "Failed to build MMR", then you should reset the Miden webclient store. When switching between Miden networks such as from localhost to testnet be sure to reset the browser store. To clear the account and node data in the browser, paste this code snippet into the browser console:

```javascript
(async () => {
  const dbs = await indexedDB.databases();
  for (const db of dbs) {
    await indexedDB.deleteDatabase(db.name);
    console.log(`Deleted database: ${db.name}`);
  }
  console.log("All databases deleted.");
})();
```

### Running the Full Example

To run a full working example navigate to the `web-client` directory in the [miden-tutorials](https://github.com/0xMiden/miden-tutorials/) repository and run the web application example:

```bash
cd web-client
pnpm install
pnpm run start
```

### Continue learning

Next tutorial: [Creating Multiple Notes](creating_multiple_notes_tutorial.md)
