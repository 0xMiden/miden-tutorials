"use client";
import { useState } from "react";
import { webClient } from "../lib/webClient";

export default function Home() {
  const [started, setStarted] = useState(false);

  const handleClick = async () => {
    setStarted(true);
    await webClient();
    setStarted(false);
  };

  return (
    <main className="min-h-screen flex items-center justify-center bg-gradient-to-br from-gray-900 via-gray-800 to-black text-slate-800 dark:text-slate-100">
      <div className="text-center">
        <h1 className="text-4xl font-semibold mb-4">Miden Web App</h1>
        <p className="mb-4">Open your browser console to see WebClient logs.</p>
        <button
          onClick={handleClick}
          className="px-6 py-3 text-lg cursor-pointer bg-transparent border-2 border-orange-600 text-white rounded-lg transition-all hover:bg-orange-600 hover:text-white"
        >
          {started ? "Working..." : "Start WebClient"}
        </button>
      </div>
    </main>
  );
}
