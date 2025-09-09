import gameContractCode from "./contracts/tic_tac_toe_code";
import {
  AccountId,
  AssemblerUtils,
  AccountStorageMode,
  AccountComponent,
  AccountType,
  AccountBuilder,
  StorageSlot,
  StorageMap,
  TransactionKernel,
  TransactionRequestBuilder,
  TransactionScript,
  WebClient,
  Word,
} from "@demox-labs/miden-sdk";

const incrNonceAuthCode = `use.miden::account
        export.auth__basic
          exec.account::incr_nonce
          drop
        end`;

// lib/createGame.ts
export async function createGame(
  alice: AccountId,
  bob: AccountId,
  client: WebClient,
): Promise<string> {
  if (typeof window === "undefined") {
    console.warn("webClient() can only run in the browser");
    return "";
  }

  await client.syncState();

  console.log("Generated accounts");

  // Building the tic tac toe contract
  let assembler = TransactionKernel.assembler().withDebugMode(true);

  let emptyStorageSlot = StorageSlot.emptyValue();
  let storageMap = new StorageMap();
  let storageSlotMap = StorageSlot.map(storageMap);

  console.log("before game component");

  let gameComponent = AccountComponent.compile(gameContractCode, assembler, [
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

  console.log("after game component");

  let seed = new Uint8Array(32);
  crypto.getRandomValues(seed);

  const noAuth = AccountComponent.compile(
    incrNonceAuthCode,
    assembler,
    [],
  ).withSupportsAllTypes();

  const gameContract = new AccountBuilder(seed)
    .accountType(AccountType.RegularAccountImmutableCode)
    .storageMode(AccountStorageMode.public())
    .withComponent(gameComponent)
    .withAuthComponent(noAuth)
    .build();

  console.log("Created game contract locally");

  await client.newAccount(gameContract.account, gameContract.seed, false);

  console.log("Added game contract to client");

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
    bob.suffix(),
    bob.prefix(),
    alice.suffix(),
    alice.prefix(),
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

  // Return the game contract account ID
  return gameContract.account.id().toString();
}
