import type { IslandAux } from "./types";

/**
 * Whether the island right-side aux readout needs a full analytics "today"
 * scan. Stage 1A: `off` must not trigger log parsing every 60s.
 */
export function islandAuxNeedsAnalytics(
  aux: IslandAux | string | null | undefined,
): boolean {
  const a = aux ?? "tok_per_min";
  return a === "tok_per_min" || a === "cost_today";
}
