"use client"

import { useEffect, useState } from "react"
import { StatusPill } from "./StatusPill"
import { GaugeCard } from "./GaugeCard"
import { SectionHeader } from "./SectionHeader"
import { BarChart30 } from "./BarChart30"
import { HeatmapCalendar } from "./HeatmapCalendar"
import { ActivityDonut } from "./ActivityDonut"
import { ProjectBars } from "./ProjectBars"
import { StatTiles } from "./StatTiles"
import { BottomBar } from "./BottomBar"

export function TokenBarPanel() {
  const [visible, setVisible] = useState(false)
  useEffect(() => { setTimeout(() => setVisible(true), 60) }, [])

  return (
    <div
      className="flex flex-col"
      style={{
        width: 380,
        backgroundColor: "#FAFAFA",
        border: "1px solid #E4E4E7",
        borderRadius: 10,
        boxShadow: "0 8px 40px rgba(0,0,0,0.10), 0 1px 4px rgba(0,0,0,0.05)",
        fontFamily: "var(--font-inter)",
        opacity: visible ? 1 : 0,
        transform: visible ? "translateY(0)" : "translateY(6px)",
        transition: "opacity 0.4s ease, transform 0.4s ease",
        overflowX: "hidden",
      }}
    >
      {/* ── STATUS PILL ── */}
      <StatusPill percent={62} timeLeft="2h 10m" status="safe" />

      {/* thin rule */}
      <div style={{ height: 1, backgroundColor: "#E4E4E7" }} />

      {/* ── LIMITS SECTION ── */}
      <SectionHeader
        index="01"
        title="Limits"
        editorialLabel="What's left in the tank"
      />

      <GaugeCard
        name="Claude Code"
        icon="◆"
        windowRemaining={3.1}
        windowTotal={5}
        windowUnit="h"
        windowStatus="safe"
        windowReset="in 1h 50m"
        weeklyRemaining={124000}
        weeklyTotal={200000}
        weeklyStatus="safe"
        weeklyReset="Sun 00:00"
      />

      <GaugeCard
        name="Codex"
        icon="○"
        windowRemaining={1.2}
        windowTotal={5}
        windowUnit="h"
        windowStatus="warn"
        windowReset="in 3h 40m"
        weeklyRemaining={31000}
        weeklyTotal={150000}
        weeklyStatus="warn"
        weeklyReset="Mon 00:00"
      />

      {/* ── USAGE SECTION ── */}
      <SectionHeader
        index="02"
        title="Usage"
        editorialLabel="Thirty days of consumption"
      />

      {/* 30-day bar chart */}
      <div className="px-5 pb-1">
        <span
          className="text-[10px] uppercase tracking-[0.12em]"
          style={{ color: "#A1A1AA", fontFamily: "var(--font-inter)" }}
        >
          Daily tokens
        </span>
      </div>
      <BarChart30 />

      {/* thin rule */}
      <div className="mx-5" style={{ height: 1, backgroundColor: "#E4E4E7" }} />

      {/* Heatmap */}
      <div className="px-5 pt-4 pb-1">
        <span
          className="text-[10px] uppercase tracking-[0.12em]"
          style={{ color: "#A1A1AA", fontFamily: "var(--font-inter)" }}
        >
          Activity calendar
        </span>
      </div>
      <HeatmapCalendar />

      {/* thin rule */}
      <div className="mx-5" style={{ height: 1, backgroundColor: "#E4E4E7" }} />

      {/* Donut + Project bars side-by-side */}
      <div className="px-5 pt-4 pb-4 flex gap-6">
        <div className="flex-shrink-0">
          <div
            className="text-[10px] uppercase tracking-[0.12em] mb-3"
            style={{ color: "#A1A1AA", fontFamily: "var(--font-inter)" }}
          >
            By type
          </div>
          <ActivityDonut />
        </div>
        <div className="flex-1 min-w-0">
          <div
            className="text-[10px] uppercase tracking-[0.12em] mb-3"
            style={{ color: "#A1A1AA", fontFamily: "var(--font-inter)" }}
          >
            By project
          </div>
          <ProjectBars />
        </div>
      </div>

      {/* thin rule */}
      <div className="mx-5" style={{ height: 1, backgroundColor: "#E4E4E7" }} />

      {/* Stat tiles */}
      <div className="pt-4" />
      <StatTiles />

      {/* ── BOTTOM BAR ── */}
      <BottomBar />
    </div>
  )
}
