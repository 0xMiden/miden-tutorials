import makeMoveNoteCode from "./notes/make_a_move_code";
import gameContractCode from "./contracts/tic_tac_toe_code";

// lib/makeMove.ts
export async function readBoard(gameContractIdBech32: string): Promise<void> {
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

  const nodeEndpoint = "http://localhost:57291";
  const client = await WebClient.createClient(nodeEndpoint);
  console.log("Current block number: ", (await client.syncState()).blockNum());

  // Generate alice and bob wallets
  const alice = await client.newWallet(AccountStorageMode.public(), true);
  const bob = await client.newWallet(AccountStorageMode.public(), true);

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

  const gameContractMapping = gameContractAccount.storage().getItem(4);

  // TODO: use RpoDigest to compute the board

  // TODO: integrate wallet SDK to submit make move transaction

  // Sync state
  await client.syncState();
}
