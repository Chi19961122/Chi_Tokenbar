"use client"

import { Settings, Share2, RefreshCw } from "lucide-react"
import { useState, useEffect } from "react"

function LiveClock() {
  const [time, setTime] = useState<string>("")

  useEffect(() => {
    const update = () => {
      const now = new Date()
      setTime(now.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" }))
    }
    update()
    const id = setInterval(update, 1000)
    return () => clearInterval(id)
  }, [])

  return (
    <span
      className="tabular-nums"
      style={{ color: "#71717A", fontSize: 10, fontFamily: "var(--font-inter)", fontVariantNumeric: "tabular-nums" }}
    >
      {time}
    </span>
  )
}

export function BottomBar() {
  const [refreshing, setRefreshing] = useState(false)
  const [shared, setShared] = useState(false)

  const handleRefresh = () => {
    setRefreshing(true)
    setTimeout(() => setRefreshing(false), 800)
  }

  const handleShare = () => {
    setShared(true)
    setTimeout(() => setShared(false), 1500)
  }

  return (
    <div
      className="flex items-center justify-between px-5 py-3"
      style={{ borderTop: "1px solid #E4E4E7" }}
    >
      {/* Live clock */}
      <div className="flex items-center gap-2">
        <div
          className="w-1.5 h-1.5 rounded-full animate-pulse"
          style={{ backgroundColor: "#16A34A", animationDuration: "3s" }}
        />
        <LiveClock />
      </div>

      {/* Actions */}
      <div className="flex items-center gap-1">
        <button
          onClick={handleRefresh}
          className="flex items-center justify-center w-7 h-7 rounded-md transition-colors hover:bg-zinc-100 active:scale-95"
          title="Refresh"
          aria-label="Refresh data"
        >
          <RefreshCw
            size={13}
            strokeWidth={1.8}
            style={{
              color: "#71717A",
              transition: "transform 0.7s ease",
              transform: refreshing ? "rotate(360deg)" : "rotate(0deg)",
            }}
          />
        </button>

        <button
          onClick={handleShare}
          className="flex items-center justify-center w-7 h-7 rounded-md transition-colors hover:bg-zinc-100 active:scale-95"
          title="Share snapshot"
          aria-label="Share snapshot"
        >
          {shared ? (
            <span
              className="text-[9px] font-bold"
              style={{ color: "#16A34A", fontFamily: "var(--font-inter)" }}
            >
              ✓
            </span>
          ) : (
            <Share2 size={13} strokeWidth={1.8} style={{ color: "#71717A" }} />
          )}
        </button>

        <div className="w-px h-4 mx-0.5" style={{ backgroundColor: "#E4E4E7" }} />

        <button
          className="flex items-center justify-center w-7 h-7 rounded-md transition-colors hover:bg-zinc-100 active:scale-95"
          title="Settings"
          aria-label="Open settings"
        >
          <Settings size={13} strokeWidth={1.8} style={{ color: "#71717A" }} />
        </button>
      </div>
    </div>
  )
}
