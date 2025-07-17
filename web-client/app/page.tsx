"use client";
import { useState } from "react";
import { createMintConsume } from "../lib/createMintConsume";
import { multiSendWithDelegatedProver } from "../lib/multiSendWithDelegatedProver";
import { incrementCounterContract } from "../lib/incrementCounterContract";
import { unauthenticatedNoteTransfer } from "../lib/unauthenticatedNoteTransfer";
import { foreignProcedureInvocation } from "../lib/foreignProcedureInvocation";

export default function Home() {
  const [isCreatingNotes, setIsCreatingNotes] = useState(false);
  const [isMultiSendNotes, setIsMultiSendNotes] = useState(false);
  const [isIncrementCounter, setIsIncrementCounter] = useState(false);
  const [isUnauthenticatedNoteTransfer, setIsUnauthenticatedNoteTransfer] = useState(false);
  const [isForeignProcedureInvocation, setIsForeignProcedureInvocation] = useState(false);

  const handleCreateMintConsume = async () => {
    setIsCreatingNotes(true);
    await createMintConsume();
    setIsCreatingNotes(false);
  };

  const handleMultiSendNotes = async () => {
    setIsMultiSendNotes(true);
    await multiSendWithDelegatedProver();
    setIsMultiSendNotes(false);
  };

  const handleIncrementCounterContract = async () => {
    setIsIncrementCounter(true);
    await incrementCounterContract();
    setIsIncrementCounter(false);
  };

  const handleUnauthenticatedNoteTransfer = async () => {
    setIsUnauthenticatedNoteTransfer(true);
    await unauthenticatedNoteTransfer();
    setIsUnauthenticatedNoteTransfer(false);
  };

  const handleForeignProcedureInvocation = async () => {
    setIsForeignProcedureInvocation(true);
    await foreignProcedureInvocation();
    setIsForeignProcedureInvocation(false);
  };

  return (
    <main className="min-h-screen flex items-center justify-center bg-gradient-to-br from-gray-900 via-gray-800 to-black text-slate-800 dark:text-slate-100">
      <div className="text-center">
        <h1 className="text-4xl font-semibold mb-4">Miden Web App</h1>
        <p className="mb-6">Open your browser console to see WebClient logs.</p>

        <div className="max-w-sm w-full bg-gray-800/20 border border-gray-600 rounded-2xl p-6 mx-auto flex flex-col gap-4">
          <button
            onClick={handleCreateMintConsume}
            className="w-full px-6 py-3 text-lg cursor-pointer bg-transparent border-2 border-orange-600 text-white rounded-lg transition-all hover:bg-orange-600 hover:text-white"
          >
            {isCreatingNotes
              ? "Working..."
              : "Tutorial #1: Create, Mint, Consume Notes"}
          </button>

          <button
            onClick={handleMultiSendNotes}
            className="w-full px-6 py-3 text-lg cursor-pointer bg-transparent border-2 border-orange-600 text-white rounded-lg transition-all hover:bg-orange-600 hover:text-white"
          >
            {isMultiSendNotes
              ? "Working..."
              : "Tutorial #2: Send 1 to N P2ID Notes with Delegated Proving"}
          </button>

          <button
            onClick={handleIncrementCounterContract}
            className="w-full px-6 py-3 text-lg cursor-pointer bg-transparent border-2 border-orange-600 text-white rounded-lg transition-all hover:bg-orange-600 hover:text-white"
          >
            {isIncrementCounter
              ? "Working..."
              : "Tutorial #3: Increment Counter Contract"}
          </button>

          <button
            onClick={handleUnauthenticatedNoteTransfer}
            className="w-full px-6 py-3 text-lg cursor-pointer bg-transparent border-2 border-orange-600 text-white rounded-lg transition-all hover:bg-orange-600 hover:text-white"
          >
            {isUnauthenticatedNoteTransfer
              ? "Working..."
              : "Tutorial #4: Unauthenticated Note Transfer"}
          </button>

          <button
            onClick={handleForeignProcedureInvocation}
            className="w-full px-6 py-3 text-lg cursor-pointer bg-transparent border-2 border-orange-600 text-white rounded-lg transition-all hover:bg-orange-600 hover:text-white"
          >
            {isForeignProcedureInvocation
              ? "Working..."
              : "Tutorial #4: Foreign Procedure Invocation"}
          </button>
        </div>
      </div>
    </main>
  );
}
