// lib/incrementCounterContract.ts
export async function incrementCounterContract(): Promise<void> {
  if (typeof window === 'undefined') {
    console.warn('webClient() can only run in the browser');
    return;
  }

  // dynamic import → only in the browser, so WASM is loaded client‑side
  const {
    Address,
    AccountBuilder,
    AccountComponent,
    AccountStorageMode,
    AccountType,
    AuthSecretKey,
    StorageSlot,
    TransactionRequestBuilder,
    WebClient,
  } = await import('@miden-sdk/miden-sdk');

  const nodeEndpoint = 'https://rpc.devnet.miden.io';
  const client = await WebClient.createClient(nodeEndpoint);
  console.log('Current block number: ', (await client.syncState()).blockNum());

  // Counter contract code in Miden Assembly
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

  // Building the counter contract
  // Counter contract account id on testnet
  const counterContractId = Address.fromBech32(
    'mtst1arjemrxne8lj5qz4mg9c8mtyxg954483',
  ).accountId();

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

  const builder = client.createCodeBuilder();
  const counterSlotName = 'miden::tutorials::counter';
  const counterStorageSlot = StorageSlot.emptyValue(counterSlotName);

  const counterComponentCode =
    builder.compileAccountComponentCode(counterContractCode);
  const counterAccountComponent = AccountComponent.compile(
    counterComponentCode,
    [counterStorageSlot],
  ).withSupportsAllTypes();

  const walletSeed = new Uint8Array(32);
  crypto.getRandomValues(walletSeed);

  const secretKey = AuthSecretKey.rpoFalconWithRNG(walletSeed);
  const authComponent =
    AccountComponent.createAuthComponentFromSecretKey(secretKey);

  const accountBuilderResult = new AccountBuilder(walletSeed)
    .accountType(AccountType.RegularAccountImmutableCode)
    .storageMode(AccountStorageMode.public())
    .withAuthComponent(authComponent)
    .withComponent(counterAccountComponent)
    .build();

  await client.addAccountSecretKeyToWebStore(
    accountBuilderResult.account.id(),
    secretKey,
  );
  await client.newAccount(accountBuilderResult.account, false);

  await client.syncState();

  const accountCodeLib = builder.buildLibrary(
    'external_contract::counter_contract',
    counterContractCode,
  );

  builder.linkDynamicLibrary(accountCodeLib);

  // Building the transaction script which will call the counter contract
  const txScriptCode = `
    use external_contract::counter_contract
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
  const count = counter?.storage().getItem(counterSlotName);

  // Converting the Word represented as a hex to a single integer value
  const counterValue = Number(
    BigInt('0x' + count!.toHex().slice(-16).match(/../g)!.reverse().join('')),
  );

  console.log('Count: ', counterValue);
}
