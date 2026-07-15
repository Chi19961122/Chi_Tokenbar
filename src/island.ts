// Island (collapsed) view — fuel capsule(s) + % left, per the Live Island design.
// Layout follows the global display filter (settings.providers): both providers
// side-by-side (default), or a single provider. The backend already filtered the
// snapshot, so `mode` only decides the layout, never what data exists.
// Right side: today's burn rate (tok/min).

import type { Limit, ProviderFilter, Snapshot } from "./types";
import { fmtTokens, pctLeft } from "./format";
import { providerIcon } from "./icons";

export interface IslandOpts {
  mode: ProviderFilter;
  /** Today's average tokens/minute, for the right-side readout. */
  tokPerMin: number | null;
}

/** What a press on the island means. */
export type IslandIntent = "hide" | "expand" | "none";

/** Marks the hide-to-tray button; the one place this selector is written. */
const HIDE_SEL = "[data-hide]";

/**
 * Route a press on the island: hide to tray, expand the panel, or nothing.
 *
 * Split out of main.ts's listeners because the island has to serve three
 * gestures on one 340×52 pill and the interesting part is which one wins.
 *
 * `dragged` is checked **first**, deliberately. The pill is small, so a drag to
 * reposition it very often ends with the pointer over the hide button; routing
 * that to "hide" would make the window vanish when the user only meant to move
 * it — and `skipTaskbar: true` means the tray menu is the only way back. A
 * gesture that was a drag is never also a click.
 */
export function islandIntent(target: EventTarget | null, dragged: boolean): IslandIntent {
  if (dragged) return "none";
  const el = target instanceof Element ? target : null;
  return el?.closest(HIDE_SEL) ? "hide" : "expand";
}

/**
 * Hide-to-tray affordance, present in every island state — what blocks the
 * user's view is the island itself, so requiring them to expand the panel first
 * would defeat the purpose.
 *
 * Deliberately a minimise bar, not an ✕: the tray menu offers both "Show /
 * Hide" and "Quit TokenBar", and an ✕ on a window whose only route back is that
 * same menu would read as the latter.
 */
function hideBtn(): string {
  return `<button class="ihide" data-hide type="button" aria-label="隱藏到系統匣" title="隱藏到系統匣（可從系統匣圖示叫回）"><svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><rect x="1" y="4.4" width="8" height="1.2" rx="0.6" fill="currentColor"/></svg></button>`;
}

/** Small battery/fuel capsule; inner fill = % left (remaining). */
function capsuleSvg(left: number): string {
  const w = (Math.max(0, Math.min(100, left)) / 100) * 13;
  return `<svg class="cap" width="21" height="12" viewBox="0 0 21 12" aria-hidden="true">
    <rect x="0.75" y="0.75" width="17" height="10.5" rx="3" fill="none" stroke="currentColor" stroke-width="1.5" opacity="0.85"/>
    <rect x="19" y="3.75" width="1.6" height="4.5" rx="0.8" fill="currentColor" opacity="0.85"/>
    <rect x="2.75" y="2.75" width="${w.toFixed(1)}" height="6.5" rx="1.5" fill="currentColor"/>
  </svg>`;
}

/** Most dangerous limit of one provider: locked > near > highest util. */
function worstOf(limits: Limit[], provider: "anthropic" | "codex"): Limit | null {
  const rank = (l: Limit) => (l.status === "locked" ? 2 : l.status === "near" ? 1 : 0);
  return limits
    .filter((l) => l.provider === provider)
    .reduce<Limit | null>(
      (best, l) =>
        !best || rank(l) > rank(best) || (rank(l) === rank(best) && l.util > best.util)
          ? l
          : best,
      null,
    );
}

/** One capsule group: provider brand icon + capsule + % left, colored by status. */
function group(l: Limit | null, provider: "anthropic" | "codex"): string {
  const icon = providerIcon(provider, 13);
  const cls = provider === "anthropic" ? "prov-claude" : "prov-codex";
  if (!l) {
    return `<span class="igroup status-empty ${cls}">${icon}${capsuleSvg(0)}<span class="pct">—</span></span>`;
  }
  const left = pctLeft(l.util);
  return `<span class="igroup status-${l.status} ${cls}">${icon}${capsuleSvg(left)}<span class="pct">${left}%</span></span>`;
}

export function renderIsland(root: HTMLElement, snap: Snapshot | null, opts: IslandOpts): void {
  const limits = snap?.limits ?? [];
  const worst =
    snap && snap.worst_id ? limits.find((l) => l.id === snap.worst_id) ?? null : null;

  if (!snap || limits.length === 0) {
    root.className = "island status-empty";
    root.innerHTML = `${capsuleSvg(0)}<span class="pct">—</span>${hideBtn()}`;
    return;
  }

  // Root keeps the overall-worst status: border glow / locked blink apply whole-pill.
  root.className = `island island-${opts.mode} status-${worst?.status ?? "empty"}`;

  let body = "";
  if (opts.mode === "claude" || opts.mode === "codex") {
    const p = opts.mode === "claude" ? "anthropic" : "codex";
    body = group(worstOf(limits, p), p);
  } else {
    // "both" (and any legacy/unknown value): providers side-by-side
    body =
      group(worstOf(limits, "anthropic"), "anthropic") +
      group(worstOf(limits, "codex"), "codex");
  }

  // Aux: today's burn rate.
  const auxHtml =
    opts.tokPerMin != null
      ? `<span class="iaux">${fmtTokens(opts.tokPerMin)}/min</span>`
      : "";

  root.innerHTML = body + auxHtml + hideBtn();
}
