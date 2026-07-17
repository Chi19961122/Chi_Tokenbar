// Shared palette for TS-side SVG charts (mirrors CSS vars — v8 gem family).

export const STATUS = {
  near: "#e0a63a",
  locked: "#e05e58",
  muted: "#6f7883",
};

// Categorical series palette for stacked charts (gem family, distinct).
export const SERIES = [
  "#2fa87e",
  "#2b6fb8",
  "#7a4fc9",
  "#c2497a",
  "#2ba3a0",
  "#5b62d4",
  "#8d93e8",
  "#5cc39f",
];

export function seriesColor(i: number): string {
  return SERIES[i % SERIES.length];
}

// Fixed color per known model / agent, so a given series keeps its color across
// pages regardless of enumeration order. Substring match: real keys carry
// suffixes/versions ("opus-4.8", "gpt-5-codex"). Unknown keys fall back to the
// rotating SERIES palette by index.
const NAMED: Array<[string, string]> = [
  // models
  ["sonnet", "#2fa87e"],
  ["opus", "#2b6fb8"],
  ["haiku", "#7a4fc9"],
  // agents (main/root session, then the role fleet)
  ["main", "#2fa87e"],
  ["root", "#2fa87e"],
  ["executor", "#2b6fb8"],
  ["scout", "#7a4fc9"],
  ["verifier", "#c2497a"],
  // codex spans both a model family and its agent
  ["codex", "#5b62d4"],
  // 階段 E multi-tool clients (agent keys "OpenCode" / "Gemini CLI")
  ["opencode", "#2ba3a0"],
  ["gemini", "#c2497a"],
];

/** Color for a named series (model/agent), falling back to the index palette. */
export function keyColor(name: string, i: number): string {
  const n = name.toLowerCase();
  for (const [needle, color] of NAMED) if (n.includes(needle)) return color;
  return seriesColor(i);
}
