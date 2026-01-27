// lib/createMintConsume.ts
export async function createMintConsume(): Promise<void> {
  if (typeof window === 'undefined') {
    console.warn('webClient() can only run in the browser');
    return;
  }

  // dynamic import → only in the browser, so WASM is loaded client‑side
  const {
    WebClient,
    AccountStorageMode,
    AuthScheme,
    NoteType,
    Address,
  } = await import('@miden-sdk/miden-sdk');

  const nodeEndpoint = 'https://rpc.devnet.miden.io';
  const client = await WebClient.createClient(nodeEndpoint);

  // 1. Sync with the latest blockchain state
  const state = await client.syncState();
  console.log('Latest block number:', state.blockNum());

  // 2. Create Alice's account
  console.log('Creating account for Alice…');
  const aliceSeed = new Uint8Array(32);
  crypto.getRandomValues(aliceSeed);
  const alice = await client.newWallet(
    AccountStorageMode.public(),
    true,
    AuthScheme.AuthRpoFalcon512,
    aliceSeed,
  );
  console.log('Alice ID:', alice.id().toString());

  // 3. Deploy a fungible faucet
  console.log('Creating faucet…');
  const faucet = await client.newFaucet(
    AccountStorageMode.public(),
    false,
    'MID',
    8,
    BigInt(1_000_000),
    AuthScheme.AuthRpoFalcon512,
  );
  console.log('Faucet ID:', faucet.id().toString());

  await client.syncState();

  // 4. Mint tokens to Alice
  await client.syncState();

  console.log('Minting tokens to Alice...');
  const mintTxRequest = client.newMintTransactionRequest(
    alice.id(),
    faucet.id(),
    NoteType.Public,
    BigInt(1000),
  );

  await client.submitNewTransaction(faucet.id(), mintTxRequest);

  console.log('Waiting 10 seconds for transaction confirmation...');
  await new Promise((resolve) => setTimeout(resolve, 10000));
  await client.syncState();

  // 5. Fetch minted notes
  const mintedNotes = await client.getConsumableNotes(alice.id());
  const mintedNoteList = mintedNotes.map((n) =>
    n.inputNoteRecord().toNote(),
  );
  console.log(
    'Minted notes:',
    mintedNoteList.map((note) => note.id().toString()),
  );

  // 6. Consume minted notes
  console.log('Consuming minted notes...');
  const consumeTxRequest = client.newConsumeTransactionRequest(mintedNoteList);

  await client.submitNewTransaction(alice.id(), consumeTxRequest);

  await client.syncState();
  console.log('Notes consumed.');

  // 7. Send tokens to Bob
  const bobAccountId = Address.fromBech32(
    'mtst1apve54rq8ux0jqqqqrkh5y0r0y8cwza6_qruqqypuyph',
  ).accountId();
  console.log("Sending tokens to Bob's account...");
  const sendTxRequest = client.newSendTransactionRequest(
    alice.id(),
    bobAccountId,
    faucet.id(),
    NoteType.Public,
    BigInt(100),
  );

  await client.submitNewTransaction(alice.id(), sendTxRequest);
  console.log('Tokens sent successfully!');
}
