"use client";

import React, { useState, useEffect } from "react";
import { useWallet } from "@demox-labs/miden-wallet-adapter-react";
import { WalletMultiButton } from "@demox-labs/miden-wallet-adapter-reactui";
import { createGame } from "../lib/createGame";
import { findGame } from "../lib/findGame";
import { makeMove } from "../lib/makeMove";
import { getEndGameTransactionRequest } from "../lib/endGame";
import {
  CustomTransaction,
  Transaction,
  TransactionType,
} from "@demox-labs/miden-wallet-adapter";

type Player = "X" | "O";
type BoardState = (Player | null)[];

export default function Home() {
  const [board, setBoard] = useState<BoardState>(() => Array(9).fill(null));
  const [currentPlayer, setCurrentPlayer] = useState<Player>("X");
  const [gameId, setGameId] = useState<string | null>(null);
  const [showCreateGameForm, setShowCreateGameForm] = useState(false);
  const [player1Id, setPlayer1Id] = useState("");
  const [player2Id, setPlayer2Id] = useState("");
  const [isCreatingGame, setIsCreatingGame] = useState(false);
  const [isGeneratingPlayer2, setIsGeneratingPlayer2] = useState(false);
  const [gameAccountId, setGameAccountId] = useState("");
  const [isFindingGame, setIsFindingGame] = useState(false);
  const [showFindGameForm, setShowFindGameForm] = useState(false);
  const [isEndingGame, setIsEndingGame] = useState(false);

  const {
    wallet,
    accountId: rawAccountId,
    connected,
    requestTransaction,
  } = useWallet();

  useEffect(() => {
    console.log("🧑‍💼 rawAccountId", rawAccountId);
    console.log("🧑‍💼 connected", connected);
    console.log("🧑‍💼 wallet", wallet);
  }, [rawAccountId, connected, wallet]);

  // Check if there's a winning line for the given player
  const checkWinningLine = (board: BoardState, player: Player): boolean => {
    const winningCombinations = [
      [0, 1, 2],
      [3, 4, 5],
      [6, 7, 8], // rows
      [0, 3, 6],
      [1, 4, 7],
      [2, 5, 8], // columns
      [0, 4, 8],
      [2, 4, 6], // diagonals
    ];

    return winningCombinations.some((combination) =>
      combination.every((index) => board[index] === player),
    );
  };

  // Check if the current connected user has a winning line
  const currentUserHasWon = (): boolean => {
    if (!gameId || !connected) return false;

    // For now, we'll assume the connected user is playing as 'X'
    // TODO: This should be determined based on which player the connected wallet is
    return checkWinningLine(board, currentPlayer === "X" ? "O" : "X");
  };

  const togglePlayer = () => {
    setCurrentPlayer(currentPlayer === "X" ? "O" : "X");
  };

  // Internal function to handle making a move
  const executeMove = async (
    gameId: string,
    accountIdString: string,
    requestTransaction: any,
  ) => {
    try {
      await makeMove(gameId, accountIdString, requestTransaction);
    } catch (error) {
      console.error("Failed to make move:", error);
      alert("Failed to make move. Please try again.");
    }
  };

  const handleSquareClick = async (index: number) => {
    if (board[index] || !gameId || !rawAccountId || !requestTransaction) return;

    // Only run on client side
    if (typeof window === "undefined") return;

    const newBoard = [...board];
    newBoard[index] = currentPlayer;
    setBoard(newBoard);
    setCurrentPlayer(currentPlayer === "X" ? "O" : "X");

    await executeMove(gameId, rawAccountId, requestTransaction);
  };

  const handleEndGame = async () => {
    if (!gameId || !wallet?.adapter?.accountId) {
      alert("Game ID or wallet not available");
      return;
    }

    setIsEndingGame(true);
    try {
      // For now, we'll use playerSlot 1 (assuming connected user is player 1)
      // TODO: This should be determined based on which player the connected wallet is
      const noteRequest = await getEndGameTransactionRequest(
        gameId,
        wallet.adapter.accountId,
        BigInt(1),
      );

      if (!noteRequest) {
        alert("Failed to get end game transaction request");
        return;
      }

      const customTransaction = new CustomTransaction(
        wallet.adapter.accountId,
        noteRequest,
      );

      const transaction = new Transaction(
        TransactionType.Custom,
        customTransaction,
      );
      if (requestTransaction) await requestTransaction(transaction);
      console.log("End game transaction submitted");
    } catch (error) {
      console.error("Failed to end game:", error);
      alert("Failed to end game. Please try again.");
    } finally {
      setIsEndingGame(false);
    }
  };

  // Internal function to generate a new wallet
  const generateNewWallet = async () => {
    try {
      const { AccountStorageMode, NetworkId, AccountInterface, WebClient } =
        await import("@demox-labs/miden-sdk");

      // Create a temporary client for wallet generation
      const { NODE_URL } = await import("../lib/constants");
      const client = await WebClient.createClient(NODE_URL);

      const randomWallet = await client.newWallet(
        AccountStorageMode.public(),
        true,
      );

      const id = randomWallet
        .id()
        .toBech32(NetworkId.Testnet, AccountInterface.BasicWallet)
        .toString();

      setPlayer2Id(id);
      console.log("Generated Player 2 wallet:", id);
    } catch (error) {
      console.error("Failed to generate wallet:", error);
      alert("Failed to generate wallet. Please try again.");
    }
  };

  const handleGeneratePlayer2 = async () => {
    setIsGeneratingPlayer2(true);

    // Only run on client side
    if (typeof window === "undefined") {
      setIsGeneratingPlayer2(false);
      return;
    }

    await generateNewWallet();
    setIsGeneratingPlayer2(false);
  };

  const handleCreateGame = async () => {
    if (!player1Id || !player2Id) {
      alert("Please provide both player account IDs");
      return;
    }

    // Only run on client side
    if (typeof window === "undefined") return;

    setIsCreatingGame(true);

    try {
      const newGameId = await createGame(player1Id, player2Id);
      setGameId(newGameId);
      setShowCreateGameForm(false);
      console.log("Game created with ID:", newGameId);
    } catch (error) {
      console.error("Failed to create game:", error);
      alert("Failed to create game. Please try again.");
    }

    setIsCreatingGame(false);
  };

  // Internal function to find and join a game
  const findAndJoinGame = async (
    gameAccountId: string,
    accountIdString: string,
  ) => {
    try {
      const isPlayerInGame = await findGame(gameAccountId, accountIdString);

      if (isPlayerInGame) {
        setGameId(gameAccountId);
        setShowFindGameForm(false);
        console.log("Successfully joined game:", gameAccountId);
      } else {
        alert("You are not a player in this game or the game was not found.");
      }
    } catch (error) {
      console.error("Failed to find game:", error);
      alert("Failed to find game. Please check the account ID and try again.");
    }
  };

  const handleFindGame = async () => {
    if (!gameAccountId || !rawAccountId) {
      alert("Please provide a game account ID and connect your wallet");
      return;
    }

    // Only run on client side
    if (typeof window === "undefined") return;

    setIsFindingGame(true);
    await findAndJoinGame(gameAccountId, rawAccountId);
    setIsFindingGame(false);
  };

  const handleConnectWallet = async () => {
    if (wallet && connected) {
      try {
        const accountIdString = wallet.adapter.accountId;
        if (accountIdString) {
          setPlayer1Id(accountIdString);
        }
      } catch (error) {
        console.error("Failed to get wallet accounts:", error);
      }
    }
  };

  // Auto-populate player1 ID when wallet connects
  useEffect(() => {
    if (connected) {
      handleConnectWallet();
    }
  }, [connected]);

  return (
    <main className="min-h-screen bg-gradient-to-br from-gray-900 via-gray-800 to-black text-slate-100 relative">
      {/* Title Header - Top Left */}
      <div className="absolute top-6 left-6">
        <div className="bg-gray-800 rounded-2xl shadow-2xl border border-gray-700 px-6 py-4">
          <h1 className="text-2xl font-semibold text-orange-400">
            Miden Tic Tac Toe Game
          </h1>
        </div>
      </div>

      {/* Wallet Connect - Top Right */}
      <div className="absolute top-6 right-6 flex gap-4 items-center">
        {gameId && (
          <div className="bg-gray-800 rounded-lg shadow-lg border border-gray-700 px-4 py-2">
            <p className="text-green-400 font-semibold text-sm">
              Game: {gameId.slice(0, 12)}...
            </p>
          </div>
        )}
        {gameId && currentUserHasWon() && (
          <button
            onClick={handleEndGame}
            disabled={isEndingGame}
            className="px-4 py-2 bg-green-500 hover:bg-green-600 disabled:bg-gray-600 disabled:cursor-not-allowed rounded-lg text-white font-semibold transition-all duration-200 shadow-lg hover:shadow-xl"
          >
            {isEndingGame ? "Ending..." : "End Game"}
          </button>
        )}
        <WalletMultiButton />
        {!gameId && (
          <button
            onClick={togglePlayer}
            className="px-4 py-2 bg-orange-500 hover:bg-orange-600 rounded-lg text-white font-semibold transition-all duration-200 shadow-lg hover:shadow-xl"
          >
            Current: {currentPlayer}
          </button>
        )}
      </div>

      {/* Main Content */}
      <div className="flex items-center justify-center min-h-screen">
        {!gameId ? (
          <div className="bg-gray-800 rounded-2xl shadow-2xl border border-gray-700 p-8 max-w-md w-full">
            {!showCreateGameForm && !showFindGameForm ? (
              <div className="text-center">
                <h2 className="text-2xl font-bold text-orange-400 mb-6">
                  Welcome to Tic Tac Toe
                </h2>
                {!connected ? (
                  <>
                    <p className="text-gray-300 mb-8">
                      Please connect your wallet to create a new game or join an
                      existing one!
                    </p>
                    <p className="text-yellow-400 text-sm mb-4">
                      👆 Use the wallet button in the top right corner
                    </p>
                  </>
                ) : (
                  <>
                    <p className="text-gray-300 mb-8">
                      Choose an option to get started:
                    </p>
                    <div className="space-y-4">
                      <button
                        onClick={() => setShowCreateGameForm(true)}
                        className="w-full px-6 py-3 bg-orange-500 hover:bg-orange-600 rounded-lg text-white font-semibold transition-all duration-200 shadow-lg hover:shadow-xl"
                      >
                        Create New Game
                      </button>
                      <button
                        onClick={() => setShowFindGameForm(true)}
                        className="w-full px-6 py-3 bg-blue-500 hover:bg-blue-600 rounded-lg text-white font-semibold transition-all duration-200 shadow-lg hover:shadow-xl"
                      >
                        Find Game
                      </button>
                    </div>
                  </>
                )}
              </div>
            ) : showCreateGameForm ? (
              <div>
                <h2 className="text-2xl font-bold text-orange-400 mb-6">
                  Create New Game
                </h2>
                <div className="space-y-4">
                  <div>
                    <label className="block text-sm font-medium text-gray-300 mb-2">
                      Player 1 Account ID (Your Wallet)
                    </label>
                    <div className="w-full px-3 py-2 bg-gray-600 border border-gray-500 rounded-lg text-gray-300 font-mono text-sm">
                      {player1Id || "Connect wallet to see account ID"}
                    </div>
                  </div>
                  <div>
                    <label className="block text-sm font-medium text-gray-300 mb-2">
                      Player 2 Account ID
                    </label>
                    <div className="flex gap-2">
                      <input
                        type="text"
                        value={player2Id}
                        onChange={(e) => setPlayer2Id(e.target.value)}
                        className="flex-1 px-3 py-2 bg-gray-700 border border-gray-600 rounded-lg text-white focus:outline-none focus:border-orange-400"
                        placeholder="Enter Player 2 Account ID"
                      />
                      <button
                        onClick={handleGeneratePlayer2}
                        disabled={isGeneratingPlayer2}
                        className="px-4 py-2 bg-blue-500 hover:bg-blue-600 disabled:bg-gray-600 disabled:cursor-not-allowed rounded-lg text-white font-semibold transition-all duration-200 whitespace-nowrap"
                      >
                        {isGeneratingPlayer2 ? "Generating..." : "Generate"}
                      </button>
                    </div>
                  </div>
                  <div className="flex gap-4">
                    <button
                      onClick={handleCreateGame}
                      disabled={isCreatingGame || !player1Id || !player2Id}
                      className="flex-1 px-4 py-2 bg-orange-500 hover:bg-orange-600 disabled:bg-gray-600 disabled:cursor-not-allowed rounded-lg text-white font-semibold transition-all duration-200"
                    >
                      {isCreatingGame ? "Creating..." : "Create Game"}
                    </button>
                    <button
                      onClick={() => setShowCreateGameForm(false)}
                      className="flex-1 px-4 py-2 bg-gray-600 hover:bg-gray-700 rounded-lg text-white font-semibold transition-all duration-200"
                    >
                      Cancel
                    </button>
                  </div>
                </div>
              </div>
            ) : (
              <div>
                <h2 className="text-2xl font-bold text-blue-400 mb-6">
                  Find Game
                </h2>
                <div className="space-y-4">
                  <div>
                    <label className="block text-sm font-medium text-gray-300 mb-2">
                      Game Account ID
                    </label>
                    <input
                      type="text"
                      value={gameAccountId}
                      onChange={(e) => setGameAccountId(e.target.value)}
                      className="w-full px-3 py-2 bg-gray-700 border border-gray-600 rounded-lg text-white focus:outline-none focus:border-blue-400"
                      placeholder="Enter Game Account ID (bech32 format)"
                    />
                  </div>
                  <div className="flex gap-4">
                    <button
                      onClick={handleFindGame}
                      disabled={isFindingGame || !gameAccountId || !connected}
                      className="flex-1 px-4 py-2 bg-blue-500 hover:bg-blue-600 disabled:bg-gray-600 disabled:cursor-not-allowed rounded-lg text-white font-semibold transition-all duration-200"
                    >
                      {isFindingGame ? "Finding..." : "Play"}
                    </button>
                    <button
                      onClick={() => setShowFindGameForm(false)}
                      className="flex-1 px-4 py-2 bg-gray-600 hover:bg-gray-700 rounded-lg text-white font-semibold transition-all duration-200"
                    >
                      Cancel
                    </button>
                  </div>
                </div>
              </div>
            )}
          </div>
        ) : (
          <div className="bg-gray-800 rounded-2xl shadow-2xl border border-gray-700 p-8">
            {/* Tic Tac Toe Board */}
            <div className="grid grid-cols-3 gap-2">
              {board.map((cell, index) => (
                <button
                  key={index}
                  onClick={() => handleSquareClick(index)}
                  className="w-24 h-24 bg-gray-700 hover:bg-gray-600 border-2 border-orange-400 rounded-lg flex items-center justify-center text-4xl font-bold text-orange-400 transition-all duration-200 hover:border-orange-300 hover:shadow-lg hover:shadow-orange-400/20 active:scale-95 flex-shrink-0"
                >
                  {cell}
                </button>
              ))}
            </div>
          </div>
        )}
      </div>
    </main>
  );
}
