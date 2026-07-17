"use client"

const STATUS = {
  safe: { color: "#16A34A", label: "healthy" },
  warn: { color: "#D97706", label: "near limit" },
  locked: { color: "#DC2626", label: "locked" },
  stale: { color: "#A1A1AA", label: "stale" },
} as const

type Status = keyof typeof STATUS

function GaugeBar({ pct, status }: { pct: number; status: Status }) {
  return (
    <div className="relative h-[3px] w-full overflow-hidden rounded-full" style={{ backgroundColor: "#E4E4E7" }}>
      <div
        className="absolute left-0 top-0 h-full rounded-full transition-all duration-700"
        style={{ width: `${Math.max(0, Math.min(100, pct))}%`, backgroundColor: STATUS[status].color }}
      />
    </div>
  )
}

function GaugeRow({
  kicker,
  pct,
  status,
  detail,
  reset,
}: {
  kicker: string
  pct: number
  status: Status
  detail: string
  reset: string
}) {
  const { color } = STATUS[status]
  return (
    <div>
      {/* editorial kicker */}
      <div
        className="mb-1 text-[9px] uppercase tracking-[0.16em]"
        style={{ color: "#A1A1AA", fontFamily: "var(--font-inter)" }}
      >
        {kicker}
      </div>

      {/* hero numeral */}
      <div className="flex items-baseline gap-1.5">
        <span
          className="tabular-nums leading-[0.82]"
          style={{
            fontSize: 60,
            color,
            fontFamily: "var(--font-inter)",
            letterSpacing: "-0.055em",
            fontWeight: 800,
          }}
        >
          {pct}
        </span>
        <span
          className="leading-none"
          style={{ fontSize: 15, color, fontFamily: "var(--font-inter)", fontWeight: 700 }}
        >
          %
        </span>
        <span
          className="ml-1 italic leading-none"
          style={{ fontSize: 13, color: "#A1A1AA", fontFamily: "var(--font-serif)", fontStyle: "italic" }}
        >
          left
        </span>
      </div>

      {/* gauge bar */}
      <div className="mt-3">
        <GaugeBar pct={pct} status={status} />
        <div className="mt-2 flex flex-col gap-0.5">
          <span className="text-[10px] tabular-nums" style={{ color: "#52525B", fontFamily: "var(--font-inter)" }}>
            {detail} left
          </span>
          <span className="text-[9px]" style={{ color: "#A1A1AA", fontFamily: "var(--font-inter)" }}>
            resets {reset}
          </span>
        </div>
      </div>
    </div>
  )
}

interface GaugeCardProps {
  name: string
  icon: string
  windowRemaining: number
  windowTotal: number
  windowUnit: string
  windowStatus: Status
  windowReset: string
  weeklyRemaining: number
  weeklyTotal: number
  weeklyStatus: Status
  weeklyReset: string
}

export function GaugeCard({
  name,
  icon,
  windowRemaining,
  windowTotal,
  windowUnit,
  windowStatus,
  windowReset,
  weeklyRemaining,
  weeklyTotal,
  weeklyStatus,
  weeklyReset,
}: GaugeCardProps) {
  const windowPct = Math.round((windowRemaining / windowTotal) * 100)
  const weeklyPct = Math.round((weeklyRemaining / weeklyTotal) * 100)
  const worst = windowStatus === "locked" || weeklyStatus === "locked" ? "locked" : windowStatus === "warn" || weeklyStatus === "warn" ? "warn" : "safe"

  return (
    <div className="px-5 py-6" style={{ borderBottom: "1px solid #E4E4E7" }}>
      {/* card header */}
      <div className="mb-6 flex items-center justify-between">
        <div className="flex items-center gap-2">
          <span style={{ fontSize: 14, color: "#18181B" }}>{icon}</span>
          <span
            className="text-[11px] font-semibold uppercase tracking-[0.12em]"
            style={{ color: "#09090B", fontFamily: "var(--font-inter)" }}
          >
            {name}
          </span>
        </div>
        <span
          className="flex items-center gap-1.5 text-[10px] font-medium tracking-wide"
          style={{ color: STATUS[worst].color, fontFamily: "var(--font-inter)" }}
        >
          <span className="inline-block h-1.5 w-1.5 rounded-full" style={{ backgroundColor: STATUS[worst].color }} />
          {STATUS[worst].label}
        </span>
      </div>

      {/* two hero gauges */}
      <div className="grid grid-cols-2 gap-x-6">
        <GaugeRow
          kicker="5-hour window"
          pct={windowPct}
          status={windowStatus}
          detail={`${windowRemaining}${windowUnit} / ${windowTotal}${windowUnit}`}
          reset={windowReset}
        />
        <GaugeRow
          kicker="Weekly"
          pct={weeklyPct}
          status={weeklyStatus}
          detail={`${(weeklyRemaining / 1000).toFixed(0)}k / ${(weeklyTotal / 1000).toFixed(0)}k`}
          reset={weeklyReset}
        />
      </div>
    </div>
  )
}
