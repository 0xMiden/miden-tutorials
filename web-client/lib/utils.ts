import { NODE_URL } from "./constants";

export const instantiateClient = async ({
  accountsToImport,
}: {
  accountsToImport: string[];
}) => {
  const { WebClient, AccountId } = await import("@demox-labs/miden-sdk");
  const nodeEndpoint = NODE_URL;
  const client = await WebClient.createClient(nodeEndpoint);
  for (const accString of accountsToImport) {
    try {
      const accountId = AccountId.fromBech32(accString);
      await safeAccountImport(client, accountId);
    } catch {}
  }
  await client.syncState();
  return client;
};

export const safeAccountImport = async (
  client: any, // WebClient
  accountId: any, // AccountId
) => {
  if ((await client.getAccount(accountId)) == null) {
    try {
      client.importAccountById(accountId);
    } catch (e) {
      console.warn(e);
    }
  }
};
