// app/page.tsx
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
    <main style={{ padding: 20, textAlign: "center" }}>
      <h1>Miden Web App</h1>
      <p>Open your browser console to see WebClient logs.</p>
      <button
        onClick={handleClick}
        style={{
          padding: "10px 20px",
          fontSize: 16,
          cursor: "pointer",
          background: "transparent",
          border: "1px solid currentColor",
          borderRadius: "9999px",
        }}
      >
        {started ? "Working..." : "Start WebClient"}
      </button>
    </main>
  );
}
