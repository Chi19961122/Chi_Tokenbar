// Expanded panel — grouped "Limits" list with battery gauges. 階段 B removed the
// per-limit detail drill-down: the list row now carries everything (reset time,
// est./unavailable badges, and the re-login affordance for a login-class
// failure), so there is no second screen to route to.

import type { Limit, Provider, ResetDisplay, Snapshot } from "./types";
import { fmtResetClock, fmtResetRel, pctLeft } from "./format";
import type { Locale } from "./i18n";
import { providerIcon } from "./icons";
import { windowShort } from "./island";
import { t } from "./i18n";

/** Re-login button lifecycle. Lives in main.ts's `ui` rather than the DOM
 *  because the 1s countdown tick re-renders this whole subtree. */
export type ReloginState = "idle" | "launching" | "ok" | "failed";
export type PanelOpts = {
  relogin?: ReloginState;
  copied?: boolean;
  /** Reset-time rendering (settings.reset_display); must match the island. */
  resetDisplay: ResetDisplay;
  /** Epoch seconds "now", so the countdown ticks and tests stay deterministic. */
  now: number;
  /** Active UI locale (clock format follows it). */
  locale: Locale;
  /**
   * 階段 C: how the limits render.
   *   "full"    → the grouped battery list (Limits tab, or settings open).
   *   "summary" → a single-line quota digest that expands to the full list on
   *               click, so the Usage tab leads with charts, not half a screen
   *               of quota. `summaryExpanded` holds the (session-only) toggle.
   */
  variant?: "full" | "summary";
  summaryExpanded?: boolean;
};

/** The exact command shown (and copied) when we can't start it ourselves. */
export const MANUAL_LOGIN_CMD = "claude auth login";

const PROVIDER_META: Record<Provider, { name: string; cls: string }> = {
  anthropic: { name: "Claude Code", cls: "prov-claude" },
  codex: { name: "Codex", cls: "prov-codex" },
  grok: { name: "Grok", cls: "prov-grok" },
};
const provClass = (l: Limit) => PROVIDER_META[l.provider].cls;
// Grok trails the two subscription-quota providers: its context-fill limit is a
// different kind of reading, so it reads last in the list and the digest.
const PROVIDER_ORDER: Provider[] = ["anthropic", "codex", "grok"];

/** Display-name i18n keys per limit id (provider context comes from the group). */
const LIMIT_NAME_KEYS = {
  "cc.5h": "limit.cc5h",
  "cc.week": "limit.ccWeek",
  "cc.opus": "limit.ccOpus",
  "cc.extra": "limit.ccExtra",
  "codex.5h": "limit.codex5h",
  "codex.week": "limit.codexWeek",
  "codex.credits": "limit.codexCredits",
  "grok.ctx": "limit.grokCtx",
} as const;
const displayName = (l: Limit) => {
  const key = LIMIT_NAME_KEYS[l.id as keyof typeof LIMIT_NAME_KEYS];
  if (key) return t(key);
  // Model-scoped weekly windows from the limits array (cc.w.<slug>), e.g. Fable.
  if (l.id.startsWith("cc.w."))
    return t("limit.weeklyModel", { name: (l.label.split("·")[1] ?? l.label).trim() });
  return l.label;
};

const isUnknown = (l: Limit) =>
  l.status === "source_failed" || l.status === "insufficient_data";

/**
 * Escape before interpolating backend strings into innerHTML.
 * `hint` and `label` are variable-length values that originate outside this
 * file (label is even derived from an API response), so they can't be trusted
 * the way the hard-coded copy around them can.
 */
function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

type GaugeState = "safe" | "warn" | "locked" | "stale" | "degraded";

const gaugeState = (l: Limit): GaugeState => {
  if (l.status === "locked") return "locked";
  if (l.status === "near") return "warn";
  if (isUnknown(l)) return "degraded";
  if (l.status === "stale" || l.status === "idle") return "stale";
  return "safe";
};

const GAUGE_STATE_PRIORITY: GaugeState[] = ["locked", "warn", "degraded", "stale", "safe"];
const GAUGE_STATE_LABEL: Record<GaugeState, string> = {
  safe: "healthy",
  warn: "near limit",
  locked: "locked",
  stale: "stale",
  degraded: "degraded",
};

function worstGaugeState(limits: Limit[]): GaugeState {
  return GAUGE_STATE_PRIORITY.find((state) => limits.some((l) => gaugeState(l) === state)) ?? "safe";
}

/** The reset instant as either a countdown or a clock, following reset_display —
 *  kept identical to the island so both surfaces read the same. */
function resetValue(l: Limit, opts: PanelOpts): string {
  return opts.resetDisplay === "clock"
    ? fmtResetClock(l.resets_at, opts.now, opts.locale)
    : fmtResetRel(l.resets_at, opts.now);
}

/**
 * The row's note line — reset time only (階段 B dropped the pace copy). Content
 * is fixed copy plus formatted times, except the source_failed hint, which is
 * backend free-text and is escaped.
 *
 *   locked        → "Locked · resets in 3h 12m"  / "Locked · resets 14:00"
 *   has reset     → "Resets in 16:30" (relative)  / "Resets 16:30" (clock)
 *   source_failed → the backend hint (why it's unavailable)
 *   otherwise     → ""
 */
function rowNote(l: Limit, opts: PanelOpts): string {
  if (l.status === "source_failed") return l.hint ? escapeHtml(l.hint) : "";
  // Grok's context fill has no reset schedule — it empties on a new session. Show
  // that honestly instead of a reset countdown, so the semantic difference from a
  // subscription quota is visible (T-917). Fixed copy, no interpolation.
  if (l.provider === "grok") return t("note.grokSession");
  if (l.status === "locked") {
    if (l.resets_at <= 0) return t("note.locked");
    return opts.resetDisplay === "clock"
      ? t("note.lockedResets", { r: resetValue(l, opts) })
      : t("note.lockedResetsIn", { d: resetValue(l, opts) });
  }
  if (l.resets_at > 0) {
    return opts.resetDisplay === "clock"
      ? t("note.resets", { r: resetValue(l, opts) })
      : t("note.resetsIn", { d: resetValue(l, opts) });
  }
  return "";
}

/**
 * The re-login affordance, shown only when the backend said this failure is
 * one that logging in again actually fixes (`l.action === "relogin"`).
 *
 * The "failed" branch is the point of the whole thing: `claude` frequently
 * isn't on TokenBar's PATH (it inherits Explorer's/autostart's environment,
 * and Claude Code may live in WSL entirely). A dead-end error would leave the
 * user stuck, so we show the command itself, copyable.
 */
function reloginBlock(state: ReloginState, copied: boolean): string {
  if (state === "failed") {
    return `<div class="relogin-manual">
      <div>${t("relogin.cantLaunch")}</div>
      <div class="relogin-cmd">
        <code>${escapeHtml(MANUAL_LOGIN_CMD)}</code>
        <button class="relogin-copy" data-relogin-copy>${copied ? t("relogin.copied") : t("relogin.copy")}</button>
      </div>
    </div>`;
  }
  if (state === "ok") {
    return `<div class="relogin-note">${t("relogin.ok")}</div>`;
  }
  const busy = state === "launching";
  return `<button class="relogin" data-relogin ${busy ? "disabled" : ""}>${
    busy ? t("relogin.opening") : t("relogin.button")
  }</button>`;
}

function row(l: Limit, opts: PanelOpts): string {
  const unknown = isUnknown(l);
  const left = pctLeft(l.util);
  const state = gaugeState(l);
  const pct = unknown ? "—" : `${left}`;
  // source_failed is not an estimate (the backend sends 0% placeholders) — say
  // "Unavailable"; insufficient_data is a real estimate; stale flags "从上次".
  const badge = unknown
    ? `<span class="badge">${l.status === "source_failed" ? t("badge.unavailable") : t("badge.estimate")}</span>`
    : l.status === "stale"
      ? `<span class="badge">${t("badge.stale")}</span>`
      : "";
  const note = rowNote(l, opts);
  // The re-login affordance now lives inline in the list (there is no detail
  // view to host it). Gated on the backend's decision, never on hint text.
  const action =
    l.status === "source_failed" && l.action === "relogin"
      ? `<div class="lrow-action">${reloginBlock(opts.relogin ?? "idle", opts.copied ?? false)}</div>`
      : "";
  // Badges only — the hero digits directly above already say "N% left", and
  // repeating the same value in small type read as clutter (二次驗收).
  const detail = badge;
  return `<div class="gauge-row gauge-state-${state} status-${l.status} ${provClass(l)}">
    <div class="gauge-kicker">${escapeHtml(displayName(l))}</div>
    <div class="gauge-hero">
      <span class="gauge-value">${pct}</span>
      ${unknown ? "" : `<span class="gauge-unit">%</span>`}
      <span class="gauge-left">${t("share.left")}</span>
    </div>
    <div class="gauge-track" aria-hidden="true"><span class="gauge-fill" style="width:${unknown ? 0 : left}%"></span></div>
    <div class="gauge-meta">
      <span class="gauge-detail">${detail}</span>
      ${note ? `<span class="gauge-reset">${note}</span>` : ""}
    </div>
    ${action}
  </div>`;
}

function list(limits: Limit[], opts: PanelOpts): string {
  const groups = PROVIDER_ORDER.map((p) => {
    const items = limits.filter((l) => l.provider === p);
    if (items.length === 0) return "";
    const meta = PROVIDER_META[p];
    const state = worstGaugeState(items);
    return `<div class="lsec gauge-card ${meta.cls}">
      <div class="gauge-card-head">
        <span class="gauge-provider">${providerIcon(p, 14)}<span class="gauge-provider-name">${meta.name}</span></span>
        <span class="gauge-card-status gauge-state-${state}"><span class="gauge-status-dot"></span>${GAUGE_STATE_LABEL[state]}</span>
      </div>
      <div class="gauge-grid">${items.map((l) => row(l, opts)).join("")}</div>
    </div>`;
  }).join("");
  return groups || `<div class="empty-note">${t("list.noTools")}</div>`;
}

// ── Usage-tab quota summary (階段 C) ──────────────────────────────────

/** Fixed-English provider labels for the summary line (mirrors the island's
 *  short labels — never localized, D1). */
const SUMMARY_NAME: Record<Provider, string> = { anthropic: "Claude", codex: "Codex", grok: "Grok" };

export interface QuotaSummarySeg {
  /** Fixed-English window short label ("5h", "wk", model name). */
  short: string;
  /** "62%" left, or "—" when the reading is unavailable. */
  pct: string;
}
export interface QuotaSummaryGroup {
  provider: Provider;
  name: string;
  segs: QuotaSummarySeg[];
}

/**
 * Condense the limits into a per-provider digest for the one-line summary, e.g.
 * Claude → [5h 62%, wk 18%], Codex → [wk 0%]. Pure so the digest (which windows,
 * what %, unavailable → "—") is unit-testable. Data comes straight from the
 * snapshot — no extra backend call.
 */
export function buildQuotaSummary(limits: Limit[]): QuotaSummaryGroup[] {
  return PROVIDER_ORDER.flatMap((p) => {
    const items = limits.filter((l) => l.provider === p);
    if (items.length === 0) return [];
    const segs = items.map((l) => ({
      short: windowShort(l) || l.id,
      pct: isUnknown(l) ? "—" : `${pctLeft(l.util)}%`,
    }));
    return [{ provider: p, name: SUMMARY_NAME[p], segs }];
  });
}

/** The collapsed summary row: a full-width toggle showing every provider's
 *  quota on one line; clicking it (main.ts) expands the full list beneath. */
function summaryBar(limits: Limit[], expanded: boolean): string {
  const groups = buildQuotaSummary(limits);
  const inner = groups.length
    ? groups
        .map(
          (g) =>
            `<span class="qs-group ${PROVIDER_META[g.provider].cls}">` +
            `<span class="qs-dot"></span><span class="qs-name">${g.name}</span>` +
            g.segs
              .map((s) => `<span class="qs-seg">${escapeHtml(s.short)} ${s.pct}</span>`)
              .join(`<span class="qs-mid">·</span>`) +
            `</span>`,
        )
        .join("")
    : `<span class="qs-empty">${t("list.noTools")}</span>`;
  return `<button class="quota-summary${expanded ? " on" : ""}" data-quota-toggle type="button" aria-expanded="${expanded}">
    <span class="qs-line">${inner}</span>
    <span class="qs-chev">${expanded ? "▾" : "▸"}</span>
  </button>`;
}

// No section head here: the header tabs directly above are the page title, and
// repeating it ("01 LIMITS" under 限額) was pure duplication (三次驗收).
export function renderPanel(container: HTMLElement, snap: Snapshot | null, opts: PanelOpts): void {
  const limits = snap?.limits ?? [];
  if (opts.variant === "summary") {
    const expanded = opts.summaryExpanded ?? false;
    container.innerHTML = summaryBar(limits, expanded) + (expanded ? list(limits, opts) : "");
    return;
  }
  container.innerHTML = list(limits, opts);
}
