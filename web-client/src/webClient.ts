import {
  WebClient,
  AccountStorageMode,
  AccountId,
  NoteType,
} from "@demox-labs/miden-sdk";

const nodeEndpoint = "https://rpc.testnet.miden.io:443";

export async function webClient(): Promise<void> {
  try {
    // 1. Create client
    const client = await WebClient.createClient(nodeEndpoint);

    // 2. Sync and log block
    const state = await client.syncState();
    console.log("Latest block number:", state.blockNum());

    // 3. Create Alice account (public, updatable)
    console.log("Creating account for Alice");
    const aliceAccount = await client.newWallet(
      AccountStorageMode.public(),
      true,
    );
    const aliceIdHex = aliceAccount.id().toString();
    console.log("Alice's account ID:", aliceIdHex);

    // 4. Create faucet
    console.log("Creating faucet...");
    const faucetAccount = await client.newFaucet(
      AccountStorageMode.public(),
      false,
      "MID",
      8,
      BigInt(1_000_000),
    );
    const faucetIdHex = faucetAccount.id().toString();
    console.log("Faucet account ID:", faucetIdHex);

    // 5. Mint tokens to Alice
    await client.fetchAndCacheAccountAuthByAccountId(
      AccountId.fromHex(faucetIdHex),
    );
    await client.syncState();

    console.log("Minting tokens to Alice...");
    let mintTxRequest = client.newMintTransactionRequest(
      AccountId.fromHex(aliceIdHex),
      AccountId.fromHex(faucetIdHex),
      NoteType.public(),
      BigInt(1000),
    );

    let txResult = await client.newTransaction(
      faucetAccount.id(),
      mintTxRequest,
    );

    await client.submitTransaction(txResult);

    console.log("Waiting 15 seconds for transaction confirmation...");
    await new Promise((resolve) => setTimeout(resolve, 15000));
    await client.syncState();

    await client.fetchAndCacheAccountAuthByAccountId(
      AccountId.fromHex(aliceIdHex),
    );

    // 6. Fetch minted notes
    const mintedNotes = await client.getConsumableNotes(
      AccountId.fromHex(aliceIdHex),
    );
    const mintedNoteIds = mintedNotes.map((n) =>
      n.inputNoteRecord().id().toString(),
    );
    console.log("Minted note IDs:", mintedNoteIds);

    // 7. Consume minted notes
    console.log("Consuming minted notes...");
    let consumeTxRequest = client.newConsumeTransactionRequest(mintedNoteIds);

    let txResult_2 = await client.newTransaction(
      aliceAccount.id(),
      consumeTxRequest,
    );

    await client.submitTransaction(txResult_2);

    await client.syncState();
    console.log("Notes consumed.");

    // 8. Send tokens to a dummy account
    const dummyIdHex = "0x599a54603f0cf9000000ed7a11e379";
    console.log("Sending tokens to dummy account...");
    let sendTxRequest = client.newSendTransactionRequest(
      AccountId.fromHex(aliceIdHex),
      AccountId.fromHex(dummyIdHex),
      AccountId.fromHex(faucetIdHex),
      NoteType.public(),
      BigInt(100),
    );

    let txResult_3 = await client.newTransaction(
      aliceAccount.id(),
      sendTxRequest,
    );

    await client.submitTransaction(txResult_3);

    await client.syncState();
    console.log("Tokens sent.");
  } catch (error) {
    console.error("Error:", error);
    throw error;
  }
}
