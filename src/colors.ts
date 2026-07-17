// Shared neutral palette for TS-side charts (DESIGN-SPEC chart-scale).

export const STATUS = {
  near: "#e0a63a",
  locked: "#e05e58",
  muted: "#6f7883",
};

// Charts intentionally carry no provider/status family colors. The pink accent
// is reserved for explicit "today" / first-place marks in their renderers.
export const SERIES = [
  "#18181B",
  "#52525B",
  "#A1A1AA",
  "#D4D4D8",
  "#F4F4F5",
];

export function seriesColor(i: number): string {
  return SERIES[i % SERIES.length];
}

// Fixed neutral shade per known model / agent, so a given series keeps its
// shade across pages without reintroducing provider-family colors.
const NAMED: Array<[string, string]> = [
  // models
  ["sonnet", "#52525B"],
  ["opus", "#18181B"],
  ["haiku", "#A1A1AA"],
  // agents (main/root session, then the role fleet)
  ["main", "#18181B"],
  ["root", "#18181B"],
  ["executor", "#52525B"],
  ["scout", "#A1A1AA"],
  ["verifier", "#D4D4D8"],
  // codex spans both a model family and its agent
  ["codex", "#52525B"],
  // 階段 E multi-tool clients (agent keys "OpenCode" / "Gemini CLI")
  ["opencode", "#A1A1AA"],
  ["gemini", "#D4D4D8"],
];

/** Color for a named series (model/agent), falling back to the index palette. */
export function keyColor(name: string, i: number): string {
  const n = name.toLowerCase();
  for (const [needle, color] of NAMED) if (n.includes(needle)) return color;
  return seriesColor(i);
}
