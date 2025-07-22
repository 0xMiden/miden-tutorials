# Incrementing the Count of the Counter Contract

_Using the Miden WebClient to interact with a custom smart contract_

## Overview

In this tutorial, we will interact with a counter contract already deployed on chain by incrementing the count using the Miden WebClient.

Using a script, we will invoke the increment function within the counter contract to update the count. This tutorial provides a foundational understanding of interacting with custom smart contracts on Miden.

## What we'll cover

- Interacting with a custom smart contract on Miden
- Calling procedures in an account from a script

## Prerequisites

- Node `v20` or greater
- Familiarity with TypeScript
- `pnpm`

This tutorial assumes you have a basic understanding of Miden assembly. To quickly get up to speed with Miden assembly (MASM), please play around with running basic Miden assembly programs in the [Miden playground](https://0xmiden.github.io/examples/).

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
import { incrementCounterContract } from "../lib/incrementCounterContract";

export default function Home() {
  const [isIncrementCounter, setIsIncrementCounter] = useState(false);

  const handleIncrementCounterContract = async () => {
    setIsIncrementCounter(true);
    await incrementCounterContract();
    setIsIncrementCounter(false);
  };

  return (
    <main className="min-h-screen flex items-center justify-center bg-gradient-to-br from-gray-900 via-gray-800 to-black text-slate-800 dark:text-slate-100">
      <div className="text-center">
        <h1 className="text-4xl font-semibold mb-4">Miden Web App</h1>
        <p className="mb-6">Open your browser console to see WebClient logs.</p>

        <div className="max-w-sm w-full bg-gray-800/20 border border-gray-600 rounded-2xl p-6 mx-auto flex flex-col gap-4">
          <button
            onClick={handleIncrementCounterContract}
            className="w-full px-6 py-3 text-lg cursor-pointer bg-transparent border-2 border-orange-600 text-white rounded-lg transition-all hover:bg-orange-600 hover:text-white"
          >
            {isIncrementCounter
              ? "Working..."
              : "Tutorial #3: Increment Counter Contract"}
          </button>
        </div>
      </div>
    </main>
  );
}
```

## Step 3 — Incrementing the Count of the Counter Contract

Create the file `lib/incrementCounterContract.ts` and add the following code.

```
mkdir -p lib
touch lib/incrementCounterContract.ts
```

Copy and paste the following code into the `lib/incrementCounterContract.ts` file:

```ts
// lib/incrementCounterContract.ts
export async function incrementCounterContract(): Promise<void> {
  if (typeof window === "undefined") {
    console.warn("webClient() can only run in the browser");
    return;
  }

  // dynamic import → only in the browser, so WASM is loaded client‑side
  const {
    AccountId,
    AssemblerUtils,
    StorageSlot,
    TransactionKernel,
    TransactionRequestBuilder,
    TransactionScript,
    TransactionScriptInputPairArray,
    WebClient,
  } = await import("@demox-labs/miden-sdk");

  const nodeEndpoint = "https://rpc.testnet.miden.io:443";
  const client = await WebClient.createClient(nodeEndpoint);
  console.log("Current block number: ", (await client.syncState()).blockNum());

  // Counter contract code in Miden Assembly
  const counterContractCode = `
      use.miden::account
      use.std::sys

      # => []
      export.get_count
          push.0
          # => [index]
          
          # exec.account::get_item
          # => [count]
          
          # exec.sys::truncate_stack
          # => []
      end

      # => []
      export.increment_count
          push.0
          # => [index]
          
          exec.account::get_item
          # => [count]
          
          push.1 add
          # => [count+1]

          # debug statement with client
          debug.stack

          push.0
          # [index, count+1]
          
          exec.account::set_item
          # => []
          
          push.1 exec.account::incr_nonce
          # => []
          
          exec.sys::truncate_stack
          # => []
      end
    `;

  // Building the counter contract
  let assembler = TransactionKernel.assembler();

  // Counter contract account id on testnet
  const counterContractId = AccountId.fromBech32(
    "mtst1qz43ftxkrzcjsqz3hpw332qwny2ggsp0",
  );

  // Reading the public state of the counter contract from testnet,
  // and importing it into the WebClient
  let counterContractAccount = await client.getAccount(counterContractId);
  if (!counterContractAccount) {
    await client.importAccountById(counterContractId);
    await client.syncState();
    counterContractAccount = await client.getAccount(counterContractId);
    if (!counterContractAccount) {
      throw new Error(`Account not found after import: ${counterContractId}`);
    }
  }

  // Building the transaction script which will call the counter contract
  let txScriptCode = `
    use.external_contract::counter_contract
    begin
        call.counter_contract::increment_count
    end
  `;

  // Creating the library to call the counter contract
  let counterComponentLib = AssemblerUtils.createAccountComponentLibrary(
    assembler, // assembler
    "external_contract::counter_contract", // library path to call the contract
    counterContractCode, // account code of the contract
  );

  // Creating the transaction script
  let txScript = TransactionScript.compile(
    txScriptCode,
    assembler.withLibrary(counterComponentLib),
  );

  // Creating a transaction request with the transaction script
  let txIncrementRequest = new TransactionRequestBuilder()
    .withCustomScript(txScript)
    .build();

  // Executing the transaction script against the counter contract
  let txResult = await client.newTransaction(
    counterContractAccount.id(),
    txIncrementRequest,
  );

  // Submitting the transaction result to the node
  await client.submitTransaction(txResult);

  // Sync state
  await client.syncState();

  // Logging the count of counter contract
  let counter = await client.getAccount(counterContractAccount.id());

  // Here we get the first Word from storage of the counter contract
  // A word is comprised of 4 Felts, 2**64 - 2**32 + 1
  let count = counter?.storage().getItem(1);

  // Converting the Word represented as a hex to a single integer value
  const counterValue = Number(
    BigInt("0x" + count!.toHex().slice(-16).match(/../g)!.reverse().join("")),
  );

  console.log("Count: ", counterValue);
}
```

To run the code above in our frontend, run the following command:

```
pnpm run dev
```

Open the browser console and click the button "Increment Counter Contract".

This is what you should see in the browser console:

```
Current block number:  2168
incrementCounterContract.ts:153 Count:  3
```

## Miden Assembly Counter Contract Explainer

#### Here's a breakdown of what the `get_count` procedure does:

1. Pushes `0` onto the stack, representing the index of the storage slot to read.
2. Calls `account::get_item` with the index of `0`.
3. Calls `sys::truncate_stack` to truncate the stack to size 16.
4. The value returned from `account::get_item` is still on the stack and will be returned when this procedure is called.

#### Here's a breakdown of what the `increment_count` procedure does:

1. Pushes `0` onto the stack, representing the index of the storage slot to read.
2. Calls `account::get_item` with the index of `0`.
3. Pushes `1` onto the stack.
4. Adds `1` to the count value returned from `account::get_item`.
5. _For demonstration purposes_, calls `debug.stack` to see the state of the stack
6. Pushes `0` onto the stack, which is the index of the storage slot we want to write to.
7. Calls `account::set_item` which saves the incremented count to storage at index `0`
8. Calls `sys::truncate_stack` to truncate the stack to size 16.

```masm
use.miden::account
use.std::sys

# => []
export.get_count
    push.0
    # => [index]

    exec.account::get_item
    # => [count]

    exec.sys::truncate_stack
    # => []
end

# => []
export.increment_count
    push.0
    # => [index]

    exec.account::get_item
    # => [count]

    push.1 add
    # => [count+1]

    # debug statement with client
    debug.stack

    push.0
    # [index, count+1]

    exec.account::set_item
    # => []

    push.1 exec.account::incr_nonce
    # => []

    exec.sys::truncate_stack
    # => []
end
```

**Note**: _It's a good habit to add comments below each line of MASM code with the expected stack state. This improves readability and helps with debugging._

### Concept of function visibility and modifiers in Miden smart contracts

The `export.increment_count` function in our Miden smart contract behaves like an "external" Solidity function without a modifier, meaning any user can call it to increment the contract's count. This is because it calls `account::incr_nonce` during execution. For internal procedures, use the `proc` keyword as opposed to `export`.

If the `increment_count` procedure did not call the `account::incr_nonce` procedure during its execution, only the deployer of the counter contract would be able to increment the count of the smart contract (if the RpoFalcon512 component was added to the account, in this case we didn't add it).

In essence, if a procedure performs a state change in the Miden smart contract, and does not call `account::incr_nonce` at some point during its execution, this function can be equated to having an `onlyOwner` Solidity modifer, meaning only the user with knowledge of the private key of the account can execute transactions that result in a state change.

**Note**: _Adding the `account::incr_nonce` to a state changing procedure allows any user to call the procedure._

### Custom script

This is the Miden assembly script that calls the `increment_count` procedure during the transaction.

```masm
use.external_contract::counter_contract

begin
    call.counter_contract::increment_count
end
```

### Running the example

To run a full working example navigate to the `web-client` directory in the [miden-tutorials](https://github.com/0xMiden/miden-tutorials/) repository and run the web application example:

```bash
cd web-client
pnpm i
pnpm run start
```

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
