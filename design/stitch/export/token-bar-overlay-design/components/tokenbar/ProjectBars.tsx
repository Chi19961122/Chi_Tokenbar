"use client"

const projects = [
  { name: "web-app", tokens: 284, max: 400 },
  { name: "api-service", tokens: 197, max: 400 },
  { name: "ml-pipeline", tokens: 143, max: 400 },
  { name: "cli-tools", tokens: 88, max: 400 },
  { name: "docs-gen", tokens: 41, max: 400 },
]

export function ProjectBars() {
  const maxTokens = Math.max(...projects.map((p) => p.tokens))

  return (
    <div className="flex flex-col gap-2.5">
      {projects.map((proj, i) => {
        const pct = (proj.tokens / maxTokens) * 100
        const isTop = i === 0
        return (
          <div key={proj.name} className="flex items-center gap-3">
            <span
              className="text-[10px] tabular-nums text-right flex-shrink-0"
              style={{
                width: 72,
                color: isTop ? "#09090B" : "#71717A",
                fontFamily: "var(--font-inter)",
                fontWeight: isTop ? 600 : 400,
                overflow: "hidden",
                textOverflow: "ellipsis",
                whiteSpace: "nowrap",
              }}
            >
              {proj.name}
            </span>
            <div
              className="flex-1 relative rounded-full overflow-hidden"
              style={{ height: 3, backgroundColor: "#E4E4E7" }}
            >
              <div
                className="absolute left-0 top-0 h-full rounded-full transition-all duration-500"
                style={{
                  width: `${pct}%`,
                  backgroundColor: isTop ? "#EC4899" : "#18181B",
                }}
              />
            </div>
            <span
              className="text-[10px] tabular-nums flex-shrink-0"
              style={{
                width: 36,
                color: isTop ? "#09090B" : "#A1A1AA",
                fontFamily: "var(--font-inter)",
                fontWeight: isTop ? 600 : 400,
              }}
            >
              {proj.tokens}k
            </span>
          </div>
        )
      })}
    </div>
  )
}
