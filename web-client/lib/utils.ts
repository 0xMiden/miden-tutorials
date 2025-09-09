import type { AccountId, WebClient } from "@demox-labs/miden-sdk";
import { NODE_URL } from "./constants";

export const instantiateClient = async ({
  accountsToImport,
}: {
  accountsToImport: AccountId[];
}) => {
  const { WebClient } = await import("@demox-labs/miden-sdk");
  const nodeEndpoint = NODE_URL;
  const client = await WebClient.createClient(nodeEndpoint);
  for (const acc of accountsToImport) {
    try {
      await safeAccountImport(client, acc);
    } catch {}
  }
  await client.syncState();
  return client;
};

export const safeAccountImport = async (
  client: WebClient,
  accountId: AccountId,
) => {
  if ((await client.getAccount(accountId)) == null) {
    try {
      client.importAccountById(accountId);
    } catch (e) {
      console.warn(e);
    }
  }
};
