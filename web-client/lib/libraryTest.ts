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
    NoteType,
  } = await import("@demox-labs/miden-sdk");

  const nodeEndpoint = "https://rpc.testnet.miden.io:443";
  // const nodeEndpoint = "http://localhost:57291";
  const client = await WebClient.createClient(nodeEndpoint);

  console.log((await client.syncState()).blockNum());

  const accountCode = `
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
    
    push.1 exec.account::incr_nonce
    # => []
    
    exec.sys::truncate_stack
    # => []
end

      `;

  let txScript = `
      use.external_contract::counter_contract
      begin
          call.counter_contract::increment_count
      end`;

  let assembler = TransactionKernel.assembler().withDebugMode(true);
  let emptyStorageSlot = StorageSlot.emptyValue();
  let storageMap = new StorageMap();
  let storageSlotMap = StorageSlot.map(storageMap);

  let mappingAccountComponent = AccountComponent.compile(
    accountCode,
    assembler,
    [emptyStorageSlot, storageSlotMap],
  ).withSupportsAllTypes();

  console.log(mappingAccountComponent);
}
