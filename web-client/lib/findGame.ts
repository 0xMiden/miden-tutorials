import { AccountId } from "@demox-labs/miden-sdk";
import { WebClient } from "@demox-labs/miden-sdk";

// lib/findGame.ts
export async function findGame(
  gameAccountId: AccountId,
  connectedWalletId: AccountId,
  client: WebClient,
): Promise<boolean> {
  if (typeof window === "undefined") {
    console.warn("findGame() can only run in the browser");
    return false;
  }

  try {
    await client.syncState();

    console.log("Connected wallet ID:", connectedWalletId);

    // Get the game account to access its storage
    let gameAccount = await client.getAccount(gameAccountId);
    if (!gameAccount) {
      await client.importAccountById(gameAccountId);
      await client.syncState();
      gameAccount = await client.getAccount(gameAccountId);
      if (!gameAccount) {
        throw new Error(`Account not found after import: ${gameAccountId}`);
      }
    }

    // Check storage slots 1 and 2 for player1 and player2 account IDs
    const storage = gameAccount.storage();

    // Storage slot 1: player1 account ID
    const player1Slot = storage.getItem(0);
    // Storage slot 2: player2 account ID
    const player2Slot = storage.getItem(1);

    console.log("before connected id");

    // const connectedId = AccountId.fromBech32(
    //   "mtst1qrhk2vv5tk5xgyprjkwpep87t4cqqwa3vqx",
    // );
    const connectedIdPrefix = connectedWalletId.prefix();
    const connectedIdSuffix = connectedWalletId.suffix();

    // 0, 0, prefix, suffix (reversed)
    if (player1Slot !== undefined) {
      const felts = player1Slot.toFelts();

      const prefixFelt = felts[1];
      const suffixFelt = felts[0];

      console.log(prefixFelt.asInt(), suffixFelt.asInt());
      console.log(connectedIdPrefix.asInt(), connectedIdSuffix.asInt());

      if (
        prefixFelt.asInt() === connectedIdPrefix.asInt() &&
        suffixFelt.asInt() === connectedIdSuffix.asInt()
      ) {
        return true;
      }
    }

    if (player2Slot !== undefined) {
      const felts = player2Slot.toFelts();

      const prefixFelt = felts[1];
      const suffixFelt = felts[0];

      console.log(prefixFelt.asInt(), suffixFelt.asInt());
      console.log(connectedIdPrefix.asInt(), connectedIdSuffix.asInt());

      if (
        prefixFelt.asInt() === connectedIdPrefix.asInt() &&
        suffixFelt.asInt() === connectedIdSuffix.asInt()
      ) {
        return true;
      }
    }

    return false;
  } catch (error) {
    console.error("Failed to find game:", error);
    return false;
  }
}
