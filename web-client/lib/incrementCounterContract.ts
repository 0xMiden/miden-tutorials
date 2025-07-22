// lib/incrementCounterContract.ts
export async function incrementCounterContract(): Promise<void> {
  if (typeof window === "undefined") {
    console.warn("webClient() can only run in the browser");
    return;
  }

  // dynamic import → only in the browser, so WASM is loaded client‑side
  const {
    AccountId,
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
  const counterContractCode = `
    use.miden::account
    use.std::sys

    # => []
    export.get_count
        push.0
        # => [index]
        
        exec.account::get_item
        # => [count]
        
        exec.sys::truncate_stack
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
        
        exec.sys::truncate_stack
        # => []
    end
    `;

  // Building the counter contract
  let assembler = TransactionKernel.assembler();

  // Counter contract account id on testnet
  const counterContractId = AccountId.fromBech32(
    "mtst1qz43ftxkrzcjsqz3hpw332qwny2ggsp0",
  );

  // Reading the public state of the counter contract from testnet,
  // and importing it into the WebClient
  let counterContractAccount = await client.getAccount(counterContractId);
  if (!counterContractAccount) {
    await client.importAccountById(counterContractId);
    await client.syncState();
    counterContractAccount = await client.getAccount(counterContractId);
    if (!counterContractAccount) {
      throw new Error(`Account not found after import: ${counterContractId}`);
    }
  }

  // Building the transaction script which will call the counter contract
  let txScriptCode = `
    use.external_contract::counter_contract
    begin
        call.counter_contract::increment_count
    end
  `;

  // Creating the library to call the counter contract
  let counterComponentLib = AssemblerUtils.createAccountComponentLibrary(
    assembler, // assembler
    "external_contract::counter_contract", // library path to call the contract
    counterContractCode, // account code of the contract
  );

  // Creating the transaction script
  let txScript = TransactionScript.compile(
    txScriptCode,
    assembler.withLibrary(counterComponentLib),
  );

  // Creating a transaction request with the transaction script
  let txIncrementRequest = new TransactionRequestBuilder()
    .withCustomScript(txScript)
    .build();

  // Executing the transaction script against the counter contract
  let txResult = await client.newTransaction(
    counterContractAccount.id(),
    txIncrementRequest,
  );

  // Submitting the transaction result to the node
  await client.submitTransaction(txResult);

  // Sync state
  await client.syncState();

  // Logging the count of counter contract
  let counter = await client.getAccount(counterContractAccount.id());

  // Here we get the first Word from storage of the counter contract
  // A word is comprised of 4 Felts, 2**64 - 2**32 + 1
  let count = counter?.storage().getItem(1);

  // Converting the Word represented as a hex to a single integer value
  const counterValue = Number(
    BigInt("0x" + count!.toHex().slice(-16).match(/../g)!.reverse().join("")),
  );

  console.log("Count: ", counterValue);
}
