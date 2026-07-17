"use client"

const segments = [
  { label: "Completion", value: 52, color: "#18181B" },
  { label: "Chat", value: 28, color: "#71717A" },
  { label: "Embed", value: 12, color: "#D4D4D8" },
  { label: "Other", value: 8, color: "#EC4899" },
]

function DonutSVG() {
  const size = 56
  const r = 20
  const cx = size / 2
  const cy = size / 2
  const circumference = 2 * Math.PI * r
  const gap = 2

  let offset = 0
  const arcs = segments.map((seg) => {
    const dash = (seg.value / 100) * circumference - gap
    const arc = { ...seg, dashArray: `${dash} ${circumference - dash}`, dashOffset: -offset }
    offset += (seg.value / 100) * circumference
    return arc
  })

  return (
    <svg width={size} height={size} viewBox={`0 0 ${size} ${size}`}>
      {/* background ring */}
      <circle cx={cx} cy={cy} r={r} fill="none" stroke="#F4F4F5" strokeWidth={7} />
      {arcs.map((arc, i) => (
        <circle
          key={i}
          cx={cx}
          cy={cy}
          r={r}
          fill="none"
          stroke={arc.color}
          strokeWidth={7}
          strokeDasharray={arc.dashArray}
          strokeDashoffset={arc.dashOffset}
          transform={`rotate(-90 ${cx} ${cy})`}
          strokeLinecap="butt"
        />
      ))}
    </svg>
  )
}

export function ActivityDonut() {
  return (
    <div className="flex items-center gap-5">
      <DonutSVG />
      <div className="flex flex-col gap-1.5">
        {segments.map((seg) => (
          <div key={seg.label} className="flex items-center gap-2">
            <div
              className="rounded-full flex-shrink-0"
              style={{ width: 5, height: 5, backgroundColor: seg.color }}
            />
            <span
              className="text-[10px]"
              style={{ color: "#71717A", fontFamily: "var(--font-inter)" }}
            >
              {seg.label}
            </span>
            <span
              className="text-[10px] font-semibold tabular-nums ml-auto"
              style={{ color: "#09090B", fontFamily: "var(--font-inter)" }}
            >
              {seg.value}%
            </span>
          </div>
        ))}
      </div>
    </div>
  )
}
