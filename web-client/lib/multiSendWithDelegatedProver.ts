/**
 * Demonstrates multi-send functionality using a local prover on the Miden Network
 * Creates multiple P2ID (Pay to ID) notes for different recipients
 *
 * @throws {Error} If the function cannot be executed in a browser environment
 */
export async function multiSendWithDelegatedProver(): Promise<void> {
  // Ensure this runs only in a browser context
  if (typeof window === 'undefined') return console.warn('Run in browser');

  const {
    WebClient,
    AccountStorageMode,
    AuthScheme,
    Address,
    NoteType,
    TransactionProver,
    Note,
    NoteAssets,
    OutputNoteArray,
    FungibleAsset,
    NoteAttachment,
    TransactionRequestBuilder,
    OutputNote,
  } = await import('@miden-sdk/miden-sdk');

  const client = await WebClient.createClient('https://rpc.devnet.miden.io');
  const prover = TransactionProver.newLocalProver();

  console.log('Latest block:', (await client.syncState()).blockNum());

  // ── Creating new account ──────────────────────────────────────────────────────
  console.log('Creating account for Alice…');
  const alice = await client.newWallet(
    AccountStorageMode.public(),
    true,
    AuthScheme.AuthRpoFalcon512,
  );
  console.log('Alice accout ID:', alice.id().toString());

  // ── Creating new faucet ──────────────────────────────────────────────────────
  const faucet = await client.newFaucet(
    AccountStorageMode.public(),
    false,
    'MID',
    8,
    BigInt(1_000_000),
    AuthScheme.AuthRpoFalcon512,
  );
  console.log('Faucet ID:', faucet.id().toString());

  // ── mint 10 000 MID to Alice ──────────────────────────────────────────────────────
  {
    const txResult = await client.executeTransaction(
      faucet.id(),
      client.newMintTransactionRequest(
        alice.id(),
        faucet.id(),
        NoteType.Public,
        BigInt(10_000),
      ),
    );
    const proven = await client.proveTransaction(txResult, prover);
    const submissionHeight = await client.submitProvenTransaction(
      proven,
      txResult,
    );
    await client.applyTransaction(txResult, submissionHeight);

    console.log('waiting for settlement');
    await new Promise((r) => setTimeout(r, 7_000));
    await client.syncState();
  }

  // ── consume the freshly minted notes ──────────────────────────────────────────────
  const noteList = (await client.getConsumableNotes(alice.id())).map((rec) =>
    rec.inputNoteRecord().toNote(),
  );

  {
    const txResult = await client.executeTransaction(
      alice.id(),
      client.newConsumeTransactionRequest(noteList),
    );
    const proven = await client.proveTransaction(txResult, prover);
    await client.syncState();
    const submissionHeight = await client.submitProvenTransaction(
      proven,
      txResult,
    );
    await client.applyTransaction(txResult, submissionHeight);
  }

  // ── build 3 P2ID notes (100 MID each) ─────────────────────────────────────────────
  const recipientAddresses = [
    'mtst1aqezqc90x7dkzypr9m5fmlpp85w6cl04',
    'mtst1apjg2ul76wrkxyr5qlcnczaskypa4ljn',
    'mtst1arpee6y9cm8t7ypn33pc8fzj6gkzz7kd',
  ];

  const assets = new NoteAssets([new FungibleAsset(faucet.id(), BigInt(100))]);

  const p2idNotes = recipientAddresses.map((addr) => {
    const receiverAccountId = Address.fromBech32(addr).accountId();
    const note = Note.createP2IDNote(
      alice.id(),
      receiverAccountId,
      assets,
      NoteType.Public,
      new NoteAttachment(),
    );

    return OutputNote.full(note);
  });

  // ── create all P2ID notes ───────────────────────────────────────────────────────────────
  await client.submitNewTransaction(
    alice.id(),
    new TransactionRequestBuilder()
      .withOwnOutputNotes(new OutputNoteArray(p2idNotes))
      .build(),
  );

  console.log('All notes created ✅');
}
