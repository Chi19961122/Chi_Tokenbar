"use client"

interface SectionHeaderProps {
  index: string
  title: string
  editorialLabel?: string
}

export function SectionHeader({ index, title, editorialLabel }: SectionHeaderProps) {
  return (
    <div className="px-5 pt-7 pb-4" style={{ borderTop: "1px solid #E4E4E7" }}>
      <div className="flex items-baseline gap-2.5">
        <span
          className="tabular-nums leading-none"
          style={{ fontSize: 11, color: "#A1A1AA", fontFamily: "var(--font-inter)", fontWeight: 600 }}
        >
          {index}
        </span>
        <h2
          className="text-[13px] font-bold uppercase tracking-[0.16em]"
          style={{ color: "#09090B", fontFamily: "var(--font-inter)" }}
        >
          {title}
        </h2>
      </div>
      {editorialLabel && (
        <p
          className="mt-1 italic"
          style={{ fontSize: 15, color: "#52525B", fontFamily: "var(--font-serif)", fontStyle: "italic" }}
        >
          {editorialLabel}
        </p>
      )}
    </div>
  )
}
