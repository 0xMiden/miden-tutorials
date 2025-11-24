// lib/incrementCounterContract.ts
export async function incrementCounterContract(): Promise<void> {
  if (typeof window === 'undefined') {
    console.warn('webClient() can only run in the browser');
    return;
  }

  // dynamic import → only in the browser, so WASM is loaded client‑side
  const {
    AccountId,
    AccountBuilder,
    AccountComponent,
    AccountStorageMode,
    AccountType,
    SecretKey,
    StorageMap,
    StorageSlot,
    TransactionRequestBuilder,
    WebClient,
  } = await import('@demox-labs/miden-sdk');

  const nodeEndpoint = 'https://rpc.testnet.miden.io';
  const client = await WebClient.createClient(nodeEndpoint);
  console.log('Current block number: ', (await client.syncState()).blockNum());

  // Counter contract code in Miden Assembly
  const counterContractCode = `
    use.miden::active_account
    use miden::native_account
    use.std::sys

    const.COUNTER_SLOT=0

    #! Inputs:  []
    #! Outputs: [count]
    export.get_count
        push.COUNTER_SLOT
        # => [index]

        exec.active_account::get_item
        # => [count]

        # clean up stack
        movdn.4 dropw
        # => [count]
    end

    #! Inputs:  []
    #! Outputs: []
    export.increment_count
        push.COUNTER_SLOT
        # => [index]

        exec.active_account::get_item
        # => [count]

        add.1
        # => [count+1]

        debug.stack

        push.COUNTER_SLOT
        # [index, count+1]

        exec.native_account::set_item
        # => [OLD_VALUE]

        dropw
        # => []
    end
`;

  // Building the counter contract
  // Counter contract account id on testnet
  const counterContractId = AccountId.fromHex(
    '0xe59d8cd3c9ff2a0055da0b83ed6432',
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

  const builder = client.createScriptBuilder();
  const storageMap = new StorageMap();
  const storageSlotMap = StorageSlot.map(storageMap);

  const mappingAccountComponent = AccountComponent.compile(
    counterContractCode,
    builder,
    [storageSlotMap],
  ).withSupportsAllTypes();

  const walletSeed = new Uint8Array(32);
  crypto.getRandomValues(walletSeed);

  const secretKey = SecretKey.rpoFalconWithRNG(walletSeed);
  const authComponent = AccountComponent.createAuthComponent(secretKey);

  const accountBuilderResult = new AccountBuilder(walletSeed)
    .accountType(AccountType.RegularAccountImmutableCode)
    .storageMode(AccountStorageMode.public())
    .withAuthComponent(authComponent)
    .withComponent(mappingAccountComponent)
    .build();

  await client.addAccountSecretKeyToWebStore(secretKey);
  await client.newAccount(accountBuilderResult.account, false);

  await client.syncState();

  const accountCodeLib = builder.buildLibrary(
    'external_contract::counter_contract',
    counterContractCode,
  );

  builder.linkDynamicLibrary(accountCodeLib);

  // Building the transaction script which will call the counter contract
  const txScriptCode = `
    use.external_contract::counter_contract
    begin
    call.counter_contract::increment_count
    end
`;

  const txScript = builder.compileTxScript(txScriptCode);
  const txIncrementRequest = new TransactionRequestBuilder()
    .withCustomScript(txScript)
    .build();

  // Executing the transaction script against the counter contract
  await client.submitNewTransaction(
    counterContractAccount.id(),
    txIncrementRequest,
  );

  // Sync state
  await client.syncState();

  // Logging the count of counter contract
  const counter = await client.getAccount(counterContractAccount.id());

  // Here we get the first Word from storage of the counter contract
  // A word is comprised of 4 Felts, 2**64 - 2**32 + 1
  const count = counter?.storage().getItem(0);

  // Converting the Word represented as a hex to a single integer value
  const counterValue = Number(
    BigInt('0x' + count!.toHex().slice(-16).match(/../g)!.reverse().join('')),
  );

  console.log('Count: ', counterValue);
}
