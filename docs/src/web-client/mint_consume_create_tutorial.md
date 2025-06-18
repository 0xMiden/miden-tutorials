# Mint, Consume, and Create Notes

_Using the Miden WebClient in TypeScript to mint, consume, and create notes_

## Overview

In the previous section, we initialized our repository and covered how to create an account and deploy a faucet. In this section, we will mint tokens from the faucet for _Alice_, consume the newly created notes, and demonstrate how to send assets to other accounts.

## What we'll cover

- Minting assets from a faucet
- Consuming notes to fund an account
- Sending tokens to other users

## Step 1: Minting tokens from the faucet

To mint notes with tokens from the faucet we created, Alice can use the WebClient's `newMintTransactionRequest()` function.

Below is an example of a transaction request minting tokens from the faucet for Alice.

Add this snippet to the end of the `webClient` function in the `src/webClient.ts` file that we created in the previous chapter:

```ts
// 4. Mint tokens to Alice
await client.fetchAndCacheAccountAuthByAccountId(faucet.id());
await client.syncState();

console.log("Minting tokens to Alice...");
let mintTxRequest = client.newMintTransactionRequest(
  alice.id(),
  faucet.id(),
  NoteType.Public,
  BigInt(1000),
);

let txResult = await client.newTransaction(faucet.id(), mintTxRequest);

await client.submitTransaction(txResult);

console.log("Waiting 10 seconds for transaction confirmation...");
await new Promise((resolve) => setTimeout(resolve, 10000));
await client.syncState();
```

## Step 2: Identifying consumable notes

Once Alice has minted a note from the faucet, she will eventually want to spend the tokens that she received in the note created by the mint transaction.

Minting a note from a faucet on Miden means a faucet account creates a new note targeted to the requesting account. The requesting account must consume this note for the assets to appear in their account.

To identify notes that are ready to consume, the Miden WebClient has a useful function `getConsumableNotes`. It is also important to sync the state of the client before calling the `getConsumableNotes` function.

_Tip: If you know the expected number of notes after a transaction, use `await` or a loop condition to verify their availability before calling `getConsumableNotes`. This prevents unnecessary application idling._

#### Identifying which notes are available:

```ts
consumable_notes = await client.getConsumableNotes(accountId);
```

## Step 3: Consuming multiple notes in a single transaction:

Now that we know how to identify notes ready to consume, let's consume the notes created by the faucet in a single transaction. After consuming the notes, Alice's wallet balance will be updated.

The following code snippet identifies and consumes notes in a single transaction.

Add this snippet to the end of the `webClient` function in the `src/webClient.ts` file:

```ts
// 5. Fetch minted notes
const mintedNotes = await client.getConsumableNotes(alice.id());
const mintedNoteIds = mintedNotes.map((n) =>
  n.inputNoteRecord().id().toString(),
);
console.log("Minted note IDs:", mintedNoteIds);

// 6. Consume minted notes
console.log("Consuming minted notes...");
let consumeTxRequest = client.newConsumeTransactionRequest(mintedNoteIds);

let txResult_2 = await client.newTransaction(alice.id(), consumeTxRequest);

await client.submitTransaction(txResult_2);

await client.syncState();
console.log("Notes consumed.");
```

## Step 4: Sending tokens to other accounts

After consuming the notes, Alice has tokens in her wallet. Now, she wants to send tokens to her friends. She has two options: create a separate transaction for each transfer or batch multiple notes in a single transaction.

_The standard asset transfer note on Miden is the P2ID note (Pay to Id). There is also the P2IDR (Pay to Id Reclaimable) variant which allows the creator of the note to reclaim the note after a certain block height._

In our example, Alice will now send 50 tokens to a different account.

### Basic P2ID transfer

Now as an example, Alice will send some tokens to an account in a single transaction.

Add this snippet to the end of your file in the `main()` function:

```ts
// 7. Send tokens to Bob
const bobAccountId = "0x599a54603f0cf9000000ed7a11e379";
console.log("Sending tokens to Bob's account...");
let sendTxRequest = client.newSendTransactionRequest(
  alice.id(),
  AccountId.fromHex(bobAccountId),
  faucet.id(),
  NoteType.Public,
  BigInt(100),
);

let txResult_3 = await client.newTransaction(alice.id(), sendTxRequest);

await client.submitTransaction(txResult_3);
```

## Summary

Your `src/webClient.ts` function should now look like this:

```ts
// lib/webClient.ts
export async function webClient(): Promise<void> {
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

  // 4. Mint tokens to Alice
  await client.fetchAndCacheAccountAuthByAccountId(faucet.id());
  await client.syncState();

  console.log("Minting tokens to Alice...");
  let mintTxRequest = client.newMintTransactionRequest(
    alice.id(),
    faucet.id(),
    NoteType.Public,
    BigInt(1000),
  );

  let txResult = await client.newTransaction(faucet.id(), mintTxRequest);

  await client.submitTransaction(txResult);

  console.log("Waiting 10 seconds for transaction confirmation...");
  await new Promise((resolve) => setTimeout(resolve, 10000));
  await client.syncState();

  // 5. Fetch minted notes
  const mintedNotes = await client.getConsumableNotes(alice.id());
  const mintedNoteIds = mintedNotes.map((n) =>
    n.inputNoteRecord().id().toString(),
  );
  console.log("Minted note IDs:", mintedNoteIds);

  // 6. Consume minted notes
  console.log("Consuming minted notes...");
  let consumeTxRequest = client.newConsumeTransactionRequest(mintedNoteIds);

  let txResult_2 = await client.newTransaction(alice.id(), consumeTxRequest);

  await client.submitTransaction(txResult_2);

  await client.syncState();
  console.log("Notes consumed.");

  // 7. Send tokens to Bob
  const bobAccountId = "0x599a54603f0cf9000000ed7a11e379";
  console.log("Sending tokens to Bob's account...");
  let sendTxRequest = client.newSendTransactionRequest(
    alice.id(),
    AccountId.fromHex(bobAccountId),
    faucet.id(),
    NoteType.Public,
    BigInt(100),
  );

  let txResult_3 = await client.newTransaction(alice.id(), sendTxRequest);

  await client.submitTransaction(txResult_3);
}
```

Let's run the `src/webClient.ts` function again. Reload the page and click "Start WebClient".

The output will look like this:

```
Latest block number: 4807
Alice's account ID: 0x1a20f4d1321e681000005020e69b1a
Creating faucet...
Faucet account ID: 0xaa86a6f05ae40b2000000f26054d5d
Minting tokens to Alice...
Waiting 15 seconds for transaction confirmation...
Minted note IDs: ['0x4edbb3d5dbdf6944f229a4711533114e0602ad48b70cda400993925c61f5bfaa']
Consuming minted notes...
Notes consumed.
Sending tokens to dummy account...
Tokens sent.
```

### Resetting the `MidenClientDB`

The Miden webclient stores account and note data in the browser. To clear the account and node data in the browser, paste this code snippet into the browser console:

```javascript
(async () => {
  const dbs = await indexedDB.databases(); // Get all database names
  for (const db of dbs) {
    await indexedDB.deleteDatabase(db.name);
    console.log(`Deleted database: ${db.name}`);
  }
  console.log("All databases deleted.");
})();
```

### Running the example

To run a full working example navigate to the `web-client` directory in the [miden-tutorials](https://github.com/0xMiden/miden-tutorials/) repository and run the web application example:

```bash
cd web-client
npm i
npm run start
```
