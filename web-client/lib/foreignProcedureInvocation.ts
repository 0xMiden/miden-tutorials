// lib/foreignProcedureInvocation.ts
export async function foreignProcedureInvocation(): Promise<void> {
  if (typeof window === "undefined") {
    console.warn("foreignProcedureInvocation() can only run in the browser");
    return;
  }

  // dynamic import → only in the browser, so WASM is loaded client‑side
  const {
    AccountBuilder,
    AccountComponent,
    AccountId,
    AccountType,
    AssemblerUtils,
    StorageSlot,
    TransactionKernel,
    TransactionRequestBuilder,
    TransactionScript,
    TransactionScriptInputPairArray,
    Felt,
    StorageMap,
    ForeignAccount,
    AccountStorageRequirements,
    WebClient,
    AccountStorageMode,
  } = await import("@demox-labs/miden-sdk");

  const nodeEndpoint = "https://rpc.testnet.miden.io:443";
  const client = await WebClient.createClient(nodeEndpoint);
  console.log("Current block number: ", (await client.syncState()).blockNum());

  // -------------------------------------------------------------------------
  // STEP 1: Create the Count Reader Contract
  // -------------------------------------------------------------------------
  console.log("\n[STEP 1] Creating count reader contract.");

  // Count reader contract code in Miden Assembly (exactly from count_reader.masm)
  const countReaderCode = `
    use.miden::account
    use.miden::tx
    use.std::sys

    # => [account_id_prefix, account_id_suffix, get_count_proc_hash]
    export.copy_count
        exec.tx::execute_foreign_procedure
        # => [count]
        
        debug.stack
        # => [count]
        
        push.0
        # [index, count]
        
        exec.account::set_item
        # => []
        
        push.1 exec.account::incr_nonce
        # => []

        exec.sys::truncate_stack
        # => []
    end
  `;

  // Prepare assembler (debug mode = true)
  let assembler = TransactionKernel.assembler().withDebugMode(true);

  let countReaderComponent = AccountComponent.compile(
    countReaderCode,
    assembler,
    [StorageSlot.emptyValue()],
  ).withSupportsAllTypes();

  const seed = new Uint8Array(32);
  crypto.getRandomValues(seed);

  let anchor = await client.getLatestEpochBlock();

  let countReaderContract = new AccountBuilder(seed)
    .anchor(anchor)
    .accountType(AccountType.RegularAccountImmutableCode)
    .storageMode(AccountStorageMode.public())
    .withComponent(countReaderComponent)
    .build();

  // Create the count reader contract account (using available WebClient API)
  console.log("Creating count reader contract account...");
  console.log(
    "Count reader contract ID:",
    countReaderContract.account.id().toString(),
  );

  await client.newAccount(
    countReaderContract.account,
    countReaderContract.seed,
    false,
  );

  // -------------------------------------------------------------------------
  // STEP 2: Build & Get State of the Counter Contract
  // -------------------------------------------------------------------------
  console.log("\n[STEP 2] Building counter contract from public state");

  // Define the Counter Contract account id from counter contract deploy (same as Rust)
  const counterContractId = AccountId.fromHex(
    "0xb32d619dfe9e2f0000010ecb441d3f",
  );

  // Import the counter contract
  let counterContractAccount = await client.getAccount(counterContractId);
  if (!counterContractAccount) {
    await client.importAccountById(counterContractId);
    await client.syncState();
    counterContractAccount = await client.getAccount(counterContractId);
    if (!counterContractAccount) {
      throw new Error(`Account not found after import: ${counterContractId}`);
    }
  }
  console.log(
    "Account storage slot 0:",
    counterContractAccount.storage().getItem(0)?.toHex(),
  );

  // -------------------------------------------------------------------------
  // STEP 3: Call the Counter Contract via Foreign Procedure Invocation (FPI)
  // -------------------------------------------------------------------------
  console.log(
    "\n[STEP 3] Call counter contract with FPI from count reader contract",
  );

  // Counter contract code (exactly from counter.masm)
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
      
      push.1 exec.account::incr_nonce
      # => []
      
      exec.sys::truncate_stack
      # => []
  end
  `;

  // Create the counter contract component to get the procedure hash (following Rust pattern)
  let counterContractComponent = AccountComponent.compile(
    counterContractCode,
    assembler,
    [],
  ).withSupportsAllTypes();

  let getCountProcHash = counterContractComponent.getProcedureHash("get_count");

  // Build the script that calls the count reader contract (exactly from reader_script.masm with replacements)
  let fpiScriptCode = `
    use.external_contract::count_reader_contract
    use.std::sys

    begin
        push.${getCountProcHash}
        # => [GET_COUNT_HASH]

        push.${counterContractAccount.id().suffix()}
        # => [account_id_suffix, GET_COUNT_HASH]

        push.${counterContractAccount.id().prefix()}
        # => [account_id_prefix, account_id_suffix, GET_COUNT_HASH]

        call.count_reader_contract::copy_count
        # => []

        exec.sys::truncate_stack
        # => []

    end
  `;

  console.log("fpiScript", fpiScriptCode);

  // Empty inputs to the transaction script
  const inputs = new TransactionScriptInputPairArray();

  // Create the library for the count reader contract
  let countReaderLib = AssemblerUtils.createAccountComponentLibrary(
    assembler,
    "external_contract::count_reader_contract",
    countReaderCode,
  );

  // Compile the transaction script with the count reader library
  let txScript = TransactionScript.compile(
    fpiScriptCode,
    inputs,
    assembler.withLibrary(countReaderLib),
  );

  // foreign account
  let storageRequirements = new AccountStorageRequirements();

  let foreignAccount = ForeignAccount.public(
    counterContractId,
    storageRequirements,
  );

  // Build a transaction request with the custom script
  let txRequest = new TransactionRequestBuilder()
    .withCustomScript(txScript)
    .withForeignAccounts([foreignAccount])
    .build();

  console.log("HERE");

  // Execute the transaction locally on the count reader contract (following Rust pattern)
  let txResult = await client.newTransaction(
    countReaderContract.account.id(),
    txRequest,
  );

  console.log("HERE1");
  console.log(
    "View transaction on MidenScan: https://testnet.midenscan.com/tx/" +
      txResult.executedTransaction().id().toHex(),
  );

  // Submit transaction to the network
  await client.submitTransaction(txResult);
  await client.syncState();

  // Retrieve updated contract data to see the results (following Rust pattern)
  let updatedCounterContract = await client.getAccount(
    counterContractAccount.id(),
  );
  console.log(
    "counter contract storage:",
    updatedCounterContract?.storage().getItem(0)?.toHex(),
  );

  let updatedCountReaderContract = await client.getAccount(
    countReaderContract.account.id(),
  );
  console.log(
    "count reader contract storage:",
    updatedCountReaderContract?.storage().getItem(0)?.toHex(),
  );

  // Log the count value copied via FPI
  let countReaderStorage = updatedCountReaderContract?.storage().getItem(0);
  if (countReaderStorage) {
    const countValue = Number(
      BigInt(
        "0x" +
          countReaderStorage
            .toHex()
            .slice(-16)
            .match(/../g)!
            .reverse()
            .join(""),
      ),
    );
    console.log("Count copied via Foreign Procedure Invocation:", countValue);
  }

  console.log("\nForeign Procedure Invocation Transaction completed!");
}
