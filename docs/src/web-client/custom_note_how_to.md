# How to Create a Custom Note

_Using the Miden WebClient to create notes with custom logic_

## Overview

In this tutorial, we will create a custom note on Miden that can only be consumed by someone who knows the preimage of the hash stored in the note. This approach securely embeds assets into the note and restricts spending to those who possess the correct secret number.

Using the Miden WebClient, we will demonstrate how to:

- Create a note with custom logic
- Leverage Miden's privacy features to keep certain transaction details private
- Consume notes with custom authentication requirements

Unlike Ethereum, where all pending transactions are publicly visible in the mempool, Miden enables you to partially or completely hide transaction details.

## What we'll cover

- Writing Miden assembly for a custom note script
- Creating and consuming custom notes using the WebClient
- Understanding hash-based authentication in notes

## Prerequisites

- Node `v20` or greater
- Familiarity with TypeScript
- `pnpm`

This tutorial assumes you have a basic understanding of Miden assembly. To quickly get up to speed with Miden assembly (MASM), please play around with running basic Miden assembly programs in the [Miden playground](https://0xmiden.github.io/examples/).

## Step 1: Initialize your Next.js project

1. Create a new Next.js app with TypeScript:

   ```bash
   npx create-next-app@latest miden-custom-note-app --typescript
   ```

   Hit enter for all terminal prompts.

2. Change into the project directory:

   ```bash
   cd miden-custom-note-app
   ```

3. Install the Miden WebClient SDK:
   ```bash
   pnpm i @demox-labs/miden-sdk@0.10.1
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

Add the following code to the `app/page.tsx` file. This code defines the main page of our web application:

```tsx
"use client";
import { useState } from "react";
import { customNoteDemo } from "../lib/customNoteDemo";

export default function Home() {
  const [isRunningDemo, setIsRunningDemo] = useState(false);

  const handleCustomNoteDemo = async () => {
    setIsRunningDemo(true);
    await customNoteDemo();
    setIsRunningDemo(false);
  };

  return (
    <main className="min-h-screen flex items-center justify-center bg-gradient-to-br from-gray-900 via-gray-800 to-black text-slate-800 dark:text-slate-100">
      <div className="text-center">
        <h1 className="text-4xl font-semibold mb-4">Miden Custom Note Demo</h1>
        <p className="mb-6">Open your browser console to see WebClient logs.</p>

        <div className="max-w-sm w-full bg-gray-800/20 border border-gray-600 rounded-2xl p-6 mx-auto flex flex-col gap-4">
          <button
            onClick={handleCustomNoteDemo}
            className="w-full px-6 py-3 text-lg cursor-pointer bg-transparent border-2 border-purple-600 text-white rounded-lg transition-all hover:bg-purple-600 hover:text-white"
          >
            {isRunningDemo ? "Working..." : "Create & Consume Custom Note"}
          </button>
        </div>
      </div>
    </main>
  );
}
```

## Step 3 — Creating and Consuming Custom Notes

Create the file `lib/customNoteDemo.ts` and add the following code.

```bash
mkdir -p lib
touch lib/customNoteDemo.ts
```

Copy and paste the following code into the `lib/customNoteDemo.ts` file:

```ts
// lib/customNoteDemo.ts

/**
 * Hash Preimage Note Script for Miden Network
 * This note can only be consumed by providing the correct secret preimage
 */
const HASH_PREIMAGE_NOTE_SCRIPT = `
use.miden::note
use.miden::contracts::wallets::basic->wallet

# CONSTANTS
# =================================================================================================

const.EXPECTED_DIGEST_PTR=0
const.ASSET_PTR=100

# ERRORS
# =================================================================================================

const.ERROR_DIGEST_MISMATCH="Expected digest does not match computed digest"

#! Inputs (arguments):  [HASH_PREIMAGE_SECRET]
#! Outputs: []
#!
#! Note inputs are assumed to be as follows:
#!  => EXPECTED_DIGEST
begin
    # => HASH_PREIMAGE_SECRET
    # Hashing the secret number
    hash
    # => [DIGEST]

    # Writing the note inputs to memory
    push.EXPECTED_DIGEST_PTR exec.note::get_inputs drop drop

    # Pad stack and load expected digest from memory
    padw push.EXPECTED_DIGEST_PTR mem_loadw
    # => [EXPECTED_DIGEST, DIGEST]

    # Assert that the note input matches the digest
    # Will fail if the two hashes do not match
    assert_eqw.err=ERROR_DIGEST_MISMATCH
    # => []

    # ---------------------------------------------------------------------------------------------
    # If the check is successful, we allow for the asset to be consumed
    # ---------------------------------------------------------------------------------------------

    # Write the asset in note to memory address ASSET_PTR
    push.ASSET_PTR exec.note::get_assets
    # => [num_assets, dest_ptr]

    drop
    # => [dest_ptr]

    # Load asset from memory
    mem_loadw
    # => [ASSET]

    # Call receive asset in wallet
    call.wallet::receive_asset
    # => []
end
`;

/**
 * Demonstrates creating and consuming a custom note with hash preimage authentication
 */
export async function customNoteDemo(): Promise<void> {
  if (typeof window === "undefined") {
    console.warn("customNoteDemo() can only run in the browser");
    return;
  }

  // Dynamic import → only in the browser, so WASM is loaded client‑side
  const {
    WebClient,
    AccountStorageMode,
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
    Rpo256,
  } = await import("@demox-labs/miden-sdk");

  const nodeEndpoint = "https://rpc.testnet.miden.io:443";
  const client = await WebClient.createClient(nodeEndpoint);
  const prover = TransactionProver.newRemoteProver(
    "https://tx-prover.testnet.miden.io",
  );

  console.log("Current block number: ", (await client.syncState()).blockNum());

  // ── Step 1: Create accounts ──────────────────────────────────────────────────────
  console.log("\n[STEP 1] Creating accounts");

  console.log("Creating Alice's account...");
  const alice = await client.newWallet(AccountStorageMode.public(), true);
  console.log("Alice account ID:", alice.id().toString());

  console.log("Creating Bob's account...");
  const bob = await client.newWallet(AccountStorageMode.public(), true);
  console.log("Bob account ID:", bob.id().toString());

  // ── Step 2: Create faucet and mint tokens ──────────────────────────────────────────────────────
  console.log("\n[STEP 2] Creating faucet and minting tokens");

  const faucet = await client.newFaucet(
    AccountStorageMode.public(),
    false,
    "MID",
    8,
    BigInt(1_000_000),
  );
  console.log("Faucet ID:", faucet.id().toString());

  // Mint 100 MID to Alice
  await client.submitTransaction(
    await client.newTransaction(
      faucet.id(),
      client.newMintTransactionRequest(
        alice.id(),
        faucet.id(),
        NoteType.Public,
        BigInt(100),
      ),
    ),
    prover,
  );

  console.log("Waiting for settlement...");
  await new Promise((r) => setTimeout(r, 7_000));
  await client.syncState();

  // Consume the freshly minted note
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

  // ── Step 3: Create custom note with hash preimage ──────────────────────────────────────────────
  console.log("\n[STEP 3] Creating custom note with hash preimage");

  // Define the secret values (4 field elements)
  const secretValues = [
    new Felt(BigInt(1)),
    new Felt(BigInt(2)),
    new Felt(BigInt(3)),
    new Felt(BigInt(4)),
  ];

  console.log(
    "Secret values:",
    secretValues.map((f) => f.toString()),
  );

  // Compute the hash of the secret values using Rpo256.hashElements
  const secretHash = Rpo256.hashElements(new FeltArray(secretValues));
  console.log("Secret hash:", secretHash.toString());

  // Compile the note script
  const noteScript = client.compileNoteScript(HASH_PREIMAGE_NOTE_SCRIPT);

  // Create note inputs with the hash (convert RpoDigest to array of Felts)
  const hashElements = [
    secretHash.element(0),
    secretHash.element(1),
    secretHash.element(2),
    secretHash.element(3),
  ];
  const noteInputs = new NoteInputs(new FeltArray(hashElements));

  // Create the note assets (50 MID tokens)
  const assets = new NoteAssets([new FungibleAsset(faucet.id(), BigInt(50))]);

  // Create note metadata
  const metadata = new NoteMetadata(
    alice.id(),
    NoteType.Public,
    NoteTag.fromAccountId(alice.id()),
    NoteExecutionHint.always(),
  );

  // Generate a random serial number for the note
  const serialNumber = Word.newFromFelts([
    new Felt(BigInt(Math.floor(Math.random() * 0x1_0000_0000))),
    new Felt(BigInt(Math.floor(Math.random() * 0x1_0000_0000))),
    new Felt(BigInt(Math.floor(Math.random() * 0x1_0000_0000))),
    new Felt(BigInt(Math.floor(Math.random() * 0x1_0000_0000))),
  ]);

  // Create the custom note
  const customNote = new Note(
    assets,
    metadata,
    new NoteRecipient(serialNumber, noteScript, noteInputs),
  );

  console.log("Custom note ID:", customNote.id().toString());

  // Create transaction to output the custom note
  const createNoteTransaction = await client.newTransaction(
    alice.id(),
    new TransactionRequestBuilder()
      .withOwnOutputNotes(new OutputNotesArray([OutputNote.full(customNote)]))
      .build(),
  );

  await client.submitTransaction(createNoteTransaction, prover);

  const createTxId = createNoteTransaction
    .executedTransaction()
    .id()
    .toHex()
    .toString();

  console.log(
    `Custom note created! View on MidenScan: https://testnet.midenscan.com/tx/${createTxId}`,
  );

  // ── Step 4: Consume the custom note ──────────────────────────────────────────────
  console.log("\n[STEP 4] Bob consumes the custom note with correct secret");

  // Create the secret as a Word (4 field elements) - the preimage that will be hashed
  const secretWord = Word.newFromFelts(secretValues);

  // Create note and args for consumption
  const noteAndArgs = new NoteAndArgs(customNote, secretWord);

  // Create transaction request to consume the custom note
  const consumeRequest = new TransactionRequestBuilder()
    .withUnauthenticatedInputNotes(new NoteAndArgsArray([noteAndArgs]))
    .build();

  // Execute the consumption transaction
  const consumeTransaction = await client.newTransaction(
    bob.id(),
    consumeRequest,
  );

  await client.submitTransaction(consumeTransaction, prover);

  const consumeTxId = consumeTransaction
    .executedTransaction()
    .id()
    .toHex()
    .toString();

  console.log(
    `Custom note consumed! View on MidenScan: https://testnet.midenscan.com/tx/${consumeTxId}`,
  );

  // Show the account delta (what Bob received)
  const accountDelta = consumeTransaction.accountDelta();
  console.log("Bob's account delta:", accountDelta.toString());

  console.log("\n✅ Custom note demo completed successfully!");
  console.log(
    "Bob successfully consumed the note by providing the correct secret preimage.",
  );
}
```

To run the code above in our frontend, run the following command:

```bash
pnpm run dev
```

Open the browser console and click the button "Create & Consume Custom Note".

This is what you should see in the browser console:

```
Current block number:  2168

[STEP 1] Creating accounts
Creating Alice's account...
Alice account ID: mtst1qqufkq3xr0rr5yqqqwgrc20ctythccy6
Creating Bob's account...
Bob account ID: mtst1qz76c9fvhvms2yqqqvvw8tf6m5h86y2h

[STEP 2] Creating faucet and minting tokens
Faucet ID: mtst1qpwsgjstpwvykgqqqwwzgz3u5vwuuywe
Waiting for settlement...

[STEP 3] Creating custom note with hash preimage
Secret values: 1,2,3,4
Secret hash: RpoDigest([14371582251229115050, 1386930022051078873, 17689831064175867466, 9632123050519021080])
Custom note ID: 0x14c66143377223e090e5b4da0d1e5ce6c6521622ad5b92161a704a25c915769b
Custom note created! View on MidenScan: https://testnet.midenscan.com/tx/0xffbee228a2c6283efe958c6b3cd31af88018c029221b413b0f23fcfacb2cb611

[STEP 4] Bob consumes the custom note with correct secret
Custom note consumed! View on MidenScan: https://testnet.midenscan.com/tx/0xe6c8bb7b469e03dcacd8f1f400011a781e96ad4266ede11af8e711379e85b929
Bob's account delta: AccountDelta { nonce: Some(Felt(2)), vault: AccountVaultDelta { fungible: FungibleAssetDelta({V0(AccountIdV0 { prefix: 6702563556733766432, suffix: 1016103534633728 }): 50}), non_fungible: NonFungibleAssetDelta({}) }, storage: AccountStorageDelta { cleared_items: [], updated_items: [] }, code: None }

✅ Custom note demo completed successfully!
Bob successfully consumed the note by providing the correct secret preimage.
```

## Miden Assembly Custom Note Script Explainer

The custom note script uses hash-based authentication to ensure only someone with the correct secret can consume the note.

#### Here's a breakdown of what the note script does:

1. **Hash the Secret**: Takes the secret preimage provided as note arguments and hashes it using the `hash` instruction.

2. **Load Expected Hash**: Retrieves the expected hash from the note inputs (stored when the note was created).

3. **Compare Hashes**: Uses `assert_eqw` to compare the computed hash with the expected hash. If they don't match, the transaction fails with an error.

4. **Transfer Assets**: If the hash verification succeeds, the script loads the assets from the note and transfers them to the consuming account using `wallet::receive_asset`.

```masm
use.miden::note
use.miden::contracts::wallets::basic->wallet

# CONSTANTS
const.EXPECTED_DIGEST_PTR=0
const.ASSET_PTR=100

# ERRORS
const.ERROR_DIGEST_MISMATCH="Expected digest does not match computed digest"

#! Inputs (arguments):  [HASH_PREIMAGE_SECRET]
#! Outputs: []
#!
#! Note inputs are assumed to be as follows:
#!  => EXPECTED_DIGEST
begin
    # => HASH_PREIMAGE_SECRET
    # Hashing the secret number
    hash
    # => [DIGEST]

    # Writing the note inputs to memory
    push.EXPECTED_DIGEST_PTR exec.note::get_inputs drop drop

    # Pad stack and load expected digest from memory
    padw push.EXPECTED_DIGEST_PTR mem_loadw
    # => [EXPECTED_DIGEST, DIGEST]

    # Assert that the note input matches the digest
    # Will fail if the two hashes do not match
    assert_eqw.err=ERROR_DIGEST_MISMATCH
    # => []

    # ---------------------------------------------------------------------------------------------
    # If the check is successful, we allow for the asset to be consumed
    # ---------------------------------------------------------------------------------------------

    # Write the asset in note to memory address ASSET_PTR
    push.ASSET_PTR exec.note::get_assets
    # => [num_assets, dest_ptr]

    drop
    # => [dest_ptr]

    # Load asset from memory
    mem_loadw
    # => [ASSET]

    # Call receive asset in wallet
    call.wallet::receive_asset
    # => []
end
```

**Note**: _It's a good habit to add comments below each line of MASM code with the expected stack state. This improves readability and helps with debugging._

## Key Concepts

### Privacy and Security

Unlike traditional blockchains where transaction details are fully public, Miden allows you to:

- **Hide transaction logic**: The note script logic is not visible on-chain until execution
- **Conditional execution**: Notes can only be consumed when specific conditions are met
- **Secret-based authentication**: Use hash preimages, signatures, or other cryptographic proofs

### Custom Note Components

1. **Note Script**: The Miden Assembly code that defines the consumption logic
2. **Note Inputs**: Data embedded in the note (like the expected hash)
3. **Note Arguments**: Data provided during consumption (like the secret preimage)
4. **Note Assets**: The fungible or non-fungible assets contained in the note

### Hash-based Authentication

The hash preimage pattern is useful for:

- **Atomic swaps**: Ensuring both parties know a shared secret
- **Conditional payments**: Releasing funds only when conditions are met
- **Privacy-preserving transfers**: Hiding the unlock condition until consumption

## Running the example

To run a full working example navigate to the `web-client` directory in the [miden-tutorials](https://github.com/0xMiden/miden-tutorials/) repository and run the web application example:

```bash
cd web-client
pnpm i
pnpm run start
```

## Resetting the `MidenClientDB`

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

## Conclusion

You have now learned how to create custom notes on Miden using the WebClient that require specific secrets to be consumed. We covered:

1. Creating accounts and setting up a faucet
2. Defining a custom note script with hash-based authentication
3. Creating a note with embedded hash inputs
4. Consuming the note by providing the correct secret preimage

By leveraging Miden's privacy features and custom note scripts, you can create sophisticated conditional payment systems, atomic swaps, and other advanced financial primitives that maintain privacy while ensuring security.

### Continue learning

Next tutorial: [How to Use Unauthenticated Notes](unauthenticated_note_how_to.md)
