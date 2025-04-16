## Creating Accounts and Deploying Faucets

_Using the Miden WebClient in TypeScript to create accounts and deploy faucets with Next.js_

## Overview

In this tutorial, we’ll build a simple Next.js application that interacts with Miden using the Miden WebClient. We’ll:

1. Create a Miden account for _Alice_
2. Deploy a fungible faucet
3. (Later) Mint tokens from that faucet to fund Alice’s account and send tokens to other Miden accounts

## What we'll cover

- Understanding public vs. private accounts & notes
- Instantiating the Miden client
- Creating new accounts (public or private)
- Deploying a faucet to fund an account

## Prerequisites

- Node v20 or greater
- A terminal with npm or yarn

## Public vs. private accounts & notes

Before we dive into code, a quick refresher:

- **Public accounts**: on‑chain, fully visible (code & state).
- **Private accounts**: off‑chain state & logic, known only to the owner.
- **Public notes**: UTXO‑style tokens, anyone can see.
- **Private notes**: off‑chain UTXO, must share data out‑of‑band (e.g., email) to consume.

> _Think of notes as “cryptographic cashier’s checks.” Private notes keep amounts and ownership hidden._

---

## Step 1: Initialize your Next.js project

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
   npm install @demox-labs/miden-sdk@0.8.2
   ```

---

## Step 2: Instantiate the WebClient

### Create `lib/webClient.ts`

In the project root, create a folder `lib/` and inside it `webClient.ts`:

```ts
// lib/webClient.ts
export async function webClient(): Promise<void> {
  if (typeof window === "undefined") {
    console.warn("webClient() can only run in the browser");
    return;
  }

  const { WebClient, AccountStorageMode } = await import(
    "@demox-labs/miden-sdk"
  );

  const nodeEndpoint = "https://rpc.testnet.miden.io:443";
  const client = await WebClient.createClient(nodeEndpoint);

  // 1. Sync and log block
  const state = await client.syncState();
  console.log("Latest block number:", state.blockNum());
}
```

> To instantiate the WebClient, pass in the endpoint of the Miden node.

> Since we will be handling proof generation in the browser, it will be slower than proof generation handled by the Rust client. Check out the tutorial on how to use delegated proving in the browser to speed up proof generation.

---

## Step 3: Edit the `app/page.tsx` file:

Edit `app/page.tsx` to call `webClient()` on a button click:

```tsx
// app/page.tsx
"use client";
import { useState } from "react";
import { webClient } from "../lib/webClient";

export default function Home() {
  const [started, setStarted] = useState(false);

  const handleClick = async () => {
    setStarted(true);
    await webClient();
    setStarted(false);
  };

  return (
    <main style={{ padding: 20, textAlign: "center" }}>
      <h1>Miden Web App</h1>
      <p>Open your browser console to see WebClient logs.</p>
      <button
        onClick={handleClick}
        style={{
          padding: "10px 20px",
          fontSize: 16,
          cursor: "pointer",
          background: "transparent",
          border: "1px solid currentColor",
          borderRadius: "9999px",
        }}
      >
        {started ? "Working..." : "Start WebClient"}
      </button>
    </main>
  );
}
```

---

## Step 4: Create a wallet for Alice

Back in `lib/webClient.ts`, extend `webClient()`:

```ts
// lib/webClient.ts
export async function webClient(): Promise<void> {
  if (typeof window === "undefined") {
    console.warn("webClient() can only run in the browser");
    return;
  }

  // dynamic import → only in the browser, so WASM is loaded client‑side
  const { WebClient, AccountStorageMode } = await import(
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

  await client.syncState();
}
```

---

## Step 5: Deploy a fungible faucet

Append this to the end of `webClient()`:

```ts
// 4. Deploy faucet
console.log("Creating faucet…");
const faucetAccount = await client.newFaucet(
  AccountStorageMode.public(), // public faucet
  false, // immutable
  "MID", // token symbol
  8, // decimals
  BigInt(1_000_000), // max supply
);
console.log("Faucet account ID:", faucetAccount.id().toString());

await client.syncState();
console.log("Setup complete.");
```

> Every batch minted is a “note”—think of it as a UTXO with spend conditions.

---

## Summary

Your final `lib/webClient.ts` should look like:

```ts
// lib/webClient.ts
export async function webClient(): Promise<void> {
  if (typeof window === "undefined") {
    console.warn("webClient() can only run in the browser");
    return;
  }

  // dynamic import → only in the browser, so WASM is loaded client‑side
  const { WebClient, AccountStorageMode } = await import(
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
  console.log("Setup complete.");
}
```

---

### Running the example

```bash
cd miden-web-app
npm i
npm run dev
```

Open [http://localhost:3000](http://localhost:3000) in your browser, click **Start WebClient**, and check the console:

```
Latest block: 2247
Alice ID: 0xd70b2072c6495d100000869a8bacf2
Faucet ID: 0x2d7e506fb88dde200000a1386efec8
Setup complete.
```

---

Next up: **Minting tokens** from the faucet, **consuming notes**, and **sending tokens** to other accounts!
