// Island (collapsed) view — fuel capsule(s) + % left, per the Live Island design.
// Layout follows the global display filter (settings.providers): both providers
// side-by-side (default), or a single provider. The backend already filtered the
// snapshot, so `mode` only decides the layout, never what data exists.
//
// 階段 B: which limit each provider shows is a *pin* (auto / 5h / week / a model
// window); Near and Locked carry a fixed-English short label plus the reset time
// (countdown or clock, per reset_display); the right side is an optional aux
// readout (tok/min or today's cost). The two interesting decisions — which limit
// to show (pickIslandLimit) and what text it produces (islandText) — are pure
// functions so the display matrix is unit-testable.

import type { IslandAux, Limit, ProviderFilter, ResetDisplay, Snapshot } from "./types";
import { fmtResetClock, fmtResetRel, fmtTokens, fmtUsd, pctLeft } from "./format";
import type { Locale } from "./i18n";
import { providerIcon } from "./icons";
import { t } from "./i18n";

export interface IslandOpts {
  mode: ProviderFilter;
  /** Per-provider quota pin ("auto" | "5h" | "week" | "model:<id>"). */
  pinClaude: string;
  pinCodex: string;
  /** Reset-time rendering: countdown vs absolute clock. */
  resetDisplay: ResetDisplay;
  /** Right-side aux readout selector. */
  aux: IslandAux;
  /** Today's average tokens/minute (for aux "tok_per_min"). */
  tokPerMin: number | null;
  /** Today's est. cost in USD (for aux "cost_today"). */
  costToday: number | null;
  /** Epoch seconds "now", so relative/clock resets and tests are deterministic. */
  now: number;
  /** Active UI locale (clock format follows it; short labels stay English). */
  locale: Locale;
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
  return `<button class="ihide" data-hide type="button" aria-label="${t("island.hideAria")}" title="${t("island.hideTitle")}"><svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><rect x="1" y="4.4" width="8" height="1.2" rx="0.6" fill="currentColor"/></svg></button>`;
}

/** Battery capsule: white frame + electrode nub; inner fill = % left, in the
 *  current color (family / amber / red set by the enclosing .igroup). */
function capsuleSvg(left: number): string {
  const w = (Math.max(0, Math.min(100, left)) / 100) * 16;
  return `<svg class="cap" width="24" height="12" viewBox="0 0 24 12" aria-hidden="true">
    <rect x="0.75" y="1" width="19.5" height="10" rx="2.5" fill="none" stroke="rgba(255,255,255,0.35)" stroke-width="1.5"/>
    <rect x="21" y="3.8" width="1.8" height="4.4" rx="0.9" fill="rgba(255,255,255,0.35)"/>
    <rect x="2.5" y="2.75" width="${w.toFixed(1)}" height="6.5" rx="1.5" fill="currentColor"/>
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

/** Canonical 5h / weekly limit ids per provider. */
const WINDOW_ID = {
  anthropic: { "5h": "cc.5h", week: "cc.week" },
  codex: { "5h": "codex.5h", week: "codex.week" },
} as const;

/**
 * Which limit the island shows for `provider`, honouring the pin:
 *   "auto"        → worst-ranked (the pre-階段-B behaviour)
 *   "5h" / "week" → that window
 *   "model:<id>"  → that exact limit id
 *
 * A pin that names data we don't have — a window/id absent from this snapshot —
 * returns null (the caller renders "—"). It deliberately does **not** fall back
 * to auto: the user pinned a specific view and silently showing a different
 * number would be worse than an honest blank.
 */
export function pickIslandLimit(
  limits: Limit[],
  provider: "anthropic" | "codex",
  pin: string,
): Limit | null {
  if (!pin || pin === "auto") return worstOf(limits, provider);
  const pool = limits.filter((l) => l.provider === provider);
  if (pin === "5h" || pin === "week") {
    const id = WINDOW_ID[provider][pin];
    return pool.find((l) => l.id === id) ?? null;
  }
  if (pin.startsWith("model:")) {
    const id = pin.slice("model:".length);
    return pool.find((l) => l.id === id) ?? null;
  }
  return null; // unknown pin → honest blank, never a silent auto
}

/** Fixed-English short label for a window/model limit (never localized, D1). */
export function windowShort(l: Limit): string {
  if (l.id.endsWith(".5h")) return "5h";
  if (l.id.endsWith(".week")) return "wk";
  if (l.id === "cc.opus") return "Opus";
  if (l.id === "cc.extra") return "Extra";
  if (l.id === "codex.credits") return "Cr";
  if (l.id.startsWith("cc.w.")) {
    const slug = l.id.slice("cc.w.".length);
    return slug.charAt(0).toUpperCase() + slug.slice(1);
  }
  // Fall back to the tail of the label after "·" (e.g. "Claude·Opus" → "Opus").
  const tail = l.label.split("·").pop()?.trim();
  return tail || "";
}

/** True for statuses whose util is a placeholder, not a real reading. */
const isUnknown = (l: Limit) =>
  l.status === "source_failed" || l.status === "insufficient_data";

function resetFrag(l: Limit, resetDisplay: ResetDisplay, now: number, locale: Locale): string {
  if (l.resets_at <= 0) return "—";
  return resetDisplay === "clock"
    ? fmtResetClock(l.resets_at, now, locale)
    : fmtResetRel(l.resets_at, now);
}

/**
 * The text a single provider group shows next to its capsule (階段 B matrix):
 *   normal        → "{left}%"
 *   near          → "{short} {left}% · {reset}"   (short = 5h / wk / model name)
 *   locked        → "0% · {reset}"
 *   estimate/idle → "{left}%" or "—", with an " est." tag when the number is
 *                   inferred (stale / insufficient_data)
 *
 * Pure and locale-parameterised so the whole matrix is unit-testable; the reset
 * clock follows `locale`, the short labels never do.
 */
export function islandText(
  l: Limit,
  resetDisplay: ResetDisplay,
  now: number,
  locale: Locale,
): string {
  if (l.status === "locked") {
    return `0% · ${resetFrag(l, resetDisplay, now, locale)}`;
  }
  if (l.status === "near") {
    const short = windowShort(l);
    return `${short ? short + " " : ""}${pctLeft(l.util)}% · ${resetFrag(l, resetDisplay, now, locale)}`;
  }
  const est = l.status === "stale" || l.status === "insufficient_data" ? " est." : "";
  const pct = isUnknown(l) ? "—" : `${pctLeft(l.util)}%`;
  return `${pct}${est}`;
}

/** One capsule group: provider brand icon + capsule + islandText, colored by status. */
function group(
  l: Limit | null,
  provider: "anthropic" | "codex",
  opts: IslandOpts,
): string {
  const icon = providerIcon(provider, 13);
  const cls = provider === "anthropic" ? "prov-claude" : "prov-codex";
  if (!l) {
    return `<span class="igroup status-empty ${cls}">${icon}${capsuleSvg(0)}<span class="pct">—</span></span>`;
  }
  const left = isUnknown(l) ? 0 : pctLeft(l.util);
  const text = islandText(l, opts.resetDisplay, opts.now, opts.locale);
  return `<span class="igroup status-${l.status} ${cls}">${icon}${capsuleSvg(left)}<span class="pct">${text}</span></span>`;
}

/** Whole-pill status (border glow / locked blink): worst of the shown limits. */
function pillStatus(shown: (Limit | null)[]): string {
  const rank = (s: string) => (s === "locked" ? 3 : s === "near" ? 2 : 1);
  let worst: Limit | null = null;
  for (const l of shown) {
    if (!l) continue;
    if (!worst || rank(l.status) > rank(worst.status)) worst = l;
  }
  return worst?.status ?? "empty";
}

function auxHtml(opts: IslandOpts): string {
  if (opts.aux === "tok_per_min" && opts.tokPerMin != null) {
    return `<span class="iaux">${fmtTokens(opts.tokPerMin)}/min</span>`;
  }
  if (opts.aux === "cost_today" && opts.costToday != null) {
    return `<span class="iaux">${fmtUsd(opts.costToday)}</span>`;
  }
  return ""; // "off", or a null value we won't fake a number for
}

export function renderIsland(root: HTMLElement, snap: Snapshot | null, opts: IslandOpts): void {
  const limits = snap?.limits ?? [];

  if (!snap || limits.length === 0) {
    root.className = "island status-empty";
    root.innerHTML = `${capsuleSvg(0)}<span class="pct">—</span>${hideBtn()}`;
    return;
  }

  let shown: (Limit | null)[];
  let body: string;
  if (opts.mode === "claude") {
    const l = pickIslandLimit(limits, "anthropic", opts.pinClaude);
    shown = [l];
    body = group(l, "anthropic", opts);
  } else if (opts.mode === "codex") {
    const l = pickIslandLimit(limits, "codex", opts.pinCodex);
    shown = [l];
    body = group(l, "codex", opts);
  } else {
    // "both" (and any legacy/unknown value): providers side-by-side
    const c = pickIslandLimit(limits, "anthropic", opts.pinClaude);
    const x = pickIslandLimit(limits, "codex", opts.pinCodex);
    shown = [c, x];
    body = group(c, "anthropic", opts) + group(x, "codex", opts);
  }

  // Root keeps the shown limits' worst status: border glow / locked blink apply whole-pill.
  root.className = `island island-${opts.mode} status-${pillStatus(shown)}`;
  root.innerHTML = body + auxHtml(opts) + hideBtn();
}
