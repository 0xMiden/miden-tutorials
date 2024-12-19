// src/lib/createFaucet.ts

import ClientSingleton from './createClient';
import { AccountStorageMode } from '@demox-labs/miden-sdk';

export async function setupFaucet(): Promise<string> {
  try {
    const webClient = await ClientSingleton.getInstance();

    const faucetId = await webClient.new_faucet(
      AccountStorageMode.private(),
      false,
      'TOK', // Token name
      8, // Decimals
      BigInt(1_000_000) // Initial balance
    );

    console.log(`Faucet created with ID: ${faucetId.id().to_string()}`);
    return faucetId.id().to_string();
  } catch (error) {
    console.error('Error setting up faucet:', error);
    throw error;
  }
}
