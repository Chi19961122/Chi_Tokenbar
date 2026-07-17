"use client"

interface StatTileProps {
  label: string
  value: string
  sub?: string
  accent?: boolean
}

function StatTile({ label, value, sub, accent }: StatTileProps) {
  return (
    <div
      className="flex-1 rounded-[6px] px-3 py-3"
      style={{
        backgroundColor: accent ? "#09090B" : "#FFFFFF",
        border: accent ? "none" : "1px solid #E4E4E7",
      }}
    >
      <div
        className="text-[9px] uppercase tracking-[0.12em] mb-1.5 font-medium"
        style={{
          color: accent ? "#71717A" : "#A1A1AA",
          fontFamily: "var(--font-inter)",
        }}
      >
        {label}
      </div>
      <div
        className="font-extrabold tabular-nums leading-none"
        style={{
          fontSize: 22,
          letterSpacing: "-0.04em",
          color: accent ? "#FAFAFA" : "#09090B",
          fontFamily: "var(--font-inter)",
          fontWeight: 800,
        }}
      >
        {value}
      </div>
      {sub && (
        <div
          className="text-[9px] mt-1"
          style={{
            color: accent ? "#52525B" : "#A1A1AA",
            fontFamily: "var(--font-inter)",
          }}
        >
          {sub}
        </div>
      )}
    </div>
  )
}

export function StatTiles() {
  return (
    <div className="px-5 pb-5">
      <div className="flex gap-2">
        <StatTile label="Est. Cost" value="$4.28" sub="this month" accent />
        <StatTile label="Peak Day" value="Jul 9" sub="81k tokens" />
        <StatTile label="Streak" value="14d" sub="active" />
      </div>
    </div>
  )
}
