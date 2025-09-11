/**
 * Demonstrates multi-send functionality using a delegated prover on the Miden Network
 * Creates multiple P2ID (Pay to ID) notes for different recipients
 *
 * @throws {Error} If the function cannot be executed in a browser environment
 */
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

  const client = await WebClient.createClient("https://rpc.testnet.miden.io");
  const prover = TransactionProver.newRemoteProver(
    "https://tx-prover.devnet.miden.io",
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

  const assets = new NoteAssets([new FungibleAsset(faucet.id(), BigInt(100))]);

  const p2idNotes = recipientAddresses.map((addr) => {
    const receiverAccountId = AccountId.fromHex(addr);
    let note = Note.createP2IDNote(
      alice.id(),
      receiverAccountId,
      assets,
      NoteType.Public,
      new Felt(BigInt(0)),
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
