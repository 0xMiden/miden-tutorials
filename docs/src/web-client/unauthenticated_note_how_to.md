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
   pnpm install @demox-labs/miden-sdk@0.10.1
   ```

**NOTE!**: Be sure to remove the `--turbopack` command from your `package.json` when running the `dev script`. The dev script should look like this:

`package.json`

```json
  "scripts": {
    "dev": "next dev",
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
// lib/unauthenticatedNoteTransfer.ts

/**
 * P2ID (Pay to ID) Note Script for Miden Network
 * Enables creating notes that can be received by specific account IDs
 */
const P2ID_NOTE_SCRIPT = `
use.miden::account
use.miden::account_id
use.miden::note

# ERRORS
# =================================================================================================

const.ERR_P2ID_WRONG_NUMBER_OF_INPUTS="P2ID note expects exactly 2 note inputs"

const.ERR_P2ID_TARGET_ACCT_MISMATCH="P2ID's target account address and transaction address do not match"

#! Pay-to-ID script: adds all assets from the note to the account, assuming ID of the account
#! matches target account ID specified by the note inputs.
#!
#! Requires that the account exposes:
#! - miden::contracts::wallets::basic::receive_asset procedure.
#!
#! Inputs:  []
#! Outputs: []
#!
#! Note inputs are assumed to be as follows:
#! - target_account_id is the ID of the account for which the note is intended.
#!
#! Panics if:
#! - Account does not expose miden::contracts::wallets::basic::receive_asset procedure.
#! - Account ID of executing account is not equal to the Account ID specified via note inputs.
#! - The same non-fungible asset already exists in the account.
#! - Adding a fungible asset would result in amount overflow, i.e., the total amount would be
#!   greater than 2^63.
begin
    # store the note inputs to memory starting at address 0
    padw push.0 exec.note::get_inputs
    # => [num_inputs, inputs_ptr, EMPTY_WORD]

    # make sure the number of inputs is 2
    eq.2 assert.err=ERR_P2ID_WRONG_NUMBER_OF_INPUTS
    # => [inputs_ptr, EMPTY_WORD]

    # read the target account ID from the note inputs
    mem_loadw drop drop
    # => [target_account_id_prefix, target_account_id_suffix]

    exec.account::get_id
    # => [account_id_prefix, account_id_suffix, target_account_id_prefix, target_account_id_suffix, ...]

    # ensure account_id = target_account_id, fails otherwise
    exec.account_id::is_equal assert.err=ERR_P2ID_TARGET_ACCT_MISMATCH
    # => []

    exec.note::add_note_assets_to_account
    # => []
end
`;

/**
 * Demonstrates unauthenticated note transfer chain using a delegated prover on the Miden Network
 * Creates a chain of P2ID (Pay to ID) notes: Alice → wallet 1 → wallet 2 → wallet 3 → wallet 4
 */
export async function unauthenticatedNoteTransfer(): Promise<void> {
  // Ensure this runs only in a browser context
  if (typeof window === "undefined") {
    console.warn("unauthenticatedNoteTransfer() can only run in the browser");
    return;
  }

  // Dynamic import for browser-only execution
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

  console.log("🚀 Starting unauthenticated note transfer demo");

  // Initialize WebClient and delegated prover
  const client = await WebClient.createClient(
    "https://rpc.testnet.miden.io:443",
  );
  const prover = TransactionProver.newRemoteProver(
    "https://tx-prover.testnet.miden.io",
  );

  const syncState = await client.syncState();
  console.log("Latest block:", syncState.blockNum());

  //------------------------------------------------------------
  // STEP 1: Create wallet accounts
  //------------------------------------------------------------
  console.log("\n[STEP 1] Creating wallet accounts");

  console.log("Creating account for Alice…");
  const alice = await client.newWallet(AccountStorageMode.public(), true);
  console.log("Alice account ID:", alice.id().toString());

  // Create multiple wallets for the transfer chain
  let wallets = [];
  const numberOfWallets = 5;
  for (let i = 0; i < numberOfWallets; i++) {
    let wallet = await client.newWallet(AccountStorageMode.public(), true);
    wallets.push(wallet);
    console.log(`Wallet ${i + 1} ID:`, wallet.id().toString());
  }

  //------------------------------------------------------------
  // STEP 2: Deploy a fungible faucet
  //------------------------------------------------------------
  console.log("\n[STEP 2] Deploying a fungible faucet");

  const faucet = await client.newFaucet(
    AccountStorageMode.public(),
    false,
    "MID",
    8,
    BigInt(1_000_000),
  );
  console.log("Faucet ID:", faucet.id().toString());

  //------------------------------------------------------------
  // STEP 3: Mint tokens to Alice
  //------------------------------------------------------------
  console.log("\n[STEP 3] Minting tokens to Alice");

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

  console.log("Waiting for settlement...");
  await new Promise((resolve) => setTimeout(resolve, 7_000));
  await client.syncState();

  //------------------------------------------------------------
  // STEP 4: Consume the minted tokens
  //------------------------------------------------------------
  console.log("\n[STEP 4] Consuming minted tokens");

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

  // Compile the P2ID note script
  const script = client.compileNoteScript(P2ID_NOTE_SCRIPT);

  //------------------------------------------------------------
  // STEP 5: Create unauthenticated note transfer chain
  //------------------------------------------------------------
  console.log("\n[STEP 5] Creating unauthenticated note transfer chain");
  console.log(
    "Transfer chain: Alice → Wallet 1 → Wallet 2 → Wallet 3 → Wallet 4 → Wallet 5",
  );

  const startTime = Date.now();

  // Create the transfer chain: Alice → wallet 1 → wallet 2 → wallet 3 → wallet 4 → wallet 5
  for (let i = 0; i < wallets.length; i++) {
    const iterationStart = Date.now();
    console.log(`\n--- Unauthenticated transfer ${i + 1} ---`);

    // Determine sender and receiver for this iteration
    const sender = i === 0 ? alice : wallets[i - 1];
    const receiver = wallets[i];

    console.log("Sender:", sender.id().toString());
    console.log("Receiver:", receiver.id().toString());

    // Create assets for the note (50 MID tokens)
    const assets = new NoteAssets([new FungibleAsset(faucet.id(), BigInt(50))]);

    // Set up note metadata
    const metadata = new NoteMetadata(
      sender.id(),
      NoteType.Public,
      NoteTag.fromAccountId(sender.id(), NoteExecutionMode.newLocal()),
      NoteExecutionHint.always(),
    );

    // Generate a random serial number for the note
    let serialNumber = Word.newFromFelts([
      new Felt(BigInt(Math.floor(Math.random() * 0x1_0000_0000))),
      new Felt(BigInt(Math.floor(Math.random() * 0x1_0000_0000))),
      new Felt(BigInt(Math.floor(Math.random() * 0x1_0000_0000))),
      new Felt(BigInt(Math.floor(Math.random() * 0x1_0000_0000))),
    ]);

    // Set up note inputs with receiver account ID
    const receiverAcct = AccountId.fromHex(receiver.id().toString());
    const inputs = new NoteInputs(
      new FeltArray([receiverAcct.suffix(), receiverAcct.prefix()]),
    );

    // Create the P2ID note
    let p2idNote = new Note(
      assets,
      metadata,
      new NoteRecipient(serialNumber, script, inputs),
    );

    let outputP2ID = OutputNote.full(p2idNote);

    console.log("Creating P2ID note...");

    // Create and submit the transaction to create the note
    let createTransaction = await client.newTransaction(
      sender.id(),
      new TransactionRequestBuilder()
        .withOwnOutputNotes(new OutputNotesArray([outputP2ID]))
        .build(),
    );
    await client.submitTransaction(createTransaction, prover);

    console.log("Consuming P2ID note with unauthenticated input...");

    // Create the unauthenticated consumption transaction
    let noteAndArgs = new NoteAndArgs(p2idNote, null);

    let consumeRequest = new TransactionRequestBuilder()
      .withUnauthenticatedInputNotes(new NoteAndArgsArray([noteAndArgs]))
      .build();

    let consumeTransaction = await client.newTransaction(
      receiver.id(),
      consumeRequest,
    );

    await client.submitTransaction(consumeTransaction, prover);

    const txId = consumeTransaction
      .executedTransaction()
      .id()
      .toHex()
      .toString();

    console.log(
      `✅ Consumed Note Tx on MidenScan: https://testnet.midenscan.com/tx/${txId}`,
    );

    const iterationTime = Date.now() - iterationStart;
    console.log(`⏱️  Iteration ${i + 1} completed in: ${iterationTime}ms`);
  }

  const totalTime = Date.now() - startTime;
  console.log(
    `\n🏁 Total execution time for unauthenticated note transfers: ${totalTime}ms`,
  );
  console.log("✅ Asset transfer chain completed successfully!");

  // Final sync and balance check
  await client.syncState();

  console.log("\n[FINAL BALANCES]");
  const aliceBalance = (await client.getAccount(alice.id()))
    ?.vault()
    .getBalance(faucet.id());

  console.log(`Alice balance: ${aliceBalance} MID`);

  for (let i = 0; i < wallets.length; i++) {
    const walletBalance = (await client.getAccount(wallets[i].id()))
      ?.vault()
      .getBalance(faucet.id());

    console.log(`Wallet ${i + 1} balance: ${walletBalance} MID`);
  }
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
