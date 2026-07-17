"use client"

import { Zap } from "lucide-react"

interface StatusPillProps {
  percent: number
  timeLeft: string
  status: "safe" | "warn" | "locked" | "stale"
}

const statusColors = {
  safe: "#16A34A",
  warn: "#D97706",
  locked: "#DC2626",
  stale: "#A1A1AA",
}

const statusLabels = {
  safe: "safe",
  warn: "near limit",
  locked: "locked",
  stale: "stale",
}

export function StatusPill({ percent, timeLeft, status }: StatusPillProps) {
  const color = statusColors[status]

  return (
    <div className="flex items-center justify-between px-5 py-4">
      {/* Left: glyph + label */}
      <div className="flex items-center gap-2.5">
        <div
          className="flex items-center justify-center w-6 h-6 rounded-sm"
          style={{ backgroundColor: "#18181B" }}
        >
          <Zap size={12} fill="#EC4899" stroke="none" />
        </div>
        <span
          className="text-[10px] font-semibold tracking-[0.14em] uppercase"
          style={{ color: "#71717A", fontFamily: "var(--font-inter)" }}
        >
          TokenBar
        </span>
      </div>

      {/* Right: status pill */}
      <div
        className="flex items-center gap-2 px-3 py-1.5 rounded-full"
        style={{
          backgroundColor: `${color}12`,
          border: `1px solid ${color}30`,
        }}
      >
        {/* Pulse dot */}
        <span className="relative flex h-1.5 w-1.5">
          <span
            className="absolute inline-flex h-full w-full rounded-full opacity-75 animate-ping"
            style={{ backgroundColor: color, animationDuration: "2.5s" }}
          />
          <span
            className="relative inline-flex rounded-full h-1.5 w-1.5"
            style={{ backgroundColor: color }}
          />
        </span>

        <span
          className="text-[11px] font-bold tabular-nums tracking-tight"
          style={{ color: "#09090B", fontFamily: "var(--font-inter)" }}
        >
          {percent}% left
        </span>
        <span style={{ color: "#E4E4E7", fontSize: 10 }}>·</span>
        <span
          className="text-[11px] tabular-nums"
          style={{ color: "#71717A", fontFamily: "var(--font-inter)" }}
        >
          {timeLeft}
        </span>
      </div>
    </div>
  )
}
