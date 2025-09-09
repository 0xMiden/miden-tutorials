import endGameNoteCode from "./notes/end_game_code";
import gameContractCode from "./contracts/tic_tac_toe_code";

// lib/makeMove.ts
export async function endGame(
  gameContractIdBech32: string,
  playerSlot: bigint,
): Promise<void> {
  if (typeof window === "undefined") {
    console.warn("webClient() can only run in the browser");
    return;
  }

  // dynamic import → only in the browser, so WASM is loaded client‑side
  const {
    AccountId,
    AssemblerUtils,
    AccountStorageMode,
    StorageSlot,
    TransactionKernel,
    NoteInputs,
    NoteMetadata,
    NoteScript,
    FeltArray,
    WebClient,
    NoteAssets,
    Felt,
    Word,
    NoteTag,
    NoteType,
    NoteExecutionMode,
    NoteExecutionHint,
    NoteRecipient,
    Note,
    OutputNote,
    OutputNotesArray,
    TransactionRequestBuilder,
  } = await import("@demox-labs/miden-sdk");

  const nodeEndpoint = "https://rpc.testnet.miden.io:443";
  const client = await WebClient.createClient(nodeEndpoint);
  console.log("Current block number: ", (await client.syncState()).blockNum());

  // Generate alice and bob wallets
  const alice = await client.newWallet(AccountStorageMode.public(), true);
  // TODO: replace with getting wallet from SDK

  // Building the tic tac toe contract
  const assembler = TransactionKernel.assembler();

  const gameContractId = AccountId.fromBech32(gameContractIdBech32);

  // Reading the public state of the tic tac toe contract from testnet,
  // and importing it into the WebClient
  let gameContractAccount = await client.getAccount(gameContractId);
  if (!gameContractAccount) {
    await client.importAccountById(gameContractId);
    await client.syncState();
    gameContractAccount = await client.getAccount(gameContractId);
    if (!gameContractAccount) {
      throw new Error(
        `Account not found after import: ${gameContractIdBech32}`,
      );
    }
  }

  // Creating the library to call the counter contract
  const gameComponentLib = AssemblerUtils.createAccountComponentLibrary(
    assembler, // assembler
    "external_contract::game_contract", // library path to call the contract
    gameContractCode, // account code of the contract
  );

  // const noteScript = NoteScript.compile(endGameNoteCode, gameComponentLib);
  const noteScript = client.compileNoteScript(endGameNoteCode);

  const emptyAssets = new NoteAssets([]);
  const noteInputs = new NoteInputs(new FeltArray([new Felt(playerSlot)]));
  const serialNumberValues = generateRandomSerialNumber();
  const serialNumber = Word.newFromFelts([
    new Felt(serialNumberValues[0]),
    new Felt(serialNumberValues[1]),
    new Felt(serialNumberValues[2]),
    new Felt(serialNumberValues[3]),
  ]);
  const recipient = new NoteRecipient(serialNumber, noteScript, noteInputs);
  const noteTag = NoteTag.forPublicUseCase(0, 0, NoteExecutionMode.newLocal());
  const metadata = new NoteMetadata(
    alice.id(), // TODO: replace with getting wallet from SDK
    NoteType.Public,
    noteTag,
    NoteExecutionHint.always(),
    new Felt(BigInt(0)),
  );
  const endGameNote = new Note(emptyAssets, metadata, recipient);

  const noteRequest = new TransactionRequestBuilder()
    .withOwnOutputNotes(new OutputNotesArray([OutputNote.full(endGameNote)]))
    .build();

  // TODO: integrate wallet SDK to submit make move transaction

  // Sync state
  await client.syncState();
}

export function generateRandomSerialNumber(): bigint[] {
  return [
    BigInt(Math.floor(Math.random() * 0x1_0000_0000)),
    BigInt(Math.floor(Math.random() * 0x1_0000_0000)),
    BigInt(Math.floor(Math.random() * 0x1_0000_0000)),
    BigInt(Math.floor(Math.random() * 0x1_0000_0000)),
  ];
}
