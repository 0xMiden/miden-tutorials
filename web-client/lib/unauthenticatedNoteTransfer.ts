/**
 * P2ID (Pay to ID) Note Script for Miden Network
 * Enables creating notes that can be received by specific account IDs
 */
const P2ID_NOTE_SCRIPT = `
use.miden::account
use.miden::note
use.miden::contracts::wallets::basic->wallet

const.ERR_P2ID_WRONG_NUMBER_OF_INPUTS="P2ID note expects exactly 2 note inputs"
const.ERR_P2ID_TARGET_ACCT_MISMATCH="P2ID's target account address and transaction address do not match"

proc.add_note_assets_to_account
    push.0 exec.note::get_assets
    mul.4 dup.1 add                 
    padw movup.5                    
    dup dup.6 neq                 
    while.true
        dup movdn.5                 
        mem_loadw                 
        padw swapw padw padw swapdw
        call.wallet::receive_asset
        dropw dropw dropw          
        movup.4 add.4 dup dup.6 neq
    end
    drop dropw drop
end

begin
    push.0 exec.note::get_inputs       
    eq.2 assert.err=ERR_P2ID_WRONG_NUMBER_OF_INPUTS
    padw movup.4 mem_loadw drop drop   
    exec.account::get_id               
    exec.account::is_id_equal assert.err=ERR_P2ID_TARGET_ACCT_MISMATCH
    exec.add_note_assets_to_account
end
`;

/**
 * Demonstrates unauthenticated note transfer chain using a delegated prover on the Miden Network
 * Creates a chain of P2ID (Pay to ID) notes: Alice → wallet 1 → wallet 2 → wallet 3 → wallet 4
 *
 * @throws {Error} If the function cannot be executed in a browser environment
 */
export async function unauthenticatedNoteTransfer(): Promise<void> {
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
    NoteAndArgsArray,
    NoteAndArgs,
    TransactionRequestBuilder,
    OutputNote,
  } = await import("@demox-labs/miden-sdk");

  const client = await WebClient.createClient(
    "https://rpc.testnet.miden.io:443",
  );
  const prover = TransactionProver.newRemoteProver(
    "https://tx-prover.testnet.miden.io",
  );

  console.log("Latest block:", (await client.syncState()).blockNum());

  // ── Creating new account ──────────────────────────────────────────────────────
  console.log("Creating accounts");

  console.log("Creating account for Alice…");
  const alice = await client.newWallet(AccountStorageMode.public(), true);
  console.log("Alice accout ID:", alice.id().toString());

  let wallets = [];
  for (let i = 0; i < 5; i++) {
    let wallet = await client.newWallet(AccountStorageMode.public(), true);
    wallets.push(wallet);
    console.log("wallet ", i.toString(), wallet.id().toString());
  }

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

  console.log("Waiting for settlement");
  await new Promise((r) => setTimeout(r, 7_000));
  await client.syncState();

  // ── Consume the freshly minted note ──────────────────────────────────────────────
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

  const script = client.compileNoteScript(P2ID_NOTE_SCRIPT);

  // ── Create unauthenticated note transfer chain ─────────────────────────────────────────────
  // Alice → wallet 1 → wallet 2 → wallet 3 → wallet 4
  for (let i = 0; i < wallets.length; i++) {
    console.log(`\nUnauthenticated tx ${i + 1}`);

    // Determine sender and receiver for this iteration
    const sender = i === 0 ? alice : wallets[i - 1];
    const receiver = wallets[i];

    console.log("Sender:", sender.id().toString());
    console.log("Receiver:", receiver.id().toString());

    const assets = new NoteAssets([new FungibleAsset(faucet.id(), BigInt(50))]);
    const metadata = new NoteMetadata(
      sender.id(),
      NoteType.Public,
      NoteTag.fromAccountId(sender.id(), NoteExecutionMode.newLocal()),
      NoteExecutionHint.always(),
    );

    let serialNumber = Word.newFromFelts([
      new Felt(BigInt(Math.floor(Math.random() * 0x1_0000_0000))),
      new Felt(BigInt(Math.floor(Math.random() * 0x1_0000_0000))),
      new Felt(BigInt(Math.floor(Math.random() * 0x1_0000_0000))),
      new Felt(BigInt(Math.floor(Math.random() * 0x1_0000_0000))),
    ]);

    const receiverAcct = AccountId.fromHex(receiver.id().toString());
    const inputs = new NoteInputs(
      new FeltArray([receiverAcct.suffix(), receiverAcct.prefix()]),
    );

    let p2idNote = new Note(
      assets,
      metadata,
      new NoteRecipient(serialNumber, script, inputs),
    );

    let outputP2ID = OutputNote.full(p2idNote);

    console.log("Creating P2ID note...");
    let transaction = await client.newTransaction(
      sender.id(),
      new TransactionRequestBuilder()
        .withOwnOutputNotes(new OutputNotesArray([outputP2ID]))
        .build(),
    );
    await client.submitTransaction(transaction, prover);

    console.log("Consuming P2ID note...");

    let noteIdAndArgs = new NoteAndArgs(p2idNote, null);

    let consumeRequest = new TransactionRequestBuilder()
      .withUnauthenticatedInputNotes(new NoteAndArgsArray([noteIdAndArgs]))
      .build();

    let txExecutionResult = await client.newTransaction(
      receiver.id(),
      consumeRequest,
    );

    await client.submitTransaction(txExecutionResult, prover);

    const txId = txExecutionResult
      .executedTransaction()
      .id()
      .toHex()
      .toString();

    console.log(
      `Consumed Note Tx on MidenScan: https://testnet.midenscan.com/tx/${txId}`,
    );
  }

  console.log("Asset transfer chain completed ✅");
}
