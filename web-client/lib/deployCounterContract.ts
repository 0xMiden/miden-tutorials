// lib/webClient.ts
export async function deployCounterContract(): Promise<void> {
  if (typeof window === "undefined") {
    console.warn("webClient() can only run in the browser");
    return;
  }

  // dynamic import → only in the browser, so WASM is loaded client‑side
  const {
    AccountBuilder,
    AccountComponent,
    AccountStorageMode,
    AccountType,
    AssemblerUtils,
    StorageSlot,
    TransactionKernel,
    TransactionRequestBuilder,
    TransactionScript,
    TransactionScriptInputPairArray,
    WebClient,
  } = await import("@demox-labs/miden-sdk");

  const nodeEndpoint = "https://rpc.testnet.miden.io:443";
  const client = await WebClient.createClient(nodeEndpoint);
  console.log("Current block number: ", (await client.syncState()).blockNum());

  // Counter contract code in Miden Assembly
  const accountCode = `
      use.miden::account
      use.std::sys

      # => []
      export.get_count
          push.0
          # => [index]
          
          # exec.account::get_item
          # => [count]
          
          # exec.sys::truncate_stack
          # => []
      end

      # => []
      export.increment_count
          push.0
          # => [index]
          
          exec.account::get_item
          # => [count]
          
          push.1 add
          # => [count+1]

          # debug statement with client
          debug.stack

          push.0
          # [index, count+1]
          
          exec.account::set_item
          # => []
          
          push.1 exec.account::incr_nonce
          # => []
          
          exec.sys::truncate_stack
          # => []
      end
    `;

  // Building the counter contract
  let assembler = TransactionKernel.assembler();
  let emptyStorageSlot = StorageSlot.emptyValue();

  let counterAccountComponent = AccountComponent.compile(
    accountCode, // contract code
    assembler, // assembler
    [emptyStorageSlot], // storage data
  ).withSupportsAllTypes();

  const walletSeed = new Uint8Array(32);
  crypto.getRandomValues(walletSeed);

  let anchorBlock = await client.getLatestEpochBlock();

  let accountBuilderResult = new AccountBuilder(walletSeed)
    .anchor(anchorBlock)
    .accountType(AccountType.RegularAccountImmutableCode)
    .storageMode(AccountStorageMode.public())
    .withComponent(counterAccountComponent)
    .build();

  // Importing the counter contract into the WebClient
  await client.newAccount(
    accountBuilderResult.account, // account
    accountBuilderResult.seed, // seed
    false, // overwrite
  );
  let counterContract = accountBuilderResult.account;
  console.log("Counter contract id: ", counterContract.id().toString());

  // Building the transaction script which will call the counter contract
  let txScriptCode = `
    use.external_contract::counter_contract
    begin
        call.counter_contract::increment_count
    end
  `;

  // Empty inputs to the transaction script
  const inputs = new TransactionScriptInputPairArray();

  // Creating the library to call the counter contract
  let counterComponentLib = AssemblerUtils.createAccountComponentLibrary(
    assembler, // assembler
    "external_contract::counter_contract", // library path to call the contract
    accountCode, // account code of the contract
  );

  // Creating the transaction script
  let txScript = TransactionScript.compile(
    txScriptCode,
    inputs,
    assembler.withLibrary(counterComponentLib),
  );

  // Creating a transaction request with the transaction script
  let txIncrementRequest = new TransactionRequestBuilder()
    .withCustomScript(txScript)
    .build();

  // Executing the transaction script against the counter contract
  let txResult = await client.newTransaction(
    accountBuilderResult.account.id(),
    txIncrementRequest,
  );

  // Submitting the transaction result to the node
  await client.submitTransaction(txResult);

  // Sync state
  await client.syncState();

  // Logging the count of counter contract
  let counter = await client.getAccount(counterContract.id());
  let count = counter?.storage().getItem(0);

  const counterValue = Number(
    BigInt("0x" + count!.toHex().slice(-16).match(/../g)!.reverse().join("")),
  );

  console.log("Count: ", counterValue);
}
