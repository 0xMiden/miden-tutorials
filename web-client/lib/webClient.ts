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
