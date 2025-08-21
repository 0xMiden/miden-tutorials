"use client";

import { useState } from "react";

type Player = "X" | "O";
type BoardState = (Player | null)[][];

export default function Home() {
  const [board, setBoard] = useState<BoardState>(() =>
    Array(3)
      .fill(null)
      .map(() => Array(3).fill(null)),
  );
  const [currentPlayer, setCurrentPlayer] = useState<Player>("X");

  const handleSquareClick = (row: number, col: number) => {
    if (board[row][col]) return;

    const newBoard = [...board];
    newBoard[row][col] = currentPlayer;
    setBoard(newBoard);
    setCurrentPlayer(currentPlayer === "X" ? "O" : "X");
  };

  const togglePlayer = () => {
    // TODO: add wallet adapter here
    setCurrentPlayer(currentPlayer === "X" ? "O" : "X");
  };

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

      {/* User Toggle - Top Right */}
      <div className="absolute top-6 right-6">
        <button
          onClick={togglePlayer}
          className="px-4 py-2 bg-orange-500 hover:bg-orange-600 rounded-lg text-white font-semibold transition-all duration-200 shadow-lg hover:shadow-xl"
        >
          Current: {currentPlayer}
        </button>
      </div>

      {/* Game Board Container */}
      <div className="flex items-center justify-center min-h-screen">
        <div className="bg-gray-800 rounded-2xl shadow-2xl border border-gray-700 p-8">
          {/* Tic Tac Toe Board */}
          <div className="grid grid-cols-3 gap-2">
            {board.map((row, rowIndex) =>
              row.map((cell, colIndex) => (
                <button
                  key={`${rowIndex}-${colIndex}`}
                  onClick={() => handleSquareClick(rowIndex, colIndex)}
                  className="w-24 h-24 bg-gray-700 hover:bg-gray-600 border-2 border-orange-400 rounded-lg flex items-center justify-center text-4xl font-bold text-orange-400 transition-all duration-200 hover:border-orange-300 hover:shadow-lg hover:shadow-orange-400/20 active:scale-95 flex-shrink-0"
                >
                  {cell}
                </button>
              )),
            )}
          </div>
        </div>
      </div>
    </main>
  );
}
