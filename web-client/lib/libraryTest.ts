// lib/webClient.ts
export async function libraryTest(): Promise<void> {
  if (typeof window === "undefined") {
    console.warn("webClient() can only run in the browser");
    return;
  }

  // dynamic import → only in the browser, so WASM is loaded client‑side
  const {
    WebClient,
    AccountStorageMode,
    AccountId,
    TransactionKernel,
    StorageSlot,
    AccountComponent,
    StorageMap,
    AssemblerUtils,
    NoteType,
    TransactionScript,
    TransactionScriptInputPairArray,
    TransactionRequestBuilder
  } = await import("@demox-labs/miden-sdk");

  const nodeEndpoint = "https://rpc.testnet.miden.io:443";
  // const nodeEndpoint = "http://localhost:57291";
  const client = await WebClient.createClient(nodeEndpoint);
  console.log((await client.syncState()).blockNum());

  const counterContractId = AccountId.fromHex(
    "0x5fd8e3b9f4227200000581c6032f81",
  );
  let account = await client.getAccount(counterContractId);

  if (!account) {
    await client.importAccountById(counterContractId);
    await client.syncState();
    account = await client.getAccount(counterContractId);
    if (!account) {
      throw new Error(`Account not found after import: ${counterContractId}`);
    }
  }

  const accountCode = `
      # use.miden::account
      # use.std::sys

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
          
          # exec.account::get_item
          # => [count]
          
          push.1 add
          # => [count+1]

          # debug statement with client
          debug.stack

          push.0
          # [index, count+1]
          
          # exec.account::set_item
          # => []
          
          # push.1 exec.account::incr_nonce
          # => []
          
          # exec.sys::truncate_stack
          # => []
      end
      `;

  let txScriptCode = `
      use.external_contract::counter_contract
      begin
          call.counter_contract::increment_count
      end
      `;

  let assembler = TransactionKernel.assembler().withDebugMode(true);

  let counterComponentLib = AssemblerUtils.createAccountComponentLibrary(
    assembler,
    "external_contract::counter_contract",
    accountCode,
  );

    const inputs = new TransactionScriptInputPairArray();


  let txScript = TransactionScript.compile(
    txScriptCode,
    inputs,
    assembler.withLibrary(counterComponentLib),
  );

  let txIncrementRequest = new TransactionRequestBuilder()
    .withCustomScript(txScript)
    .build();

  let txResult = await client.newTransaction(
    counterContractId,
    txIncrementRequest,
  );
  await client.submitTransaction(txResult);

  await client.syncState();

  let counter = await client.getAccount(counterContractId);

  let count = counter?.storage().getItem(0);

  console.log("count: ", count);

  // let transactionScript = new TransactionRequestBuilder().
}
