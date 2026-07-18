// Shared neutral palette for TS-side charts (DESIGN-SPEC chart-scale).

export const STATUS = {
  near: "#e0a63a",
  locked: "#e05e58",
  muted: "#6f7883",
};

// Charts intentionally carry no provider/status family colors. The pink accent
// is reserved for explicit "today" / first-place marks in their renderers.
//
// Values are CSS variables (the "ink ramp" defined in styles.css :root/.dark),
// not literal hex, so a series keeps its shade in light mode and gets a
// light-on-dark counterpart in dark mode. These strings are dropped into inline
// `style="fill:…"` / `style="background:…"` where var() resolves (a bare `fill=`
// presentation attribute would NOT resolve var(), so the SVG renderers use
// style). Light values: 18181B, 52525B, A1A1AA, D4D4D8, F4F4F5.
export const SERIES = [
  "var(--ink-900)",
  "var(--ink-700)",
  "var(--ink-400)",
  "var(--ink-300)",
  "var(--ink-100)",
];

export function seriesColor(i: number): string {
  return SERIES[i % SERIES.length];
}

// Fixed neutral shade per known model / agent, so a given series keeps its
// shade across pages without reintroducing provider-family colors.
const NAMED: Array<[string, string]> = [
  // models
  ["sonnet", "var(--ink-700)"],
  ["opus", "var(--ink-900)"],
  ["haiku", "var(--ink-400)"],
  // agents (main/root session, then the role fleet)
  ["main", "var(--ink-900)"],
  ["root", "var(--ink-900)"],
  ["executor", "var(--ink-700)"],
  ["scout", "var(--ink-400)"],
  ["verifier", "var(--ink-300)"],
  // codex spans both a model family and its agent
  ["codex", "var(--ink-700)"],
  // Grok usage (agent key "Grok CLI" / models "grok-*")
  ["grok", "var(--ink-400)"],
];

/** Color for a named series (model/agent), falling back to the index palette. */
export function keyColor(name: string, i: number): string {
  const n = name.toLowerCase();
  for (const [needle, color] of NAMED) if (n.includes(needle)) return color;
  return seriesColor(i);
}
