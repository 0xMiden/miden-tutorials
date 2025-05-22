// ────────────────────────────────── P2ID NOTE SCRIPT ──────────────────────────────────
const P2ID_NOTE_SCRIPT = `
use.miden::account
use.miden::note
use.miden::contracts::wallets::basic->wallet

const.ERR_P2ID_WRONG_NUMBER_OF_INPUTS=0x0002c000
const.ERR_P2ID_TARGET_ACCT_MISMATCH=0x0002c001

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

// ───────────────────────── multiSendWithDelegatedProver ─────────────────────────
export async function multiSendWithDelegatedProver(): Promise<void> {
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
  const prover = TransactionProver.newRemoteProver("http://0.0.0.0:8082");

  console.log("Latest block:", (await client.syncState()).blockNum());

  const alice = await client.newWallet(AccountStorageMode.public(), true);
  const faucet = await client.newFaucet(
    AccountStorageMode.public(),
    false,
    "MID",
    8,
    BigInt(1_000_000),
  );
  console.log("Alice:", alice.id().toString());
  console.log("Faucet:", faucet.id().toString());

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
  await new Promise((r) => setTimeout(r, 10_000));
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
    "0x3477d1532b97101000006f79009dda",
    "0x13eb7f06cc675f20000d28ad4256e9",
    "0x3a2ff6a4b1628120000d8a3650894b",
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

  let transaction = await client.newTransaction(
    alice.id(),
    new TransactionRequestBuilder()
      .withOwnOutputNotes(new OutputNotesArray(p2idNotes))
      .build(),
  );

  // ── create all P2ID notes ───────────────────────────────────────────────────────────────
  await client.submitTransaction(transaction, prover);

  console.log("All notes created ✅");
}
