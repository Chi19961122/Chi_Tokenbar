"use client"

import { useMemo } from "react"

function seededRand(seed: number) {
  let s = seed
  return () => {
    s = (s * 1664525 + 1013904223) & 0xffffffff
    return (s >>> 0) / 0xffffffff
  }
}

const LEVEL_COLORS = ["#F4F4F5", "#D4D4D8", "#A1A1AA", "#52525B", "#18181B"]

export function HeatmapCalendar() {
  const rand = useMemo(() => seededRand(7), [])

  // 13 weeks × 7 days = 91 cells; highlight last cell as today
  const weeks = useMemo(() => {
    const cells = Array.from({ length: 91 }, (_, i) => {
      if (i < 8) return 0
      const r = rand()
      if (r < 0.25) return 0
      if (r < 0.55) return 1
      if (r < 0.75) return 2
      if (r < 0.90) return 3
      return 4
    })
    // today = last cell = level 4 (peak)
    cells[90] = 4
    const ws: number[][] = []
    for (let w = 0; w < 13; w++) ws.push(cells.slice(w * 7, w * 7 + 7))
    return ws
  }, [rand])

  const dayLabels = ["", "M", "", "W", "", "F", ""]

  return (
    <div className="px-5 pb-5">
      <div className="flex gap-1">
        {/* day row labels */}
        <div className="flex flex-col" style={{ gap: 3, paddingTop: 1, marginRight: 2 }}>
          {dayLabels.map((d, i) => (
            <div
              key={i}
              style={{
                width: 8,
                height: 10,
                fontSize: 7,
                color: "#A1A1AA",
                fontFamily: "var(--font-inter)",
                lineHeight: "10px",
                textAlign: "right",
              }}
            >
              {d}
            </div>
          ))}
        </div>

        {/* week columns */}
        <div className="flex flex-1 justify-between">
          {weeks.map((week, wi) => (
            <div key={wi} className="flex flex-col" style={{ gap: 3 }}>
              {week.map((level, di) => {
                const isToday = wi === 12 && di === 6
                return (
                  <div
                    key={di}
                    className="rounded-[2px] transition-opacity hover:opacity-60"
                    style={{
                      width: 10,
                      height: 10,
                      backgroundColor: isToday ? "#EC4899" : LEVEL_COLORS[level],
                      outline: isToday ? "1.5px solid #EC489960" : "none",
                      outlineOffset: 1,
                    }}
                    title={isToday ? "Today" : `Level ${level}`}
                  />
                )
              })}
            </div>
          ))}
        </div>
      </div>

      {/* legend */}
      <div className="flex items-center gap-1.5 mt-2.5 justify-end">
        <span style={{ color: "#A1A1AA", fontSize: 9, fontFamily: "var(--font-inter)" }}>less</span>
        {LEVEL_COLORS.map((c, i) => (
          <div
            key={i}
            className="rounded-[2px]"
            style={{ width: 9, height: 9, backgroundColor: c }}
          />
        ))}
        <div className="rounded-[2px]" style={{ width: 9, height: 9, backgroundColor: "#EC4899" }} />
        <span style={{ color: "#A1A1AA", fontSize: 9, fontFamily: "var(--font-inter)" }}>more</span>
      </div>
    </div>
  )
}
