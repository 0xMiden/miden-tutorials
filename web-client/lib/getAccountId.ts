export async function getAccountId(
  accountIdString: string,
): Promise<any | null> {
  if (typeof window === "undefined") {
    console.warn("webClient() can only run in the browser");
    return null;
  }

  const { AccountId } = await import("@demox-labs/miden-sdk");
  return AccountId.fromBech32(accountIdString);
}
