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
};
const provClass = (l: Limit) => PROVIDER_META[l.provider].cls;
const PROVIDER_ORDER: Provider[] = ["anthropic", "codex"];

/** Display-name i18n keys per limit id (provider context comes from the group). */
const LIMIT_NAME_KEYS = {
  "cc.5h": "limit.cc5h",
  "cc.week": "limit.ccWeek",
  "cc.opus": "limit.ccOpus",
  "cc.extra": "limit.ccExtra",
  "codex.5h": "limit.codex5h",
  "codex.week": "limit.codexWeek",
  "codex.credits": "limit.codexCredits",
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

/** Battery gauge (46×22): white frame + electrode nub; inner fill = % left,
 *  drawn in the row's current color and framed by the row's --frame var. */
function battery(left: number): string {
  const w = (Math.min(100, Math.max(0, left)) / 100) * 34;
  return `<svg class="pcap" width="46" height="22" viewBox="0 0 46 22" aria-hidden="true">
    <rect x="1" y="1" width="40" height="20" rx="6" fill="none" stroke="var(--frame)" stroke-width="2"/>
    <rect x="42.5" y="7" width="2.5" height="8" rx="1.25" fill="var(--frame)"/>
    <rect x="4" y="4" width="${w.toFixed(1)}" height="14" rx="3" fill="currentColor"/>
  </svg>`;
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
  const pct = unknown ? "—" : `${left}%`;
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
  return `<div class="lrow status-${l.status} ${provClass(l)}">
    ${battery(unknown ? 0 : left)}
    <span class="lrow-mid">
      <span class="lrow-label">${escapeHtml(displayName(l))}${badge}</span>
      ${note ? `<span class="lrow-note">${note}</span>` : ""}
    </span>
    <span class="lrow-pct">${pct}</span>
  </div>${action}`;
}

function list(limits: Limit[], opts: PanelOpts): string {
  const groups = PROVIDER_ORDER.map((p) => {
    const items = limits.filter((l) => l.provider === p);
    if (items.length === 0) return "";
    const meta = PROVIDER_META[p];
    return `<div class="lsec">
      <div class="lsec-head ${meta.cls}">${providerIcon(p, 12)}${meta.name}</div>
      ${items.map((l) => row(l, opts)).join("")}
    </div>`;
  }).join("");
  return groups || `<div class="empty-note">${t("list.noTools")}</div>`;
}

function shellStatus(limits: Limit[]): "safe" | "warn" | "locked" | "stale" {
  if (limits.some((l) => l.status === "locked")) return "locked";
  if (limits.some((l) => l.status === "near")) return "warn";
  if (limits.some((l) => isUnknown(l) || l.status === "stale" || l.status === "idle")) return "stale";
  return "safe";
}

function sectionHeader(number: "01" | "02", title: "limits" | "usage"): string {
  return `<div class="section-head">
    <span class="section-number">${number}</span>
    <span class="section-title">${t(`section.${title}`)}</span>
    <span class="section-editorial">${t(`section.${title}Editorial`)}</span>
  </div>`;
}

function statusPill(limits: Limit[]): string {
  const state = shellStatus(limits);
  const known = limits.filter((l) => !isUnknown(l));
  const left = known.length ? Math.min(...known.map((l) => pctLeft(l.util))) : null;
  return `<div class="status-row">
    <span class="app-mark" aria-hidden="true">ϟ</span>
    <span class="status-pill status-pill-${state}"><span class="status-ping"></span>${left === null ? "—" : `${left}% left`}</span>
  </div>`;
}

// ── Usage-tab quota summary (階段 C) ──────────────────────────────────

/** Fixed-English provider labels for the summary line (mirrors the island's
 *  short labels — never localized, D1). */
const SUMMARY_NAME: Record<Provider, string> = { anthropic: "Claude", codex: "Codex" };

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

export function renderPanel(container: HTMLElement, snap: Snapshot | null, opts: PanelOpts): void {
  const limits = snap?.limits ?? [];
  if (opts.variant === "summary") {
    const expanded = opts.summaryExpanded ?? false;
    container.innerHTML = statusPill(limits) + sectionHeader("02", "usage") +
      summaryBar(limits, expanded) + (expanded ? list(limits, opts) : "");
    return;
  }
  container.innerHTML = statusPill(limits) + sectionHeader("01", "limits") + list(limits, opts);
}
