"use client";
import { useState } from "react";
import { webClient } from "../lib/webClient";
import { multiSendWithDelegatedProver } from "../lib/multiSendWithDelegatedProver";

export default function Home() {
  const [isStartingClient, setIsStartingClient] = useState(false);
  const [isSendingNotes, setIsSendingNotes] = useState(false);

  const handleStartClient = async () => {
    setIsStartingClient(true);
    await webClient();
    setIsStartingClient(false);
  };

  const handleSendNotes = async () => {
    setIsSendingNotes(true);
    await multiSendWithDelegatedProver();
    setIsSendingNotes(false);
  };

  return (
    <main className="min-h-screen flex items-center justify-center bg-gradient-to-br from-gray-900 via-gray-800 to-black text-slate-800 dark:text-slate-100">
      <div className="text-center">
        <h1 className="text-4xl font-semibold mb-4">Miden Web App</h1>
        <p className="mb-6">Open your browser console to see WebClient logs.</p>

        <div className="max-w-sm w-full bg-gray-800/20 border border-gray-600 rounded-2xl p-6 mx-auto flex flex-col gap-4">
          <button
            onClick={handleStartClient}
            className="w-full px-6 py-3 text-lg cursor-pointer bg-transparent border-2 border-orange-600 text-white rounded-lg transition-all hover:bg-orange-600 hover:text-white"
          >
            {isStartingClient ? "Working..." : "Start WebClient"}
          </button>

          <button
            onClick={handleSendNotes}
            className="w-full px-6 py-3 text-lg cursor-pointer bg-transparent border-2 border-orange-600 text-white rounded-lg transition-all hover:bg-orange-600 hover:text-white"
          >
            {isSendingNotes ? "Working..." : "Send 1 to N P2ID Notes"}
          </button>
        </div>
      </div>
    </main>
  );
}
