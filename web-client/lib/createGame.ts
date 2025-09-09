import gameContractCode from "./contracts/tic_tac_toe_code";

// lib/makeMove.ts
export async function createGame(): Promise<void> {
  if (typeof window === "undefined") {
    console.warn("webClient() can only run in the browser");
    return;
  }

  // dynamic import → only in the browser, so WASM is loaded client‑side
  const {
    AccountId,
    AssemblerUtils,
    AccountStorageMode,
    AccountComponent,
    AccountBuilder,
    AccountType,
    StorageSlot,
    StorageMap,
    TransactionKernel,
    TransactionRequestBuilder,
    TransactionScript,
    WebClient,
    Felt,
    Word,
  } = await import("@demox-labs/miden-sdk");

  const nodeEndpoint = "https://rpc.testnet.miden.io:443";
  const client = await WebClient.createClient(nodeEndpoint);
  console.log("Current block number: ", (await client.syncState()).blockNum());

  // Generate alice and bob wallets
  const alice = await client.newWallet(AccountStorageMode.public(), true);
  const bob = await client.newWallet(AccountStorageMode.public(), true);

  // Building the tic tac toe contract
  const assembler = TransactionKernel.assembler();

  const emptyStorageSlot = StorageSlot.fromValue(Word.newFromFelts([]));
  const storageMap = new StorageMap();
  const storageSlotMap = StorageSlot.map(storageMap);

  const gameComponent = AccountComponent.compile(gameContractCode, assembler, [
    // player1 storage slot
    emptyStorageSlot,
    // player2 storage slot
    emptyStorageSlot,
    // flag storage slot
    emptyStorageSlot,
    // winner storage slot
    emptyStorageSlot,
    // mapping storage slot
    storageSlotMap,
  ]).withSupportsAllTypes();

  let seed = new Uint8Array(32);
  seed = crypto.getRandomValues(seed);

  const gameContract = new AccountBuilder(seed)
    .accountType(AccountType.RegularAccountImmutableCode)
    .storageMode(AccountStorageMode.public())
    .withComponent(gameComponent)
    .withAuthComponent(NoAuth)
    .build();

  await client.newAccount(gameContract.account, gameContract.seed, false);

  // Building the transaction script which will call the counter contract
  const deploymentScriptCode = `
    use.external_contract::game_contract
    begin
        call.game_contract::constructor
    end
    `;

  // Creating the library to call the counter contract
  const gameComponentLib = AssemblerUtils.createAccountComponentLibrary(
    assembler, // assembler
    "external_contract::game_contract", // library path to call the contract
    gameContractCode, // account code of the contract
  );

  // Creating the transaction script
  const deploymentScript = TransactionScript.compile(
    deploymentScriptCode,
    assembler.withLibrary(gameComponentLib),
  );

  const deploymentArg = Word.newFromFelts([
    bob.id().suffix(),
    bob.id().prefix(),
    alice.id().suffix(),
    alice.id().prefix(),
  ]);

  // Creating a transaction request with the transaction script
  const deploymentRequest = new TransactionRequestBuilder()
    .withCustomScript(deploymentScript)
    .withScriptArg(deploymentArg)
    .build();

  // Executing the transaction script against the counter contract
  const txResult = await client.newTransaction(
    gameContract.account.id(),
    deploymentRequest,
  );

  // Submitting the transaction result to the node
  await client.submitTransaction(txResult);

  // Sync state
  await client.syncState();
}
