"use client";
import { useState } from "react";
import { createMintConsume } from "../lib/createMintConsume";
import { multiSendWithDelegatedProver } from "../lib/multiSendWithDelegatedProver";
import { incrementCounterContract } from "../lib/incrementCounterContract";
import { unauthenticatedNoteTransfer } from "../lib/unauthenticatedNoteTransfer";
import { foreignProcedureInvocation } from "../lib/foreignProcedureInvocation";

type TutorialState = "running" | "passed" | "failed";
type TutorialStatus = { state: TutorialState; error?: string };
type TutorialStatusMap = Record<string, TutorialStatus>;

const updateTutorialStatus = (
  name: string,
  state: TutorialState,
  error?: unknown,
) => {
  if (typeof window === "undefined") return;
  const win = window as Window & { __tutorialStatus?: TutorialStatusMap };
  const current = win.__tutorialStatus ?? {};
  current[name] = {
    state,
    error: error instanceof Error ? `${error.name}: ${error.message}` : error
      ? String(error)
      : undefined,
  };
  win.__tutorialStatus = current;
};

const runTutorial = async (
  name: string,
  action: () => Promise<void>,
  setIsRunning: (value: boolean) => void,
) => {
  setIsRunning(true);
  updateTutorialStatus(name, "running");
  try {
    await action();
    updateTutorialStatus(name, "passed");
  } catch (error) {
    console.error(`[tutorial:${name}]`, error);
    updateTutorialStatus(name, "failed", error);
  } finally {
    setIsRunning(false);
  }
};

export default function Home() {
  const [isCreatingNotes, setIsCreatingNotes] = useState(false);
  const [isMultiSendNotes, setIsMultiSendNotes] = useState(false);
  const [isIncrementCounter, setIsIncrementCounter] = useState(false);
  const [isUnauthenticatedNoteTransfer, setIsUnauthenticatedNoteTransfer] = useState(false);
  const [isForeignProcedureInvocation, setIsForeignProcedureInvocation] = useState(false);

  const handleCreateMintConsume = async () => {
    await runTutorial("createMintConsume", createMintConsume, setIsCreatingNotes);
  };

  const handleMultiSendNotes = async () => {
    await runTutorial(
      "multiSendWithDelegatedProver",
      multiSendWithDelegatedProver,
      setIsMultiSendNotes,
    );
  };

  const handleIncrementCounterContract = async () => {
    await runTutorial(
      "incrementCounterContract",
      incrementCounterContract,
      setIsIncrementCounter,
    );
  };

  const handleUnauthenticatedNoteTransfer = async () => {
    await runTutorial(
      "unauthenticatedNoteTransfer",
      unauthenticatedNoteTransfer,
      setIsUnauthenticatedNoteTransfer,
    );
  };

  const handleForeignProcedureInvocation = async () => {
    await runTutorial(
      "foreignProcedureInvocation",
      foreignProcedureInvocation,
      setIsForeignProcedureInvocation,
    );
  };

  return (
    <main className="min-h-screen flex items-center justify-center bg-gradient-to-br from-gray-900 via-gray-800 to-black text-slate-800 dark:text-slate-100">
      <div className="text-center">
        <h1 className="text-4xl font-semibold mb-4">Miden Web App</h1>
        <p className="mb-6">Open your browser console to see WebClient logs.</p>

        <div className="max-w-sm w-full bg-gray-800/20 border border-gray-600 rounded-2xl p-6 mx-auto flex flex-col gap-4">
          <button
            onClick={handleCreateMintConsume}
            data-testid="tutorial-createMintConsume"
            className="w-full px-6 py-3 text-lg cursor-pointer bg-transparent border-2 border-orange-600 text-white rounded-lg transition-all hover:bg-orange-600 hover:text-white"
          >
            {isCreatingNotes
              ? "Working..."
              : "Tutorial #1: Create, Mint, Consume Notes"}
          </button>

          <button
            onClick={handleMultiSendNotes}
            data-testid="tutorial-multiSendWithDelegatedProver"
            className="w-full px-6 py-3 text-lg cursor-pointer bg-transparent border-2 border-orange-600 text-white rounded-lg transition-all hover:bg-orange-600 hover:text-white"
          >
            {isMultiSendNotes
              ? "Working..."
              : "Tutorial #2: Send 1 to N P2ID Notes with Delegated Proving"}
          </button>

          <button
            onClick={handleIncrementCounterContract}
            data-testid="tutorial-incrementCounterContract"
            className="w-full px-6 py-3 text-lg cursor-pointer bg-transparent border-2 border-orange-600 text-white rounded-lg transition-all hover:bg-orange-600 hover:text-white"
          >
            {isIncrementCounter
              ? "Working..."
              : "Tutorial #3: Increment Counter Contract"}
          </button>

          <button
            onClick={handleUnauthenticatedNoteTransfer}
            data-testid="tutorial-unauthenticatedNoteTransfer"
            className="w-full px-6 py-3 text-lg cursor-pointer bg-transparent border-2 border-orange-600 text-white rounded-lg transition-all hover:bg-orange-600 hover:text-white"
          >
            {isUnauthenticatedNoteTransfer
              ? "Working..."
              : "Tutorial #4: Unauthenticated Note Transfer"}
          </button>

          <button
            onClick={handleForeignProcedureInvocation}
            data-testid="tutorial-foreignProcedureInvocation"
            className="w-full px-6 py-3 text-lg cursor-pointer bg-transparent border-2 border-orange-600 text-white rounded-lg transition-all hover:bg-orange-600 hover:text-white"
          >
            {isForeignProcedureInvocation
              ? "Working..."
              : "Tutorial #5: Foreign Procedure Invocation"}
          </button>
        </div>
      </div>
    </main>
  );
}
