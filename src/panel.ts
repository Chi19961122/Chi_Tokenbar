// Expanded panel — grouped "Limits" list with battery gauges, plus a
// per-limit detail view (drill-down), matching the v8 visual.

import type { Limit, Provider, Snapshot } from "./types";
import { fmtDur, fmtHM, fmtReset, fmtTokens, nowSecs, pctLeft } from "./format";
import { providerIcon } from "./icons";
import { t } from "./i18n";

export type PanelView = { kind: "list" } | { kind: "detail"; id: string };

/** Re-login button lifecycle. Lives in main.ts's `ui` rather than the DOM
 *  because the 1s countdown tick re-renders this whole subtree. */
export type ReloginState = "idle" | "launching" | "ok" | "failed";
export type PanelOpts = { relogin?: ReloginState; copied?: boolean };

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

/**
 * The row's note line — always carries the reset, so the user reads it without
 * drilling into the detail view. Content is fixed copy plus formatted
 * numbers/times (no backend free-text), so it needs no escaping.
 *
 *   safe + pace   → "On pace · resets 14:00"
 *   over pace     → "12% over pace · resets Thu 09:00"
 *   locked        → "Locked · resets in 3h 12m"   (countdown; window releasing)
 *   no pace       → "Resets 16:30"
 *   no reset info → whatever pace/status word we have, or "" (e.g. source_failed)
 */
function rowNote(l: Limit): string {
  if (l.status === "locked") {
    return l.resets_at > 0
      ? t("note.lockedResetsIn", { d: fmtDur(l.resets_at - nowSecs()) })
      : t("note.locked");
  }
  const pace = l.pace
    ? l.pace.in_deficit
      ? t("note.overPace", { n: Math.round(l.pace.deficit) })
      : t("note.onPace")
    : "";
  if (l.resets_at > 0) {
    const reset = fmtReset(l.resets_at);
    return pace ? t("note.pacedResets", { pace, r: reset }) : t("note.resets", { r: reset });
  }
  return pace;
}

function row(l: Limit): string {
  const unknown = isUnknown(l);
  const left = pctLeft(l.util);
  const pct = unknown ? "—" : `${left}%`;
  // source_failed is not an estimate (see the detail view) — say so in the list too.
  const badge = unknown
    ? `<span class="badge">${l.status === "source_failed" ? t("badge.unavailable") : t("badge.estimate")}</span>`
    : "";
  const note = rowNote(l);
  return `<button class="lrow status-${l.status} ${provClass(l)}" data-limit="${escapeHtml(l.id)}">
    ${battery(unknown ? 0 : left)}
    <span class="lrow-mid">
      <span class="lrow-label">${escapeHtml(displayName(l))}${badge}</span>
      ${note ? `<span class="lrow-note">${note}</span>` : ""}
    </span>
    <span class="lrow-pct">${pct}</span>
  </button>`;
}

function list(limits: Limit[]): string {
  const groups = PROVIDER_ORDER.map((p) => {
    const items = limits.filter((l) => l.provider === p);
    if (items.length === 0) return "";
    const meta = PROVIDER_META[p];
    return `<div class="lsec">
      <div class="lsec-head ${meta.cls}">${providerIcon(p, 12)}${meta.name}</div>
      ${items.map(row).join("")}
    </div>`;
  }).join("");
  return groups || `<div class="empty-note">${t("list.noTools")}</div>`;
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

function detail(l: Limit, opts: PanelOpts): string {
  const unknown = isUnknown(l);
  const left = pctLeft(l.util);

  // Status line: LOCKED / Unavailable / pace + runway.
  let sub = "";
  if (l.status === "locked") {
    const reset = l.resets_at > 0 ? ` ${t("detail.resetsIn", { d: fmtDur(l.resets_at - nowSecs()) })}` : "";
    sub = `<span class="lock">${t("detail.locked")}</span>${reset}`;
  } else if (l.status === "source_failed") {
    // No "Estimate" badge: nothing is estimated — the backend sends 0% placeholders.
    // Show the real reason instead of implying the 0% is a computed estimate.
    // The fallback stays provider-neutral: Codex's live degradation carries no
    // hint, and naming Claude there would just be a different lie.
    sub = `<span class="badge">${t("badge.unavailable")}</span> ${escapeHtml(l.hint ?? t("detail.unavailableFallback"))}`;
  } else if (l.status === "stale") {
    sub = `<span class="badge">${t("badge.stale")}</span> ${t("detail.staleNote")}`;
  } else if (l.status === "idle") {
    sub = t("detail.idle");
  } else {
    const parts: string[] = [];
    if (l.pace) {
      parts.push(
        l.pace.in_deficit
          ? `<span class="deficit">${t("note.overPace", { n: Math.round(l.pace.deficit) })}</span>`
          : `<span class="onpace">${t("note.onPace")}</span>`,
      );
    }
    if (l.runway_secs != null) parts.push(t("detail.left", { d: fmtDur(l.runway_secs) }));
    sub = parts.join(" · ");
  }

  // Gated on the backend's decision, never on what `hint` happens to say.
  const action =
    l.status === "source_failed" && l.action === "relogin"
      ? reloginBlock(opts.relogin ?? "idle", opts.copied ?? false)
      : "";

  const absLine = l.absolute
    ? `<div class="detail-abs">${t("detail.tokens", { a: fmtTokens(l.absolute[0]), b: fmtTokens(l.absolute[1]) })}</div>`
    : "";
  const reset =
    l.resets_at > 0 && l.status !== "locked"
      ? t("detail.resetsAt", { t: fmtHM(l.resets_at), d: fmtDur(l.resets_at - nowSecs()) })
      : "";

  return `<div class="detail status-${l.status} ${provClass(l)}">
    <div class="detail-head">
      <button class="back" data-back title="${t("detail.back")}">‹</button>
      <span class="detail-title">${escapeHtml(displayName(l))}</span>
    </div>
    <div class="dcard">
      <div class="detail-pct">${unknown ? "—" : `${left}%`}<small>${t("detail.leftLabel")}</small></div>
      <div class="dscale"><span>0</span><span>25</span><span>50</span><span>75</span><span>100</span></div>
      <div class="dmeter">
        <div class="dmeter-fill" style="width:${unknown ? 0 : left}%"></div>
        <div class="dtick" style="left:25%"></div>
        <div class="dtick" style="left:50%"></div>
        <div class="dtick" style="left:75%"></div>
      </div>
      ${sub ? `<div class="detail-sub">${sub}</div>` : ""}
      ${action}
      ${absLine}
      ${reset ? `<div class="detail-reset">${reset}</div>` : ""}
    </div>
  </div>`;
}

export function renderPanel(
  container: HTMLElement,
  snap: Snapshot | null,
  view: PanelView,
  opts: PanelOpts = {},
): void {
  const limits = snap?.limits ?? [];
  if (view.kind === "detail") {
    const l = limits.find((x) => x.id === view.id);
    if (l) {
      container.innerHTML = detail(l, opts);
      return;
    }
    // limit vanished (tool stopped) — fall through to the list
  }
  container.innerHTML = list(limits);
}
