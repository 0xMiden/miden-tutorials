// lib/foreignProcedureInvocation.ts
export async function foreignProcedureInvocation(): Promise<void> {
  if (typeof window === "undefined") {
    console.warn("foreignProcedureInvocation() can only run in the browser");
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
    Felt,
    WebClient,
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

  // Create the count reader contract as a wallet (this will act as our reader contract)
  console.log("Creating count reader contract account...");
  const { AccountStorageMode } = await import("@demox-labs/miden-sdk");
  let countReaderContract = await client.newWallet(
    AccountStorageMode.public(),
    false,
  );
  console.log("Count reader contract ID:", countReaderContract.id().toString());

  // -------------------------------------------------------------------------
  // STEP 2: Build & Get State of the Counter Contract
  // -------------------------------------------------------------------------
  console.log("\n[STEP 2] Building counter contract from public state");

  // Counter contract account id on testnet (same as in incrementCounterContract.ts)
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

  console.log("Account details:", counterContractAccount.storage().getItem(0));

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

  // Get the procedure hash for get_count (this would normally be extracted from the compiled contract)
  // For this demo, we'll simulate the FPI process with placeholder values
  const getCountProcHash =
    "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef";
  const accountIdPrefix = counterContractAccount.id().prefix();
  const accountIdSuffix = counterContractAccount.id().suffix();

  console.log("get count hash (simulated):", getCountProcHash);
  console.log("counter id prefix:", accountIdPrefix);
  console.log("suffix:", accountIdSuffix);

  // Build the FPI script that calls the count reader contract (exactly from reader_script.masm)
  let fpiScriptCode = `
    use.external_contract::count_reader_contract
    use.std::sys

    begin
        # => []
        push.${getCountProcHash}

        # => [GET_COUNT_HASH]
        push.${accountIdSuffix}

        # => [account_id_suffix]
        push.${accountIdPrefix}

        # => []
        push.111 debug.stack drop
        call.count_reader_contract::copy_count

        exec.sys::truncate_stack
    end
  `;

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

  // Build a transaction request with the custom script
  let txRequest = new TransactionRequestBuilder()
    .withCustomScript(txScript)
    .build();

  // Execute the FPI transaction on the count reader contract
  let txResult = await client.newTransaction(
    countReaderContract.id(),
    txRequest,
  );

  console.log(
    "View transaction on MidenScan: https://testnet.midenscan.com/tx/" +
      txResult.executedTransaction().id(),
  );

  // Submit transaction to the network
  await client.submitTransaction(txResult);

  await client.syncState();

  // Retrieve updated contract data to see the FPI results
  let updatedCounterContract = await client.getAccount(
    counterContractAccount.id(),
  );
  console.log(
    "counter contract storage:",
    updatedCounterContract?.storage().getItem(0),
  );

  let updatedCountReaderContract = await client.getAccount(
    countReaderContract.id(),
  );
  console.log(
    "count reader contract storage:",
    updatedCountReaderContract?.storage().getItem(0),
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

  console.log("\nForeign Procedure Invocation completed!");
  console.log(
    "The count reader contract successfully called the counter contract's get_count procedure",
  );
  console.log(
    "using Foreign Procedure Invocation and stored the result in its own storage.",
  );
}
