/**
 * Demonstrates unauthenticated note transfer chain using a delegated prover on the Miden Network
 * Creates a chain of P2ID (Pay to ID) notes: Alice → wallet 1 → wallet 2 → wallet 3 → wallet 4
 *
 * @throws {Error} If the function cannot be executed in a browser environment
 */
export async function unauthenticatedNoteTransfer(): Promise<void> {
  // Ensure this runs only in a browser context
  if (typeof window === 'undefined') return console.warn('Run in browser');

  const {
    WebClient,
    AccountStorageMode,
    NoteType,
    TransactionProver,
    Note,
    NoteAssets,
    OutputNoteArray,
    Felt,
    FungibleAsset,
    NoteAndArgsArray,
    NoteAndArgs,
    TransactionRequestBuilder,
    OutputNote,
  } = await import('@demox-labs/miden-sdk');

  const client = await WebClient.createClient('https://rpc.testnet.miden.io');
  const prover = TransactionProver.newRemoteProver(
    'https://tx-prover.testnet.miden.io',
  );

  console.log('Latest block:', (await client.syncState()).blockNum());

  // ── Creating new account ──────────────────────────────────────────────────────
  console.log('Creating accounts');

  console.log('Creating account for Alice…');
  const alice = await client.newWallet(AccountStorageMode.public(), true, 0);
  console.log('Alice accout ID:', alice.id().toString());

  const wallets = [];
  for (let i = 0; i < 5; i++) {
    const wallet = await client.newWallet(AccountStorageMode.public(), true, 0);
    wallets.push(wallet);
    console.log('wallet ', i.toString(), wallet.id().toString());
  }

  // ── Creating new faucet ──────────────────────────────────────────────────────
  const faucet = await client.newFaucet(
    AccountStorageMode.public(),
    false,
    'MID',
    8,
    BigInt(1_000_000),
    0,
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
  }

  console.log('Waiting for settlement');
  await new Promise((r) => setTimeout(r, 7_000));
  await client.syncState();

  // ── Consume the freshly minted note ──────────────────────────────────────────────
  const noteIds = (await client.getConsumableNotes(alice.id())).map((rec) =>
    rec.inputNoteRecord().id().toString(),
  );

  {
    const txResult = await client.executeTransaction(
      alice.id(),
      client.newConsumeTransactionRequest(noteIds),
    );
    const proven = await client.proveTransaction(txResult, prover);
    const submissionHeight = await client.submitProvenTransaction(
      proven,
      txResult,
    );
    await client.applyTransaction(txResult, submissionHeight);
    await client.syncState();
  }

  // ── Create unauthenticated note transfer chain ─────────────────────────────────────────────
  // Alice → wallet 1 → wallet 2 → wallet 3 → wallet 4
  for (let i = 0; i < wallets.length; i++) {
    console.log(`\nUnauthenticated tx ${i + 1}`);

    // Determine sender and receiver for this iteration
    const sender = i === 0 ? alice : wallets[i - 1];
    const receiver = wallets[i];

    console.log('Sender:', sender.id().toString());
    console.log('Receiver:', receiver.id().toString());

    const assets = new NoteAssets([new FungibleAsset(faucet.id(), BigInt(50))]);
    const p2idNote = Note.createP2IDNote(
      sender.id(),
      receiver.id(),
      assets,
      NoteType.Public,
      new Felt(BigInt(0)), // aux value
    );

    const outputP2ID = OutputNote.full(p2idNote);

    console.log('Creating P2ID note...');
    {
      const txResult = await client.executeTransaction(
        sender.id(),
        new TransactionRequestBuilder()
          .withOwnOutputNotes(new OutputNoteArray([outputP2ID]))
          .build(),
      );
      const proven = await client.proveTransaction(txResult, prover);
      const submissionHeight = await client.submitProvenTransaction(
        proven,
        txResult,
      );
      await client.applyTransaction(txResult, submissionHeight);
    }

    console.log('Consuming P2ID note...');

    const noteIdAndArgs = new NoteAndArgs(p2idNote, null);

    const consumeRequest = new TransactionRequestBuilder()
      .withUnauthenticatedInputNotes(new NoteAndArgsArray([noteIdAndArgs]))
      .build();

    {
      const txResult = await client.executeTransaction(
        receiver.id(),
        consumeRequest,
      );
      const proven = await client.proveTransaction(txResult, prover);
      const submissionHeight = await client.submitProvenTransaction(
        proven,
        txResult,
      );
      const txExecutionResult = await client.applyTransaction(
        txResult,
        submissionHeight,
      );

      const txId = txExecutionResult
        .executedTransaction()
        .id()
        .toHex()
        .toString();

      console.log(
        `Consumed Note Tx on MidenScan: https://testnet.midenscan.com/tx/${txId}`,
      );
    }
  }

  console.log('Asset transfer chain completed ✅');
}
