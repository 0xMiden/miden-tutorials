# Foreign Procedure Invocation Tutorial

_Using foreign procedure invocation to craft read-only cross-contract calls with the WebClient_

## Overview

In previous tutorials we deployed a public counter contract and incremented the count from a different client instance.

In this tutorial we will cover the basics of "foreign procedure invocation" (FPI) using the WebClient. To demonstrate FPI, we will build a "count copy" smart contract that reads the count from our previously deployed counter contract and copies the count to its own local storage.

Foreign procedure invocation (FPI) is a powerful tool for building composable smart contracts in Miden. FPI allows one smart contract or note to read the state of another contract.

The term "foreign procedure invocation" might sound a bit verbose, but it is as simple as one smart contract calling a non-state modifying procedure in another smart contract. The "EVM equivalent" of foreign procedure invocation would be a smart contract calling a read-only function in another contract.

FPI is useful for developing smart contracts that extend the functionality of existing contracts on Miden. FPI is the core primitive used by price oracles on Miden.

## What We Will Build

![Count Copy FPI diagram](../assets/count_copy_fpi_diagram.png)

The diagram above depicts the "count copy" smart contract using foreign procedure invocation to read the count state of the counter contract. After reading the state via FPI, the "count copy" smart contract writes the value returned from the counter contract to storage.

## What we'll cover

- Foreign Procedure Invocation (FPI) with the WebClient
- Building a "count copy" smart contract
- Executing cross-contract calls in the browser

## Prerequisites

- Node `v20` or greater
- Familiarity with TypeScript
- `pnpm`

This tutorial assumes you have a basic understanding of Miden assembly and completed the previous tutorial on incrementing the counter contract. To quickly get up to speed with Miden assembly (MASM), please play around with running basic Miden assembly programs in the [Miden playground](https://0xmiden.github.io/examples/).

## Step 1: Initialize your Next.js project

1. Create a new Next.js app with TypeScript:

   ```bash
   npx create-next-app@latest miden-fpi-app --typescript
   ```

   Hit enter for all terminal prompts.

2. Change into the project directory:

   ```bash
   cd miden-fpi-app
   ```

3. Install the Miden WebClient SDK:
   ```bash
   pnpm i @demox-labs/miden-sdk@0.9.4
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
import { foreignProcedureInvocation } from "../lib/foreignProcedureInvocation";

export default function Home() {
  const [isFPIRunning, setIsFPIRunning] = useState(false);

  const handleForeignProcedureInvocation = async () => {
    setIsFPIRunning(true);
    await foreignProcedureInvocation();
    setIsFPIRunning(false);
  };

  return (
    <main className="min-h-screen flex items-center justify-center bg-gradient-to-br from-gray-900 via-gray-800 to-black text-slate-800 dark:text-slate-100">
      <div className="text-center">
        <h1 className="text-4xl font-semibold mb-4">Miden FPI Web App</h1>
        <p className="mb-6">Open your browser console to see WebClient logs.</p>

        <div className="max-w-sm w-full bg-gray-800/20 border border-gray-600 rounded-2xl p-6 mx-auto flex flex-col gap-4">
          <button
            onClick={handleForeignProcedureInvocation}
            className="w-full px-6 py-3 text-lg cursor-pointer bg-transparent border-2 border-orange-600 text-white rounded-lg transition-all hover:bg-orange-600 hover:text-white"
          >
            {isFPIRunning
              ? "Working..."
              : "Foreign Procedure Invocation Tutorial"}
          </button>
        </div>
      </div>
    </main>
  );
}
```

## Step 3: Create the Foreign Procedure Invocation Implementation

Create the file `lib/foreignProcedureInvocation.ts` and add the following code.

```bash
mkdir -p lib
touch lib/foreignProcedureInvocation.ts
```

Copy and paste the following code into the `lib/foreignProcedureInvocation.ts` file:

```ts
// lib/foreignProcedureInvocation.ts
export async function foreignProcedureInvocation(): Promise<void> {
  if (typeof window === "undefined") {
    console.warn("foreignProcedureInvocation() can only run in the browser");
    return;
  }

  // dynamic import → only in the browser, so WASM is loaded client‑side
  const {
    AccountBuilder,
    AccountComponent,
    AccountId,
    AccountType,
    AssemblerUtils,
    StorageSlot,
    TransactionKernel,
    TransactionRequestBuilder,
    TransactionScript,
    TransactionScriptInputPairArray,
    ForeignAccount,
    AccountStorageRequirements,
    WebClient,
    AccountStorageMode,
  } = await import("@demox-labs/miden-sdk");

  const nodeEndpoint = "https://rpc.testnet.miden.io:443";
  const client = await WebClient.createClient(nodeEndpoint);
  console.log("Current block number: ", (await client.syncState()).blockNum());

  // -------------------------------------------------------------------------
  // STEP 1: Create the Count Reader Contract
  // -------------------------------------------------------------------------
  console.log("\n[STEP 1] Creating count reader contract.");

  // Count reader contract code in Miden Assembly
  const countReaderCode = `
    use.miden::account
    use.miden::tx
    use.std::sys

    # => [account_id_prefix, account_id_suffix, get_count_proc_hash]
    export.copy_count
        exec.tx::execute_foreign_procedure
        # => [count]
        
        debug.stack
        # => [count]
        
        push.0
        # [index, count]
        
        exec.account::set_item
        # => []
        
        push.1 exec.account::incr_nonce
        # => []

        exec.sys::truncate_stack
        # => []
    end
  `;

  // Prepare assembler (debug mode = true)
  let assembler = TransactionKernel.assembler().withDebugMode(true);

  let countReaderComponent = AccountComponent.compile(
    countReaderCode,
    assembler,
    [StorageSlot.emptyValue()],
  ).withSupportsAllTypes();

  const seed = new Uint8Array(32);
  crypto.getRandomValues(seed);

  let anchor = await client.getLatestEpochBlock();

  let countReaderContract = new AccountBuilder(seed)
    .anchor(anchor)
    .accountType(AccountType.RegularAccountImmutableCode)
    .storageMode(AccountStorageMode.public())
    .withComponent(countReaderComponent)
    .build();

  console.log(
    "Count reader contract ID:",
    countReaderContract.account.id().toString(),
  );

  await client.newAccount(
    countReaderContract.account,
    countReaderContract.seed,
    false,
  );

  // -------------------------------------------------------------------------
  // STEP 2: Build & Get State of the Counter Contract
  // -------------------------------------------------------------------------
  console.log("\n[STEP 2] Building counter contract from public state");

  // Define the Counter Contract account id from counter contract deploy
  const counterContractId = AccountId.fromHex(
    "0xb32d619dfe9e2f0000010ecb441d3f",
  );

  // Import the counter contract
  let counterContractAccount = await client.getAccount(counterContractId);
  if (!counterContractAccount) {
    await client.importAccountById(counterContractId);
    await client.syncState();
    counterContractAccount = await client.getAccount(counterContractId);
    if (!counterContractAccount) {
      throw new Error(`Account not found after import: ${counterContractId}`);
    }
  }
  console.log(
    "Account storage slot 0:",
    counterContractAccount.storage().getItem(0)?.toHex(),
  );

  // -------------------------------------------------------------------------
  // STEP 3: Call the Counter Contract via Foreign Procedure Invocation (FPI)
  // -------------------------------------------------------------------------
  console.log(
    "\n[STEP 3] Call counter contract with FPI from count reader contract",
  );

  // Counter contract code
  const counterContractCode = `
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
  `;

  // Create the counter contract component to get the procedure hash
  let counterContractComponent = AccountComponent.compile(
    counterContractCode,
    assembler,
    [],
  ).withSupportsAllTypes();

  let getCountProcHash = counterContractComponent.getProcedureHash("get_count");

  console.log("get count hash:", getCountProcHash);
  console.log("counter id prefix:", counterContractAccount.id().prefix());
  console.log("suffix:", counterContractAccount.id().suffix());

  // Build the script that calls the count reader contract
  let fpiScriptCode = `
    use.external_contract::count_reader_contract
    use.std::sys

    begin
        push.${getCountProcHash}
        # => [GET_COUNT_HASH]

        push.${counterContractAccount.id().suffix()}
        # => [account_id_suffix, GET_COUNT_HASH]

        push.${counterContractAccount.id().prefix()}
        # => [account_id_prefix, account_id_suffix, GET_COUNT_HASH]

        call.count_reader_contract::copy_count
        # => []

        exec.sys::truncate_stack
        # => []

    end
  `;

  // Empty inputs to the transaction script
  const inputs = new TransactionScriptInputPairArray();

  // Create the library for the count reader contract
  let countReaderLib = AssemblerUtils.createAccountComponentLibrary(
    assembler,
    "external_contract::count_reader_contract",
    countReaderCode,
  );

  // Compile the transaction script with the count reader library
  let txScript = TransactionScript.compile(
    fpiScriptCode,
    inputs,
    assembler.withLibrary(countReaderLib),
  );

  // Create foreign account for the counter contract
  let storageRequirements = new AccountStorageRequirements();
  let foreignAccount = ForeignAccount.public(
    counterContractId,
    storageRequirements,
  );

  // Build a transaction request with the custom script
  let txRequest = new TransactionRequestBuilder()
    .withCustomScript(txScript)
    .withForeignAccounts([foreignAccount])
    .build();

  // Execute the transaction locally on the count reader contract
  let txResult = await client.newTransaction(
    countReaderContract.account.id(),
    txRequest,
  );

  console.log(
    "View transaction on MidenScan: https://testnet.midenscan.com/tx/" +
      txResult.executedTransaction().id().toHex(),
  );

  // Submit transaction to the network
  await client.submitTransaction(txResult);
  await client.syncState();

  // Retrieve updated contract data to see the results
  let updatedCounterContract = await client.getAccount(
    counterContractAccount.id(),
  );
  console.log(
    "counter contract storage:",
    updatedCounterContract?.storage().getItem(0)?.toHex(),
  );

  let updatedCountReaderContract = await client.getAccount(
    countReaderContract.account.id(),
  );
  console.log(
    "count reader contract storage:",
    updatedCountReaderContract?.storage().getItem(0)?.toHex(),
  );

  // Log the count value copied via FPI
  let countReaderStorage = updatedCountReaderContract?.storage().getItem(0);
  if (countReaderStorage) {
    const countValue = Number(
      BigInt(
        "0x" +
          countReaderStorage
            .toHex()
            .slice(-16)
            .match(/../g)!
            .reverse()
            .join(""),
      ),
    );
    console.log("Count copied via Foreign Procedure Invocation:", countValue);
  }

  console.log("\nForeign Procedure Invocation Transaction completed!");
}
```

To run the code above in our frontend, run the following command:

```bash
pnpm run dev
```

Open the browser console and click the button "Foreign Procedure Invocation Tutorial".

This is what you should see in the browser console:

```
Current block number:  2168

[STEP 1] Creating count reader contract.
Count reader contract ID: 0x90128b4e27f34500000720bedaa49b

[STEP 2] Building counter contract from public state
Account storage slot 0: 0x0000000000000000000000000000000000000000000000001200000000000000

[STEP 3] Call counter contract with FPI from count reader contract
fpiScript
    use.external_contract::count_reader_contract
    use.std::sys

    begin
        push.0x92495ca54d519eb5e4ba22350f837904d3895e48d74d8079450f19574bb84cb6
        # => [GET_COUNT_HASH]

        push.297741160627968
        # => [account_id_suffix, GET_COUNT_HASH]

        push.12911083037950619392
        # => [account_id_prefix, account_id_suffix, GET_COUNT_HASH]

        call.count_reader_contract::copy_count
        # => []

        exec.sys::truncate_stack
        # => []

    end
View transaction on MidenScan: https://testnet.midenscan.com/tx/0xffff3dc5454154d1ccf64c1ad170bdef2df471c714f6fe6ab542d060396b559f
counter contract storage: 0x0000000000000000000000000000000000000000000000001200000000000000
count reader contract storage: 0x0000000000000000000000000000000000000000000000001200000000000000
Count copied via Foreign Procedure Invocation: 18

Foreign Procedure Invocation Transaction completed!
```

## Understanding the Count Reader Contract

The count reader smart contract contains a [`copy_count`](../../masm/accounts/count_reader.masm:50) procedure that uses [`tx::execute_foreign_procedure`](../../masm/accounts/count_reader.masm:51) to call the [`get_count`](../../masm/accounts/counter.masm:137) procedure in the counter contract.

```masm
use.miden::account
use.miden::tx
use.std::sys

# => [account_id_prefix, account_id_suffix, get_count_proc_hash]
export.copy_count
    exec.tx::execute_foreign_procedure
    # => [count]

    debug.stack
    # => [count]

    push.0
    # [index, count]

    exec.account::set_item
    # => []

    push.1 exec.account::incr_nonce
    # => []

    exec.sys::truncate_stack
    # => []
end
```

To call the [`get_count`](../../masm/accounts/counter.masm:137) procedure, we push its hash along with the counter contract's ID suffix and prefix onto the stack before calling [`tx::execute_foreign_procedure`](../../masm/accounts/count_reader.masm:51).

The stack state before calling [`tx::execute_foreign_procedure`](../../masm/accounts/count_reader.masm:51) should look like this:

```
# => [account_id_prefix, account_id_suffix, GET_COUNT_HASH]
```

After calling the [`get_count`](../../masm/accounts/counter.masm:137) procedure in the counter contract, we save the count of the counter contract to index 0 in storage.

## Understanding the Transaction Script

The transaction script that executes the foreign procedure invocation looks like this:

```masm
use.external_contract::count_reader_contract
use.std::sys

begin
    push.${getCountProcHash}
    # => [GET_COUNT_HASH]

    push.${counterContractAccount.id().suffix()}
    # => [account_id_suffix, GET_COUNT_HASH]

    push.${counterContractAccount.id().prefix()}
    # => [account_id_prefix, account_id_suffix, GET_COUNT_HASH]

    call.count_reader_contract::copy_count
    # => []

    exec.sys::truncate_stack
    # => []
end
```

This script:

1. Pushes the procedure hash of the [`get_count`](../../masm/accounts/counter.masm:137) function
2. Pushes the counter contract's account ID suffix and prefix
3. Calls the [`copy_count`](../../masm/accounts/count_reader.masm:50) procedure in our count reader contract
4. Truncates the stack

## Key WebClient Concepts for FPI

### Getting Procedure Hashes

In the WebClient, we get the procedure hash using the [`getProcedureHash`](../../web-client/lib/foreignProcedureInvocation.ts:183) method:

```ts
let getCountProcHash = counterContractComponent.getProcedureHash("get_count");
```

### Foreign Accounts

To execute foreign procedure calls, we need to specify the foreign account in our transaction request:

```ts
let foreignAccount = ForeignAccount.public(
  counterContractId,
  storageRequirements,
);

let txRequest = new TransactionRequestBuilder()
  .withCustomScript(txScript)
  .withForeignAccounts([foreignAccount])
  .build();
```

### Account Component Libraries

We create a library for the count reader contract so our transaction script can call its procedures:

```ts
let countReaderLib = AssemblerUtils.createAccountComponentLibrary(
  assembler,
  "external_contract::count_reader_contract",
  countReaderCode,
);
```

## Summary

In this tutorial we created a smart contract that calls the [`get_count`](../../masm/accounts/counter.masm:137) procedure in the counter contract using foreign procedure invocation, and then saves the returned value to its local storage using the Miden WebClient.

The key steps were:

1. Creating a count reader contract with a [`copy_count`](../../masm/accounts/count_reader.masm:50) procedure
2. Importing the counter contract from the network
3. Getting the procedure hash for the [`get_count`](../../masm/accounts/counter.masm:137) function
4. Building a transaction script that calls our count reader contract
5. Executing the transaction with a foreign account reference

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

### Continue learning

Next tutorial: [Creating Multiple Notes](creating_multiple_notes_tutorial.md)
