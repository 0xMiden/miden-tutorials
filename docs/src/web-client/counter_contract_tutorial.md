# Deploying a Counter Contract

_Using the Miden client in TypeScript to deploy and interact with a custom smart contract on Miden_

## Overview

In this tutorial, we will deploy a simple counter smart contract that maintains a count, deploy it to the Miden testnet, and interact with it by incrementing the count, all from the Miden WebClient.

Using a script, we will invoke the increment function within the counter contract to update the count. This tutorial provides a foundational understanding of developing and deploying custom smart contracts on Miden.

## What we'll cover

- Deploying a custom smart contract on Miden
- Getting up to speed with the basics of Miden assembly
- Calling procedures in an account
- Pure vs state changing procedures

## Prerequisites

- Node `v20` or greater
- Familiarity with TypeScript
- `pnpm`

This tutorial assumes you have a basic understanding of Miden assembly. To quickly get up to speed with Miden assembly (MASM), please play around with running basic Miden assembly programs in the [Miden playground](https://0xpolygonmiden.github.io/examples/).

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
   pnpm install @demox-labs/miden-sdk@0.8.4
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
import { deployCounterContract } from "../lib/deployCounterContract";

export default function Home() {
  const [isCreatingLib, setIsCreateingLib] = useState(false);

  const handleCreateLib = async () => {
    setIsCreateingLib(true);
    await deployCounterContract();
    setIsCreateingLib(false);
  };

  return (
    <main className="min-h-screen flex items-center justify-center bg-gradient-to-br from-gray-900 via-gray-800 to-black text-slate-800 dark:text-slate-100">
      <div className="text-center">
        <h1 className="text-4xl font-semibold mb-4">Miden Web App</h1>
        <p className="mb-6">Open your browser console to see WebClient logs.</p>

        <div className="max-w-sm w-full bg-gray-800/20 border border-gray-600 rounded-2xl p-6 mx-auto flex flex-col gap-4">
          <button
            onClick={handleCreateLib}
            className="w-full px-6 py-3 text-lg cursor-pointer bg-transparent border-2 border-orange-600 text-white rounded-lg transition-all hover:bg-orange-600 hover:text-white"
          >
            {isCreatingLib
              ? "Working..."
              : "Tutorial #3: Deploy Counter Contract"}
          </button>
        </div>
      </div>
    </main>
  );
}
```

## Step 3 — Create and Deploy the Counter Contract

Create the file `lib/deployCounterContract.ts` and add the following code. This snippet defines the P2ID note script, implements the function `multiSendWithDelegatedProver`, and initializes the WebClient along with the delegated prover endpoint.

```
mkdir -p lib
touch lib/deployCounterContract.ts
```

Copy and paste the following code into the `lib/deployCounterContract.ts` file:

```ts
// lib/webClient.ts
export async function deployCounterContract(): Promise<void> {
  if (typeof window === "undefined") {
    console.warn("webClient() can only run in the browser");
    return;
  }

  // dynamic import → only in the browser, so WASM is loaded client‑side
  const {
    AccountBuilder,
    AccountComponent,
    AccountStorageMode,
    AccountType,
    AssemblerUtils,
    StorageSlot,
    TransactionKernel,
    TransactionRequestBuilder,
    TransactionScript,
    TransactionScriptInputPairArray,
    WebClient,
  } = await import("@demox-labs/miden-sdk");

  const nodeEndpoint = "http://localhost:57291";
  const client = await WebClient.createClient(nodeEndpoint);
  console.log("Current block number: ", (await client.syncState()).blockNum());

  // Counter contract code in Miden Assembly
  const accountCode = `
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
  let emptyStorageSlot = StorageSlot.emptyValue();

  let counterAccountComponent = AccountComponent.compile(
    accountCode, // contract code
    assembler, // assembler
    [emptyStorageSlot], // storage data
  ).withSupportsAllTypes();

  const walletSeed = new Uint8Array(32);
  crypto.getRandomValues(walletSeed);

  let anchorBlock = await client.getLatestEpochBlock();

  let accountBuilderResult = new AccountBuilder(walletSeed)
    .anchor(anchorBlock)
    .accountType(AccountType.RegularAccountImmutableCode)
    .storageMode(AccountStorageMode.public())
    .withComponent(counterAccountComponent)
    .build();

  // Importing the counter contract into the WebClient
  await client.newAccount(
    accountBuilderResult.account, // account
    accountBuilderResult.seed, // seed
    false, // overwrite
  );
  let counterContract = accountBuilderResult.account;
  console.log("Counter contract id: ", counterContract.id().toString());

  // Building the transaction script which will call the counter contract
  let txScriptCode = `
    use.external_contract::counter_contract
    begin
        call.counter_contract::increment_count
    end
  `;

  // Empty inputs to the transaction script
  const inputs = new TransactionScriptInputPairArray();

  // Creating the library to call the counter contract
  let counterComponentLib = AssemblerUtils.createAccountComponentLibrary(
    assembler, // assembler
    "external_contract::counter_contract", // library path to call the contract
    accountCode, // account code of the contract
  );

  // Creating the transaction script
  let txScript = TransactionScript.compile(
    txScriptCode,
    inputs,
    assembler.withLibrary(counterComponentLib),
  );

  // Creating a transaction request with the transaction script
  let txIncrementRequest = new TransactionRequestBuilder()
    .withCustomScript(txScript)
    .build();

  // Executing the transaction script against the counter contract
  let txResult = await client.newTransaction(
    accountBuilderResult.account.id(),
    txIncrementRequest,
  );

  // Submitting the transaction result to the node
  await client.submitTransaction(txResult);

  // Sync state
  await client.syncState();

  // Logging the count of counter contract
  let counter = await client.getAccount(counterContract.id());
  let count = counter?.storage().getItem(0);

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

Open the browser console and click the button "Deploy Counter Contract".

This is what you should see in the browser console:
```
Current block number:  2168
deployCounterContract.ts:101 Counter contract id:  0xab77ad2ac23ff0000000445bda9e10
deployCounterContract.ts:153 Count:  1
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
