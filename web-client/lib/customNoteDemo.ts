// lib/customNoteDemo.ts

/**
 * Hash Preimage Note Script for Miden Network
 * This note can only be consumed by providing the correct secret preimage
 */
const HASH_PREIMAGE_NOTE_SCRIPT = `
use.miden::note
use.miden::contracts::wallets::basic->wallet

# CONSTANTS
# =================================================================================================

const.EXPECTED_DIGEST_PTR=0
const.ASSET_PTR=100

# ERRORS
# =================================================================================================

const.ERROR_DIGEST_MISMATCH="Expected digest does not match computed digest"

#! Inputs (arguments):  [HASH_PREIMAGE_SECRET]
#! Outputs: []
#!
#! Note inputs are assumed to be as follows:
#!  => EXPECTED_DIGEST
begin
    # => HASH_PREIMAGE_SECRET
    # Hashing the secret number
    hash
    # => [DIGEST]

    # Writing the note inputs to memory
    push.EXPECTED_DIGEST_PTR exec.note::get_inputs drop drop

    # Pad stack and load expected digest from memory
    padw push.EXPECTED_DIGEST_PTR mem_loadw
    # => [EXPECTED_DIGEST, DIGEST]

    # Assert that the note input matches the digest
    # Will fail if the two hashes do not match
    assert_eqw.err=ERROR_DIGEST_MISMATCH
    # => []

    # ---------------------------------------------------------------------------------------------
    # If the check is successful, we allow for the asset to be consumed
    # ---------------------------------------------------------------------------------------------

    # Write the asset in note to memory address ASSET_PTR
    push.ASSET_PTR exec.note::get_assets
    # => [num_assets, dest_ptr]

    drop
    # => [dest_ptr]

    # Load asset from memory
    mem_loadw
    # => [ASSET]

    # Call receive asset in wallet
    call.wallet::receive_asset
    # => []
end
`;

/**
 * Demonstrates creating and consuming a custom note with hash preimage authentication
 */
export async function customNoteDemo(): Promise<void> {
  if (typeof window === "undefined") {
    console.warn("customNoteDemo() can only run in the browser");
    return;
  }

  // Dynamic import → only in the browser, so WASM is loaded client‑side
  const {
    WebClient,
    AccountStorageMode,
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
    Rpo256,
  } = await import("@demox-labs/miden-sdk");

  const nodeEndpoint = "https://rpc.testnet.miden.io:443";
  const client = await WebClient.createClient(nodeEndpoint);
  const prover = TransactionProver.newRemoteProver(
    "https://tx-prover.testnet.miden.io",
  );

  console.log("Current block number: ", (await client.syncState()).blockNum());

  // ── Step 1: Create accounts ──────────────────────────────────────────────────────
  console.log("\n[STEP 1] Creating accounts");

  console.log("Creating Alice's account...");
  const alice = await client.newWallet(AccountStorageMode.public(), true);
  console.log("Alice account ID:", alice.id().toString());

  console.log("Creating Bob's account...");
  const bob = await client.newWallet(AccountStorageMode.public(), true);
  console.log("Bob account ID:", bob.id().toString());

  // ── Step 2: Create faucet and mint tokens ──────────────────────────────────────────────────────
  console.log("\n[STEP 2] Creating faucet and minting tokens");

  const faucet = await client.newFaucet(
    AccountStorageMode.public(),
    false,
    "MID",
    8,
    BigInt(1_000_000),
  );
  console.log("Faucet ID:", faucet.id().toString());

  // Mint 100 MID to Alice
  await client.submitTransaction(
    await client.newTransaction(
      faucet.id(),
      client.newMintTransactionRequest(
        alice.id(),
        faucet.id(),
        NoteType.Public,
        BigInt(100),
      ),
    ),
    prover,
  );

  console.log("Waiting for settlement...");
  await new Promise((r) => setTimeout(r, 7_000));
  await client.syncState();

  // Consume the freshly minted note
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

  // ── Step 3: Create custom note with hash preimage ──────────────────────────────────────────────
  console.log("\n[STEP 3] Creating custom note with hash preimage");

  // Define the secret values (4 field elements)
  const secretValues = [
    new Felt(BigInt(1)),
    new Felt(BigInt(2)),
    new Felt(BigInt(3)),
    new Felt(BigInt(4)),
  ];

  console.log("Secret values:", secretValues.map(f => f.toString()));

  // Compile the note script
  const noteScript = client.compileNoteScript(HASH_PREIMAGE_NOTE_SCRIPT);

  // Create note inputs with the expected secret
  const secretHashed = Rpo256.hashElements(new FeltArray(secretValues)).toWord();

  

  const noteInputs = new NoteInputs(new FeltArray())

  // Create the note assets (50 MID tokens)
  const assets = new NoteAssets([new FungibleAsset(faucet.id(), BigInt(50))]);

  // Create note metadata
  const metadata = new NoteMetadata(
    alice.id(),
    NoteType.Public,
    NoteTag.fromAccountId(alice.id()),
    NoteExecutionHint.always(),
  );

  // Generate a random serial number for the note
  const serialNumber = Word.newFromFelts([
    new Felt(BigInt(Math.floor(Math.random() * 0x1_0000_0000))),
    new Felt(BigInt(Math.floor(Math.random() * 0x1_0000_0000))),
    new Felt(BigInt(Math.floor(Math.random() * 0x1_0000_0000))),
    new Felt(BigInt(Math.floor(Math.random() * 0x1_0000_0000))),
  ]);

  // Create the custom note
  const customNote = new Note(
    assets,
    metadata,
    new NoteRecipient(serialNumber, noteScript, noteInputs),
  );

  console.log("Custom note ID:", customNote.id().toString());

  // Create transaction to output the custom note
  const createNoteTransaction = await client.newTransaction(
    alice.id(),
    new TransactionRequestBuilder()
      .withOwnOutputNotes(new OutputNotesArray([OutputNote.full(customNote)]))
      .build(),
  );

  await client.submitTransaction(createNoteTransaction, prover);

  const createTxId = createNoteTransaction
    .executedTransaction()
    .id()
    .toHex()
    .toString();

  console.log(
    `Custom note created! View on MidenScan: https://testnet.midenscan.com/tx/${createTxId}`,
  );

  // ── Step 4: Consume the custom note ──────────────────────────────────────────────
  console.log("\n[STEP 4] Bob consumes the custom note with correct secret");

  // Create the secret as a Word (4 field elements) - the preimage that will be hashed
  const secretWord = Word.newFromFelts(secretValues);

  // Create note and args for consumption
  const noteAndArgs = new NoteAndArgs(customNote, secretWord);

  // Create transaction request to consume the custom note
  const consumeRequest = new TransactionRequestBuilder()
    .withUnauthenticatedInputNotes(new NoteAndArgsArray([noteAndArgs]))
    .build();

  // Execute the consumption transaction
  const consumeTransaction = await client.newTransaction(
    bob.id(),
    consumeRequest,
  );

  await client.submitTransaction(consumeTransaction, prover);

  const consumeTxId = consumeTransaction
    .executedTransaction()
    .id()
    .toHex()
    .toString();

  console.log(
    `Custom note consumed! View on MidenScan: https://testnet.midenscan.com/tx/${consumeTxId}`,
  );

  // Show the account delta (what Bob received)
  const accountDelta = consumeTransaction.accountDelta();
  console.log("Bob's account delta:", accountDelta.toString());

  console.log("\n✅ Custom note demo completed successfully!");
  console.log("Bob successfully consumed the note by providing the correct secret preimage.");
}