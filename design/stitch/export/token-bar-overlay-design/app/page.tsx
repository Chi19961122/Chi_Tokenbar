import { TokenBarPanel } from "@/components/tokenbar/TokenBarPanel"

export default function Home() {
  return (
    <main
      className="min-h-screen flex items-start justify-center py-12"
      style={{ backgroundColor: "#F0F0EE" }}
    >
      {/* Simulate desktop context — blurred "app" behind the overlay */}
      <div
        className="absolute inset-0 pointer-events-none select-none overflow-hidden"
        aria-hidden="true"
        style={{ opacity: 0.06 }}
      >
        <div
          className="absolute top-16 left-12 text-[120px] font-extrabold"
          style={{ color: "#09090B", fontFamily: "var(--font-inter)", letterSpacing: "-0.06em" }}
        >
          VS Code
        </div>
        <div
          className="absolute bottom-24 right-16 text-[80px] font-light"
          style={{ color: "#09090B", fontFamily: "var(--font-inter)" }}
        >
          github.com
        </div>
      </div>

      {/* The actual 380px overlay panel */}
      <div className="relative z-10">
        <TokenBarPanel />
      </div>
    </main>
  )
}
