import makeMoveNoteCode from "./notes/make_a_move_code";
import gameContractCode from "./contracts/tic_tac_toe_code";
import { NODE_URL } from "./constants";
import {
  MidenTransaction,
  TransactionType,
  CustomTransaction,
} from "@demox-labs/miden-wallet-adapter";

// lib/makeMove.ts
export async function makeMove(
  gameContractIdString: string,
  connectedWalletIdString: string,
  requestTransaction: (transaction: MidenTransaction) => Promise<string>,
): Promise<string | null> {
  if (typeof window === "undefined") {
    console.warn("webClient() can only run in the browser");
    return null;
  }

  // Dynamic import to ensure client-side execution
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
    AccountInterface,
    NetworkId,
  } = await import("@demox-labs/miden-sdk");

  // Convert string IDs to AccountId objects
  const gameContractId = AccountId.fromBech32(gameContractIdString);
  const connectedWalletId = AccountId.fromBech32(connectedWalletIdString);

  // Create client instance
  const client = await WebClient.createClient(NODE_URL);
  const state = await client.syncState();
  console.log("Current block number: ", state.blockNum());

  // Building the tic tac toe contract
  let assembler = TransactionKernel.assembler();

  // Reading the public state of the tic tac toe contract from testnet,
  // and importing it into the WebClient
  let gameContractAccount = await client.getAccount(gameContractId);
  if (!gameContractAccount) {
    await client.importAccountById(gameContractId);
    await client.syncState();
    gameContractAccount = await client.getAccount(gameContractId);
    if (!gameContractAccount) {
      throw new Error(`Account not found after import: ${gameContractId}`);
    }
  }

  // Creating the library to call the counter contract
  const gameComponentLib = AssemblerUtils.createAccountComponentLibrary(
    assembler, // assembler
    "external_contract::game_contract", // library path to call the contract
    gameContractCode, // account code of the contract
  );

  assembler = assembler.withDebugMode(true).withLibrary(gameComponentLib);

  const noteScript = assembler.compileNoteScript(makeMoveNoteCode);

  const emptyAssets = new NoteAssets([]);
  const index: bigint = BigInt(5);
  const noteInputs = new NoteInputs(new FeltArray([new Felt(index)]));
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
    connectedWalletId,
    NoteType.Public,
    noteTag,
    NoteExecutionHint.always(),
    new Felt(BigInt(0)),
  );
  const makeMoveNote = new Note(emptyAssets, metadata, recipient);

  const noteRequest = new TransactionRequestBuilder()
    .withOwnOutputNotes(new OutputNotesArray([OutputNote.full(makeMoveNote)]))
    .build();

  const tx = new CustomTransaction(
    connectedWalletId.toBech32(NetworkId.Testnet, AccountInterface.BasicWallet),
    noteRequest,
  );

  const txId = await requestTransaction({
    type: TransactionType.Custom,
    payload: tx,
  });

  await client.syncState();

  return txId;
}

export function generateRandomSerialNumber(): bigint[] {
  return [
    BigInt(Math.floor(Math.random() * 0x1_0000_0000)),
    BigInt(Math.floor(Math.random() * 0x1_0000_0000)),
    BigInt(Math.floor(Math.random() * 0x1_0000_0000)),
    BigInt(Math.floor(Math.random() * 0x1_0000_0000)),
  ];
}
