// 供應商多選 (T-916, slimmed T-917): the unified source selection. One
// multi-select of three sources replaces the old 3-way `providers` filter plus
// the two `tool_*` checkboxes (OpenCode/Gemini removed in T-917). These are the
// pure derivations the island, panel, analytics cache key and settings UI all
// read from — kept here so the display matrix (which quota providers → which
// island layout) stays unit-testable.

import type { ProviderFilter } from "./types";

export type SourceId = "claude" | "codex" | "grok";

/** Canonical order for the chip row and the fresh-install default. */
export const ALL_SOURCES: SourceId[] = ["claude", "codex", "grok"];

/** Membership test tolerant of an undefined list (pre-load / mock edge). */
export function hasSource(sources: readonly string[] | undefined, id: SourceId): boolean {
  return !!sources && sources.includes(id);
}

/**
 * Island layout mode derived from which *quota* providers are selected:
 *   both selected → "both" (the two stacked)
 *   exactly one   → that single provider
 *   neither       → "none" (empty-state pill; the usage-only sources produce no
 *                   island quota, so nothing to show)
 *
 * Only claude/codex drive the island — Grok is a context-fill limit that shows
 * on the panel/digest but never on the island pill.
 */
export function islandMode(sources: readonly string[] | undefined): ProviderFilter | "none" {
  const claude = hasSource(sources, "claude");
  const codex = hasSource(sources, "codex");
  if (claude && codex) return "both";
  if (claude) return "claude";
  if (codex) return "codex";
  return "none";
}

/** Stable cache-key slice for the current selection (order-independent). */
export function sourcesKey(sources: readonly string[] | undefined): string {
  return [...(sources ?? ALL_SOURCES)].sort().join(",");
}
