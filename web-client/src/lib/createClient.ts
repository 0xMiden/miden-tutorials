// src/lib/createClient.ts
import { WebClient } from '@demox-labs/miden-sdk';

const localNodeUrl = 'http://localhost:57291';

class ClientSingleton {
  private static instance: WebClient | null = null;
  private constructor() {}

  public static async getInstance(): Promise<WebClient> {
    if (this.instance === null) {
      this.instance = new WebClient();
      await this.instance.create_client(localNodeUrl);
      console.log('WebClient initialized.');
    }
    return this.instance;
  }
}

export default ClientSingleton;
