// lib/foreignProcedureInvocation.ts
export async function foreignProcedureInvocation(): Promise<void> {
  if (typeof window === 'undefined') {
    console.warn('foreignProcedureInvocation() can only run in the browser');
    return;
  }

  // dynamic import → only in the browser, so WASM is loaded client‑side
  const {
    AccountBuilder,
    AccountComponent,
    Address,
    AccountType,
    AuthSecretKey,
    StorageSlot,
    TransactionRequestBuilder,
    ForeignAccount,
    ForeignAccountArray,
    AccountStorageRequirements,
    WebClient,
    AccountStorageMode,
  } = await import('@miden-sdk/miden-sdk');

  const nodeEndpoint = 'https://rpc.devnet.miden.io';
  const client = await WebClient.createClient(nodeEndpoint);
  console.log('Current block number: ', (await client.syncState()).blockNum());

  // -------------------------------------------------------------------------
  // STEP 1: Create the Count Reader Contract
  // -------------------------------------------------------------------------
  console.log('\n[STEP 1] Creating count reader contract.');

  // Count reader contract code in Miden Assembly (exactly from count_reader.masm)
  const countReaderCode = `
    use miden::protocol::active_account
    use miden::protocol::native_account
    use miden::protocol::tx
    use miden::core::word
    use miden::core::sys

    const COUNT_READER_SLOT = word("miden::tutorials::count_reader")

    # => [account_id_prefix, account_id_suffix, get_count_proc_hash]
    pub proc copy_count
        exec.tx::execute_foreign_procedure
        # => [count]
        
        push.COUNT_READER_SLOT[0..2]
        # [slot_id_prefix, slot_id_suffix, count]

        exec.native_account::set_item
        # => [OLD_VALUE]

        dropw
        # => []

        exec.sys::truncate_stack
        # => []
    end
`;

  const countReaderSlotName = 'miden::tutorials::count_reader';
  const counterSlotName = 'miden::tutorials::counter';

  const builder = client.createCodeBuilder();
  const countReaderComponentCode =
    builder.compileAccountComponentCode(countReaderCode);
  const countReaderComponent = AccountComponent.compile(
    countReaderComponentCode,
    [StorageSlot.emptyValue(countReaderSlotName)],
  ).withSupportsAllTypes();

  const walletSeed = new Uint8Array(32);
  crypto.getRandomValues(walletSeed);

  const secretKey = AuthSecretKey.rpoFalconWithRNG(walletSeed);
  const authComponent = AccountComponent.createAuthComponentFromSecretKey(
    secretKey,
  );

  const countReaderContract = new AccountBuilder(walletSeed)
    .accountType(AccountType.RegularAccountImmutableCode)
    .storageMode(AccountStorageMode.public())
    .withAuthComponent(authComponent)
    .withComponent(countReaderComponent)
    .build();

  await client.addAccountSecretKeyToWebStore(
    countReaderContract.account.id(),
    secretKey,
  );
  await client.syncState();

  // Create the count reader contract account (using available WebClient API)
  console.log('Creating count reader contract account...');
  console.log(
    'Count reader contract ID:',
    countReaderContract.account.id().toString(),
  );

  await client.newAccount(countReaderContract.account, false);

  // -------------------------------------------------------------------------
  // STEP 2: Build & Get State of the Counter Contract
  // -------------------------------------------------------------------------
  console.log('\n[STEP 2] Building counter contract from public state');

  // Define the Counter Contract account id from counter contract deploy (same as Rust)
  const counterContractId = Address.fromBech32(
    'mtst1arjemrxne8lj5qz4mg9c8mtyxg954483',
  ).accountId();

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
    'Account storage slot:',
    counterContractAccount.storage().getItem(counterSlotName)?.toHex(),
  );

  // -------------------------------------------------------------------------
  // STEP 3: Call the Counter Contract via Foreign Procedure Invocation (FPI)
  // -------------------------------------------------------------------------
  console.log(
    '\n[STEP 3] Call counter contract with FPI from count reader contract',
  );

  // Counter contract code (exactly from counter.masm)
  const counterContractCode = `
    use miden::protocol::active_account
    use miden::protocol::native_account
    use miden::core::word
    use miden::core::sys

    const COUNTER_SLOT = word("miden::tutorials::counter")

    #! Inputs:  []
    #! Outputs: [count]
    pub proc get_count
        push.COUNTER_SLOT[0..2] exec.active_account::get_item
        # => [count]

        exec.sys::truncate_stack
        # => [count]
    end

    #! Inputs:  []
    #! Outputs: []
    pub proc increment_count
        push.COUNTER_SLOT[0..2] exec.active_account::get_item
        # => [count]

        add.1
        # => [count+1]

        push.COUNTER_SLOT[0..2] exec.native_account::set_item
        # => []

        exec.sys::truncate_stack
        # => []
    end
`;

  // Create the counter contract component to get the procedure hash (following Rust pattern)
  const counterContractComponentCode =
    builder.compileAccountComponentCode(counterContractCode);
  const counterContractComponent = AccountComponent.compile(
    counterContractComponentCode,
    [StorageSlot.emptyValue(counterSlotName)],
  ).withSupportsAllTypes();

  const getCountProcHash =
    counterContractComponent.getProcedureHash('get_count');

  // Build the script that calls the count reader contract (exactly from reader_script.masm with replacements)
  const fpiScriptCode = `
    use external_contract::count_reader_contract
    use miden::core::sys

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

  // Create the library for the count reader contract
  const countReaderLib = builder.buildLibrary(
    'external_contract::count_reader_contract',
    countReaderCode,
  );
  builder.linkDynamicLibrary(countReaderLib);

  // Compile the transaction script with the count reader library
  const txScript = builder.compileTxScript(fpiScriptCode);

  // foreign account
  const storageRequirements = new AccountStorageRequirements();
  const foreignAccount = ForeignAccount.public(
    counterContractId,
    storageRequirements,
  );

  // Build a transaction request with the custom script
  const txRequest = new TransactionRequestBuilder()
    .withCustomScript(txScript)
    .withForeignAccounts(new ForeignAccountArray([foreignAccount]))
    .build();

  // Execute the transaction on the count reader contract and send it to the network (following Rust pattern)
  const txResult = await client.submitNewTransaction(
    countReaderContract.account.id(),
    txRequest,
  );

  console.log(
    'View transaction on MidenScan: https://testnet.midenscan.com/tx/' +
      txResult.toHex(),
  );

  await client.syncState();

  // Retrieve updated contract data to see the results (following Rust pattern)
  const updatedCounterContract = await client.getAccount(
    counterContractAccount.id(),
  );
  console.log(
    'counter contract storage:',
    updatedCounterContract?.storage().getItem(counterSlotName)?.toHex(),
  );

  const updatedCountReaderContract = await client.getAccount(
    countReaderContract.account.id(),
  );
  console.log(
    'count reader contract storage:',
    updatedCountReaderContract?.storage().getItem(countReaderSlotName)?.toHex(),
  );

  // Log the count value copied via FPI
  const countReaderStorage = updatedCountReaderContract
    ?.storage()
    .getItem(countReaderSlotName);
  if (countReaderStorage) {
    const countValue = Number(
      BigInt(
        '0x' +
          countReaderStorage
            .toHex()
            .slice(-16)
            .match(/../g)!
            .reverse()
            .join(''),
      ),
    );
    console.log('Count copied via Foreign Procedure Invocation:', countValue);
  }

  console.log('\nForeign Procedure Invocation Transaction completed!');
}
