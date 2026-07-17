"use client"

import { useMemo } from "react"

const RAW_DATA = [
  18, 42, 35, 67, 12, 55, 48, 30, 72, 25, 60, 44, 38, 81, 20, 53, 47, 66, 29, 74, 41, 58, 33, 69,
  22, 50, 45, 77, 36, 62,
]

export function BarChart30() {
  const maxVal = Math.max(...RAW_DATA)

  const today = useMemo(() => {
    const d = new Date()
    return d
  }, [])

  const labels = useMemo(() => {
    return RAW_DATA.map((_, i) => {
      const d = new Date(today)
      d.setDate(d.getDate() - (29 - i))
      return d.getDate()
    })
  }, [today])

  return (
    <div className="px-5 pb-5">
      <div className="flex items-end justify-between" style={{ height: 56, gap: 2 }}>
        {RAW_DATA.map((val, i) => {
          const heightPct = (val / maxVal) * 100
          const isToday = i === 29
          const isHigh = val > 60

          return (
            <div
              key={i}
              className="flex-1 rounded-[1px] transition-opacity duration-200 hover:opacity-70"
              style={{
                height: `${heightPct}%`,
                backgroundColor: isToday ? "#EC4899" : isHigh ? "#18181B" : "#D4D4D8",
                minHeight: 2,
              }}
              title={`Day ${labels[i]}: ${val}k tokens`}
            />
          )
        })}
      </div>
      {/* x-axis minimal labels */}
      <div className="flex justify-between mt-2">
        <span
          className="text-[9px] tabular-nums"
          style={{ color: "#A1A1AA", fontFamily: "var(--font-inter)" }}
        >
          30d ago
        </span>
        <span
          className="text-[9px] tabular-nums"
          style={{ color: "#EC4899", fontFamily: "var(--font-inter)", fontWeight: 600 }}
        >
          today
        </span>
      </div>
    </div>
  )
}
