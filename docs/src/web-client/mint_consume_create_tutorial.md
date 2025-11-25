---
title: 'Mint, Consume, and Create Notes'
sidebar_position: 3
---

_Using the Miden WebClient in TypeScript to mint, consume, and transfer assets_

## Overview

In the previous tutorial, we set up the foundation - creating Alice's wallet and deploying a faucet. Now we'll put these to use by minting and transferring assets.

## What we'll cover

- Minting assets from a faucet
- Consuming notes to fund an account
- Sending tokens to other users

## Prerequisites

This tutorial builds directly on the previous one. Make sure you have:

- Completed the "Creating Accounts and Deploying Faucets" tutorial
- Your Next.js app with the Miden WebClient set up

## Understanding Notes in Miden

Before we start coding, it's important to understand **notes**:

- Minting a note from a faucet does not automatically add the tokens to your account balance. It creates a note addressed to you.
- You must **consume** a note to add its tokens to your account balance.
- Until consumed, tokens exist in the note but aren't in your account yet.

## Step 1: Mint tokens from the faucet

Let's mint some tokens for Alice. When we mint from a faucet, it creates a note containing the specified amount of tokens targeted to Alice's account.

Add this to the end of your `createMintConsume` function in `lib/createMintConsume.ts`:

<!-- prettier-ignore-start -->

```ts
// 4. Mint tokens from the faucet to Alice
await client.syncState();

console.log("Minting tokens to Alice...");
const mintTxRequest = client.newMintTransactionRequest(
  alice.id(),           // Target account (who receives the tokens)
  faucet.id(),          // Faucet account (who mints the tokens)
  NoteType.Public,      // Note visibility (public = onchain)
  BigInt(1000),         // Amount to mint (in base units)
);

await client.submitNewTransaction(faucet.id(), mintTxRequest);

// Wait for the transaction to be processed
console.log("Waiting 10 seconds for transaction confirmation...");
await new Promise((resolve) => setTimeout(resolve, 10000));
await client.syncState();
```

<!-- prettier-ignore-end -->

### What's happening here?

1. **newMintTransactionRequest**: Creates a request to mint tokens to Alice. Note that this is only possible to submit transactions on the faucets' behalf if the user controls the faucet (i.e. its keys are stored in the client).
2. **newTransaction**: Locally executes and proves the transaction.
3. **submitTransaction**: Sends the transaction to the network.
4. Wait 10 seconds for the transaction to be included in a block.

## Step 2: Find consumable notes

After minting, Alice has a note waiting for her but the tokens aren't in her account yet.
To identify notes that are ready to consume, the Miden WebClient provides the `getConsumableNotes` function:

```ts
// 5. Find notes available for consumption
const consumableNotes = await client.getConsumableNotes(alice.id());
console.log(`Found ${consumableNotes.length} note(s) to consume`);

const noteIds = consumableNotes.map((note) =>
  note.inputNoteRecord().id().toString(),
);
console.log('Consumable note IDs:', noteIds);
```

## Step 3: Consume notes in a single transaction

Now let's consume the notes to add the tokens to Alice's account balance:

```ts
// 6. Consume the notes to add tokens to Alice's balance
console.log('Consuming minted notes...');
const consumeTxRequest = client.newConsumeTransactionRequest(mintedNoteIds);

await client.submitNewTransaction(alice.id(), consumeTxRequest);

await client.syncState();
console.log('Notes consumed.');
```

## Step 4: Sending tokens to other accounts

After consuming the notes, Alice has tokens in her wallet. Now, she wants to send tokens to her friends. She has two options: create a separate transaction for each transfer or batch multiple notes in a single transaction.

_The standard asset transfer note on Miden is the P2ID note (Pay-to-Id). There is also the P2IDE (Pay-to-Id Extended) variant which allows for both timelocking the note (target can only spend the note after a certain block height) and for the note to be reclaimable (the creator of the note can reclaim the note after a certain block height)._

Now that Alice has tokens in her account, she can send some to Bob:

<!-- prettier-ignore-start -->
```ts
// Add this import at the top of the file
import { NoteType } from "@demox-labs/miden-sdk";
// ...

// 7. Send tokens from Alice to Bob
const bobAccountId = "0x599a54603f0cf9000000ed7a11e379";
console.log("Sending tokens to Bob's account...");
  
const sendTxRequest = client.newSendTransactionRequest(
  alice.id(),                      // Sender account
  AccountId.fromHex(bobAccountId), // Recipient account
  faucet.id(),                     // Asset ID (faucet that created the tokens)
  NoteType.Public,                 // Note visibility
  BigInt(100),                     // Amount to send
);

await client.submitNewTransaction(alice.id(), sendTxRequest);

console.log('Tokens sent successfully!');
```

<!-- prettier-ignore-end -->

### Understanding P2ID notes

The transaction creates a **P2ID (Pay-to-ID)** note:

- It's the standard way to transfer assets in Miden
- The note is "locked" to Bob's account ID, i.e. only Bob can consume this note to receive the tokens
- Public notes are visible onchain; private notes would need to be shared offchain (e.g. via a private channel)

## Summary

Here's the complete `lib/createMintConsume.ts` file:

```ts
// lib/createMintConsume.ts
export async function createMintConsume(): Promise<void> {
  if (typeof window === 'undefined') {
    console.warn('webClient() can only run in the browser');
    return;
  }

  // dynamic import → only in the browser, so WASM is loaded client‑side
  const { WebClient, AccountStorageMode, AccountId, NoteType } = await import(
    '@demox-labs/miden-sdk'
  );

  const nodeEndpoint = 'https://rpc.testnet.miden.io';
  const client = await WebClient.createClient(nodeEndpoint);

  // 1. Sync and log block
  const state = await client.syncState();
  console.log('Latest block number:', state.blockNum());

  // 2. Create Alice’s account
  console.log('Creating account for Alice…');
  const alice = await client.newWallet(AccountStorageMode.public(), true, 0);
  console.log('Alice ID:', alice.id().toString());

  // 3. Deploy faucet
  console.log('Creating faucet…');
  const faucet = await client.newFaucet(
    AccountStorageMode.public(),
    false,
    'MID',
    8,
    BigInt(1_000_000),
    0,
  );
  console.log('Faucet ID:', faucet.id().toString());

  await client.syncState();

  // 4. Mint tokens to Alice
  await client.syncState();

  console.log('Minting 1000 tokens to Alice...');
  const mintTxRequest = client.newMintTransactionRequest(
    alice.id(),
    faucet.id(),
    NoteType.Public,
    BigInt(1000),
  );

  await client.submitNewTransaction(faucet.id(), mintTxRequest);

  console.log('Waiting 10 seconds for transaction confirmation...');
  await new Promise((resolve) => setTimeout(resolve, 10000));
  await client.syncState();

  // 5. Fetch minted notes
  const mintedNotes = await client.getConsumableNotes(alice.id());
  const mintedNoteIds = mintedNotes.map((n) =>
    n.inputNoteRecord().id().toString(),
  );
  console.log('Consumable note IDs:', mintedNoteIds);

  // 6. Consume minted notes
  console.log('Consuming minted notes...');
  const consumeTxRequest = client.newConsumeTransactionRequest(mintedNoteIds);

  await client.submitNewTransaction(alice.id(), consumeTxRequest);

  await client.syncState();
  console.log('Notes consumed.');

  // 7. Send tokens to Bob
  const bobAccountId = '0x599a54603f0cf9000000ed7a11e379';
  console.log("Sending tokens to Bob's account...");
  const sendTxRequest = client.newSendTransactionRequest(
    alice.id(),
    AccountId.fromHex(bobAccountId),
    faucet.id(),
    NoteType.Public,
    BigInt(100),
  );

  await client.submitNewTransaction(alice.id(), sendTxRequest);
  console.log('Tokens sent successfully!');
}
```

Let's run the `lib/createMintConsume.ts` function again. Reload the page and click "Start WebClient".

The output will look like this:

```
Latest block number: 4807
Creating account for Alice...
Alice ID: 0x1a20f4d1321e681000005020e69b1a
Creating faucet...
Faucet ID: 0xaa86a6f05ae40b2000000f26054d5d
Minting 1000 tokens to Alice...
Waiting 10 seconds for transaction confirmation...
Consumable note IDs: ['0x4edbb3d5dbdf694...']
Consuming notes...
Notes consumed.
Sending tokens to Bob's account...
Tokens sent successfully!
```

### Resetting the `MidenClientDB`

The Miden webclient stores account and note data in the browser. To clear the account and note data in the browser, paste this code snippet into the browser console:

```javascript
(async () => {
  const dbs = await indexedDB.databases(); // Get all database names
  for (const db of dbs) {
    await indexedDB.deleteDatabase(db.name);
    console.log(`Deleted database: ${db.name}`);
  }
  console.log('All databases deleted.');
})();
```

## What's next?

You've now learned the complete note lifecycle in Miden:

1. **Minting** - Creating new tokens from a faucet (issued in notes)
2. **Consuming** - Adding tokens from notes to an account
3. **Transferring** - Sending tokens to other accounts

In the next tutorials, we'll explore:

- Creating multiple notes in a single transaction
- Delegated proving
