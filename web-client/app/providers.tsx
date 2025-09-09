"use client";

import { WalletProvider } from "@demox-labs/miden-wallet-adapter-react";
import {
  MidenWalletAdapter,
  WalletModalProvider,
} from "@demox-labs/miden-wallet-adapter";

export function Providers({ children }: { children: React.ReactNode }) {
  const wallet = new MidenWalletAdapter({ appName: "Tic Tac Toe" });

  return (
    <WalletProvider wallets={[wallet]}>
      <WalletModalProvider>{children}</WalletModalProvider>
    </WalletProvider>
  );
}
