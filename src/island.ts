// Island (collapsed) view — fuel capsule(s) + % left, per the Live Island design.
// Layout is configurable (settings.island_mode): both providers side-by-side
// (default), a single provider, or the classic single most-dangerous limit.
// Right side: today's burn rate (tok/min).

import type { IslandMode, Limit, Snapshot } from "./types";
import { fmtTokens, pctLeft } from "./format";
import { providerIcon } from "./icons";

export interface IslandOpts {
  mode: IslandMode;
  /** Today's average tokens/minute, for the right-side readout. */
  tokPerMin: number | null;
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
    root.innerHTML = `${capsuleSvg(0)}<span class="pct">—</span>`;
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

  root.innerHTML = body + auxHtml;
}
