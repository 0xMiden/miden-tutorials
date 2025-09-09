import endGameNoteCode from "./notes/end_game_code";
import gameContractCode from "./contracts/tic_tac_toe_code";
import { NODE_URL } from "./constants";
import { TransactionRequest } from "@demox-labs/miden-sdk";

// lib/endGame.ts
export async function getEndGameTransactionRequest(
  gameContractIdBech32: string,
  connectedWalletId: string,
  playerSlot: bigint,
): Promise<TransactionRequest | null> {
  if (typeof window === "undefined") {
    console.warn("webClient() can only run in the browser");
    return null;
  }

  // dynamic import → only in the browser, so WASM is loaded client‑side
  const {
    AccountId,
    AssemblerUtils,
    TransactionKernel,
    NoteInputs,
    NoteMetadata,
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

  const client = await WebClient.createClient(NODE_URL);
  const state = await client.syncState();
  console.log("Current block number: ", state.blockNum());

  // Building the tic tac toe contract
  let assembler = TransactionKernel.assembler();

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

  assembler = assembler.withDebugMode(true).withLibrary(gameComponentLib);

  const noteScript = assembler.compileNoteScript(endGameNoteCode);

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
    AccountId.fromBech32(connectedWalletId),
    NoteType.Public,
    noteTag,
    NoteExecutionHint.always(),
    new Felt(BigInt(0)),
  );
  const endGameNote = new Note(emptyAssets, metadata, recipient);

  const noteRequest = new TransactionRequestBuilder()
    .withOwnOutputNotes(new OutputNotesArray([OutputNote.full(endGameNote)]))
    .build();

  return noteRequest;
}

export function generateRandomSerialNumber(): bigint[] {
  return [
    BigInt(Math.floor(Math.random() * 0x1_0000_0000)),
    BigInt(Math.floor(Math.random() * 0x1_0000_0000)),
    BigInt(Math.floor(Math.random() * 0x1_0000_0000)),
    BigInt(Math.floor(Math.random() * 0x1_0000_0000)),
  ];
}
