---
title: 'Incrementing the Count of the Counter Contract'
sidebar_position: 5
---

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
- `yarn`

This tutorial assumes you have a basic understanding of Miden assembly. To quickly get up to speed with Miden assembly (MASM), please play around with running basic Miden assembly programs in the [Miden playground](https://0xmiden.github.io/examples/).

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
   yarn add @demox-labs/miden-sdk@0.12.3
   ```

**NOTE!**: Be sure to add the `--webpack` command to your `package.json` when running the `dev script`. The dev script should look like this:

`package.json`

```json
  "scripts": {
    "dev": "next dev --webpack",
    ...
  }
```

## Step 2: Edit the `app/page.tsx` file:

Add the following code to the `app/page.tsx` file. This code defines the main page of our web application:

```tsx
'use client';
import { useState } from 'react';
import { incrementCounterContract } from '../lib/incrementCounterContract';

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
              ? 'Working...'
              : 'Tutorial #3: Increment Counter Contract'}
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
  if (typeof window === 'undefined') {
    console.warn('webClient() can only run in the browser');
    return;
  }

  // dynamic import → only in the browser, so WASM is loaded client‑side
  const {
    AccountId,
    AccountBuilder,
    AccountComponent,
    AccountStorageMode,
    AccountType,
    SecretKey,
    StorageMap,
    StorageSlot,
    TransactionRequestBuilder,
    WebClient,
  } = await import('@demox-labs/miden-sdk');

  const nodeEndpoint = 'https://rpc.testnet.miden.io';
  const client = await WebClient.createClient(nodeEndpoint);
  console.log('Current block number: ', (await client.syncState()).blockNum());

  // Counter contract code in Miden Assembly
  const counterContractCode = `
  use.miden::active_account
  use miden::native_account
  use.std::sys

  const.COUNTER_SLOT=0

  #! Inputs:  []
  #! Outputs: [count]
  export.get_count
      push.COUNTER_SLOT
      # => [index]

      exec.active_account::get_item
      # => [count]

      # clean up stack
      movdn.4 dropw
      # => [count]
  end

  #! Inputs:  []
  #! Outputs: []
  export.increment_count
      push.COUNTER_SLOT
      # => [index]

      exec.active_account::get_item
      # => [count]

      add.1
      # => [count+1]

      debug.stack

      push.COUNTER_SLOT
      # [index, count+1]

      exec.native_account::set_item
      # => [OLD_VALUE]

      dropw
      # => []
  end
`;

  // Building the counter contract
  // Counter contract account id on testnet
  const counterContractId = AccountId.fromHex(
    '0xe59d8cd3c9ff2a0055da0b83ed6432',
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

  const builder = client.createScriptBuilder();
  const storageMap = new StorageMap();
  const storageSlotMap = StorageSlot.map(storageMap);

  const mappingAccountComponent = AccountComponent.compile(
    counterContractCode,
    builder,
    [storageSlotMap],
  ).withSupportsAllTypes();

  const walletSeed = new Uint8Array(32);
  crypto.getRandomValues(walletSeed);

  const secretKey = SecretKey.rpoFalconWithRNG(walletSeed);
  const authComponent = AccountComponent.createAuthComponent(secretKey);

  const accountBuilderResult = new AccountBuilder(walletSeed)
    .accountType(AccountType.RegularAccountImmutableCode)
    .storageMode(AccountStorageMode.public())
    .withAuthComponent(authComponent)
    .withComponent(mappingAccountComponent)
    .build();

  await client.addAccountSecretKeyToWebStore(secretKey);
  await client.newAccount(accountBuilderResult.account, false);

  await client.syncState();

  const accountCodeLib = builder.buildLibrary(
    'external_contract::counter_contract',
    counterContractCode,
  );

  builder.linkDynamicLibrary(accountCodeLib);

  // Building the transaction script which will call the counter contract
  const txScriptCode = `
use.external_contract::counter_contract
begin
call.counter_contract::increment_count
end
`;

  const txScript = builder.compileTxScript(txScriptCode);
  const txIncrementRequest = new TransactionRequestBuilder()
    .withCustomScript(txScript)
    .build();

  // Executing the transaction script against the counter contract
  await client.submitNewTransaction(
    counterContractAccount.id(),
    txIncrementRequest,
  );

  // Sync state
  await client.syncState();

  // Logging the count of counter contract
  const counter = await client.getAccount(counterContractAccount.id());

  // Here we get the first Word from storage of the counter contract
  // A word is comprised of 4 Felts, 2**64 - 2**32 + 1
  const count = counter?.storage().getItem(0);

  // Converting the Word represented as a hex to a single integer value
  const counterValue = Number(
    BigInt('0x' + count!.toHex().slice(-16).match(/../g)!.reverse().join('')),
  );

  console.log('Count: ', counterValue);
}
```

To run the code above in our frontend, run the following command:

```
yarn dev
```

Open the browser console and click the button "Increment Counter Contract".

This is what you should see in the browser console:

```
Current block number:  2168
incrementCounterContract.ts:153 Count:  3
```

## Miden Assembly Counter Contract Explainer

#### Here's a breakdown of what the `get_count` procedure does:

1. Pushes `0` (COUNTER_SLOT) onto the stack, representing the index of the storage slot to read.
2. Calls `account::get_item` with the index of `0`.
3. Calls `sys::truncate_stack` to truncate the stack to size 16.
4. The value returned from `account::get_item` is still on the stack and will be returned when this procedure is called.

#### Here's a breakdown of what the `increment_count` procedure does:

1. Pushes `0` (COUNTER_SLOT) onto the stack, representing the index of the storage slot to read.
2. Calls `account::get_item` with the index of `0`.
3. Pushes `1` onto the stack.
4. Adds `1` to the count value returned from `account::get_item`.
5. _For demonstration purposes_, calls `debug.stack` to see the state of the stack
6. Pushes `0` (COUNTER_SLOT) onto the stack, which is the index of the storage slot we want to write to.
7. Calls `account::set_item` which saves the incremented count to storage at index `0`
8. Calls `sys::truncate_stack` to truncate the stack to size 16.

```masm
use.miden::active_account
use miden::native_account
use.std::sys

const.COUNTER_SLOT=0

#! Inputs:  []
#! Outputs: [count]
export.get_count
    push.COUNTER_SLOT
    # => [index]

    exec.active_account::get_item
    # => [count]

    # clean up stack
    movdn.4 dropw
    # => [count]
end

#! Inputs:  []
#! Outputs: []
export.increment_count
    push.COUNTER_SLOT
    # => [index]

    exec.active_account::get_item
    # => [count]

    add.1
    # => [count+1]

    debug.stack

    push.COUNTER_SLOT
    # [index, count+1]

    exec.native_account::set_item
    # => [OLD_VALUE]

    dropw
    # => []
end
```

**Note**: _It's a good habit to add comments below each line of MASM code with the expected stack state. This improves readability and helps with debugging._

### Authentication Component

**Important**: All accounts must have an authentication component. For smart contracts that do not require authentication (like our counter contract), we use a `NoAuth` component.

This `NoAuth` component allows any user to interact with the smart contract without requiring signature verification.

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
yarn install
yarn start
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
  console.log('All databases deleted.');
})();
```
