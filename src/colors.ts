// Shared palette for TS-side SVG charts (mirrors CSS vars).

export const STATUS = {
  safe: "#34d399",
  near: "#fbbf24",
  locked: "#f87171",
  muted: "#8a929d",
};

// Categorical series palette for stacked charts (brand-neutral, distinct).
export const SERIES = [
  "#8b7cf6",
  "#34d399",
  "#fbbf24",
  "#60a5fa",
  "#f472b6",
  "#22d3ee",
  "#a3e635",
  "#fb923c",
];

export function seriesColor(i: number): string {
  return SERIES[i % SERIES.length];
}
