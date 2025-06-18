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

  const client = await WebClient.createClient(
    "https://rpc.testnet.miden.io:443",
  );
  const prover = TransactionProver.newRemoteProver(
    "https://tx-prover.testnet.miden.io",
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

  const script = client.compileNoteScript(P2ID_NOTE_SCRIPT);

  const assets = new NoteAssets([new FungibleAsset(faucet.id(), BigInt(100))]);
  const metadata = new NoteMetadata(
    alice.id(),
    NoteType.Public,
    NoteTag.fromAccountId(alice.id(), NoteExecutionMode.newLocal()),
    NoteExecutionHint.always(),
  );

  const p2idNotes = recipientAddresses.map((addr) => {
    let serialNumber = Word.newFromFelts([
      new Felt(BigInt(Math.floor(Math.random() * 0x1_0000_0000))),
      new Felt(BigInt(Math.floor(Math.random() * 0x1_0000_0000))),
      new Felt(BigInt(Math.floor(Math.random() * 0x1_0000_0000))),
      new Felt(BigInt(Math.floor(Math.random() * 0x1_0000_0000))),
    ]);

    const acct = AccountId.fromHex(addr);
    const inputs = new NoteInputs(
      new FeltArray([acct.suffix(), acct.prefix()]),
    );

    let note = new Note(
      assets,
      metadata,
      new NoteRecipient(serialNumber, script, inputs),
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
