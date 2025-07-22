# Creating Multiple Notes in a Single Transaction

_Using the Miden WebClient in TypeScript to create several P2ID notes in a single transaction_

## Overview

In the previous sections we learned how to create accounts, deploy faucets, and mint tokens. In this tutorial we will:

- **Mint** test tokens from a faucet to Alice
- **Consume** the minted notes so the assets appear in Alice’s wallet
- **Create three P2ID notes in a _single_ transaction** using a custom note‑script and delegated proving

The entire flow is wrapped in a helper called `multiSendWithDelegatedProver()` that you can call from any browser page.

## What we’ll cover

1. Setting‑up the WebClient and initializing a remote prover
2. Building three P2ID notes worth 100 `MID` each
3. Submitting the transaction _using delegated proving_

## Prerequisites

- Node `v20` or greater
- Familiarity with TypeScript
- `pnpm`

## What is Delegated Proving?

Before diving into our code example, let's clarify what in the world "delegated proving" actually is.

Delegated proving is the process of outsourcing a part of the ZK proof generation of your transaction to a third party. For certain computationally constrained devices such as mobile phones and web browser environments, generating ZK proofs might take too long to ensure an acceptable user experience. Devices that do not have the computational resources to generate Miden proofs in under 1-2 seconds can use delegated proving to provide a more responsive user experience.

_How does it work?_ When a user choses to use delegated proving, they send off a portion of the zk proof of their transaction to a dedicated server. This dedicated server generates the remainder of the ZK proof of the transaction and submits it to the network. Submitting a transaction with delegated proving is trustless, meaning if the delegated prover is malicious, the could not compromise the security of the account that is submitting a transaction to be processed by the delegated prover. The downside of using delegated proving is that it reduces the privacy of the account that uses delegated proving, because the delegated prover would have knowledge of the transaction that is being proven. Additionally, transactions that require sensitive data such as the knowledge of a hash preimage or a secret, should not use delegated proving as this data will be shared with the delegated prover for proof generation.

Anyone can run their own delegated prover server. If you are building a product on Miden, it may make sense to run your own delegated prover server for your users. To run your own delegated proving server, follow the instructions here: https://crates.io/crates/miden-proving-service

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

## Step 2: Edit the `app/page.tsx` file:

Add the following code to the `app/page.tsx` file:

```tsx
"use client";
import { useState } from "react";
import { multiSendWithDelegatedProver } from "../lib/multiSendWithDelegatedProver";

export default function Home() {
  const [isMultiSendNotes, setIsMultiSendNotes] = useState(false);

  const handleMultiSendNotes = async () => {
    setIsMultiSendNotes(true);
    await multiSendWithDelegatedProver();
    setIsMultiSendNotes(false);
  };

  return (
    <main className="min-h-screen flex items-center justify-center bg-gradient-to-br from-gray-900 via-gray-800 to-black text-slate-800 dark:text-slate-100">
      <div className="text-center">
        <h1 className="text-4xl font-semibold mb-4">Miden Web App</h1>
        <p className="mb-6">Open your browser console to see WebClient logs.</p>

        <div className="max-w-sm w-full bg-gray-800/20 border border-gray-600 rounded-2xl p-6 mx-auto flex flex-col gap-4">
          <button
            onClick={handleMultiSendNotes}
            className="w-full px-6 py-3 text-lg cursor-pointer bg-transparent border-2 border-orange-600 text-white rounded-lg transition-all hover:bg-orange-600 hover:text-white"
          >
            {isMultiSendNotes
              ? "Working..."
              : "Tutorial #2: Send 1 to N P2ID Notes with Delegated Proving"}
          </button>
        </div>
      </div>
    </main>
  );
}
```

## Step 3 — Initalize the WebClient & Define the Note Script 

Create the file `lib/multiSendWithDelegatedProver.ts` and add the following code. This snippet defines the P2ID note script, implements the function `multiSendWithDelegatedProver`, and initializes the WebClient along with the delegated prover endpoint.

```
mkdir -p lib
touch lib/multiSendWithDelegatedProver.ts
```

```ts
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

export async function multiSendWithDelegatedProver(): Promise<void> {
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
    TransactionRequestBuilder,
    OutputNote,
  } = await import("@demox-labs/miden-sdk");

  const client = await WebClient.createClient(
    "https://rpc.testnet.miden.io:443",
  );
  const prover = TransactionProver.newRemoteProver(
    "https://tx-prover.testnet.miden.io",
  );

  console.log("Latest block:", (await client.syncState()).blockNum());
}
```

## Step 4 — Create an account, deploy a faucet, mint and consume tokens 

Add the code snippet below to the `multiSendWithDelegatedProver` function. This code creates a wallet and faucet, mints tokens from the faucet for the wallet, and then consumes the minted tokens.

```ts
// ── Creating new account ──────────────────────────────────────────────────────
console.log("Creating account for Alice…");
const alice = await client.newWallet(AccountStorageMode.public(), true);
console.log("Alice accout ID:", alice.id().toString());

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

console.log("waiting for settlement");
await new Promise((r) => setTimeout(r, 7_000));
await client.syncState();

// ── consume the freshly minted notes ──────────────────────────────────────────────
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
```

## Step 5 — Build and Create P2ID notes

Add the following code to the `multiSendWithDelegatedProver` function. This code defines three recipient addresses, builds three P2ID notes with 100 `MID` each, and then creates all three notes in the same transaction.

```ts
// ── build 3 P2ID notes (100 MID each) ─────────────────────────────────────────────
const recipientAddresses = [
  "0xbf1db1694c83841000008cefd4fce0",
  "0xee1a75244282c32000010a29bed5f4",
  "0x67dc56bd0cbe629000006f36d81029",
];

const script = client.compileNoteScript(P2ID_NOTE_SCRIPT);

const assets = new NoteAssets([new FungibleAsset(faucet.id(), BigInt(100))]);
const metadata = new NoteMetadata(
  alice.id(),
  NoteType.Public,
  NoteTag.fromAccountId(alice.id(), NoteExecutionMode.newLocal()),
  NoteExecutionHint.always(),
);

const p2idNotes = recipientAddresses.map((addr) => {
  let serialNumber = Word.newFromFelts([
    new Felt(BigInt(Math.floor(Math.random() * 0x1_0000_0000))),
    new Felt(BigInt(Math.floor(Math.random() * 0x1_0000_0000))),
    new Felt(BigInt(Math.floor(Math.random() * 0x1_0000_0000))),
    new Felt(BigInt(Math.floor(Math.random() * 0x1_0000_0000))),
  ]);

  const acct = AccountId.fromHex(addr);
  const inputs = new NoteInputs(new FeltArray([acct.suffix(), acct.prefix()]));

  let note = new Note(
    assets,
    metadata,
    new NoteRecipient(serialNumber, script, inputs),
  );

  return OutputNote.full(note);
});

// ── create all P2ID notes ───────────────────────────────────────────────────────────────
let transaction = await client.newTransaction(
  alice.id(),
  new TransactionRequestBuilder()
    .withOwnOutputNotes(new OutputNotesArray(p2idNotes))
    .build(),
);

// ── submit tx ───────────────────────────────────────────────────────────────
await client.submitTransaction(transaction, prover);

console.log("All notes created ✅");
```

## Summary

Your `lib/multiSendWithDelegatedProver.ts` file sould now look like this:

```ts
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

export async function multiSendWithDelegatedProver(): Promise<void> {
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
    TransactionRequestBuilder,
    OutputNote,
  } = await import("@demox-labs/miden-sdk");

  const client = await WebClient.createClient(
    "https://rpc.testnet.miden.io:443",
  );
  const prover = TransactionProver.newRemoteProver(
    "https://tx-prover.testnet.miden.io",
  );

  console.log("Latest block:", (await client.syncState()).blockNum());

  // ── Creating new account ──────────────────────────────────────────────────────
  console.log("Creating account for Alice…");
  const alice = await client.newWallet(AccountStorageMode.public(), true);
  console.log("Alice accout ID:", alice.id().toString());

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

  console.log("waiting for settlement");
  await new Promise((r) => setTimeout(r, 7_000));
  await client.syncState();

  // ── consume the freshly minted notes ──────────────────────────────────────────────
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

  // ── build 3 P2ID notes (100 MID each) ─────────────────────────────────────────────
  const recipientAddresses = [
    "0xbf1db1694c83841000008cefd4fce0",
    "0xee1a75244282c32000010a29bed5f4",
    "0x67dc56bd0cbe629000006f36d81029",
  ];

  const script = client.compileNoteScript(P2ID_NOTE_SCRIPT);

  const assets = new NoteAssets([new FungibleAsset(faucet.id(), BigInt(100))]);
  const metadata = new NoteMetadata(
    alice.id(),
    NoteType.Public,
    NoteTag.fromAccountId(alice.id(), NoteExecutionMode.newLocal()),
    NoteExecutionHint.always(),
  );

  const p2idNotes = recipientAddresses.map((addr) => {
    let serialNumber = Word.newFromFelts([
      new Felt(BigInt(Math.floor(Math.random() * 0x1_0000_0000))),
      new Felt(BigInt(Math.floor(Math.random() * 0x1_0000_0000))),
      new Felt(BigInt(Math.floor(Math.random() * 0x1_0000_0000))),
      new Felt(BigInt(Math.floor(Math.random() * 0x1_0000_0000))),
    ]);

    const acct = AccountId.fromHex(addr);
    const inputs = new NoteInputs(
      new FeltArray([acct.suffix(), acct.prefix()]),
    );

    let note = new Note(
      assets,
      metadata,
      new NoteRecipient(serialNumber, script, inputs),
    );

    return OutputNote.full(note);
  });

  // ── create all P2ID notes ───────────────────────────────────────────────────────────────
  let transaction = await client.newTransaction(
    alice.id(),
    new TransactionRequestBuilder()
      .withOwnOutputNotes(new OutputNotesArray(p2idNotes))
      .build(),
  );

  // ── submit tx ───────────────────────────────────────────────────────────────
  await client.submitTransaction(transaction, prover);

  console.log("All notes created ✅");
}
```

### Running the example

To run a full working example navigate to the `web-client` directory in the [miden-tutorials](https://github.com/0xMiden/miden-tutorials/) repository and run the web application example:

```bash
cd web-client
pnpm i
pnpm run start
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
