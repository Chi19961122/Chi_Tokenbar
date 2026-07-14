// Expanded panel — grouped "Limits" list with progress rings, plus a
// per-limit detail view (drill-down), matching the Live Island design.

import type { Limit, Provider, Snapshot } from "./types";
import { fmtClock, fmtDur, fmtTokens, nowSecs, pctLeft } from "./format";
import { providerIcon } from "./icons";

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
const PROVIDER_ORDER: Provider[] = ["anthropic", "codex"];

/** Display names per the prototype (provider context comes from the group). */
const LIMIT_NAMES: Record<string, string> = {
  "cc.5h": "5h Session",
  "cc.week": "Weekly · all models",
  "cc.opus": "Weekly · Opus",
  "cc.extra": "Extra usage",
  "codex.5h": "5h window",
  "codex.week": "Weekly window",
  "codex.credits": "Credits",
};
const displayName = (l: Limit) => {
  if (LIMIT_NAMES[l.id]) return LIMIT_NAMES[l.id];
  // Model-scoped weekly windows from the limits array (cc.w.<slug>), e.g. Fable.
  if (l.id.startsWith("cc.w.")) return `Weekly · ${l.label.split("·")[1] ?? l.label}`;
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

/** Circular progress ring; arc = consumed (util%). */
function ring(l: Limit): string {
  const r = 8;
  const c = 2 * Math.PI * r;
  const used = Math.min(100, Math.max(0, l.util)) / 100;
  return `<svg class="ring" width="22" height="22" viewBox="0 0 22 22" aria-hidden="true">
    <circle cx="11" cy="11" r="${r}" fill="none" stroke="var(--track)" stroke-width="3"/>
    <circle cx="11" cy="11" r="${r}" fill="none" stroke="currentColor" stroke-width="3"
      stroke-linecap="round" stroke-dasharray="${(used * c).toFixed(1)} ${c.toFixed(1)}"
      transform="rotate(-90 11 11)"/>
  </svg>`;
}

function row(l: Limit): string {
  const pct = isUnknown(l) ? "—" : `${pctLeft(l.util)}%`;
  // source_failed is not an estimate (see the detail view) — say so in the list too.
  const badge = l.status === "source_failed" ? "無法取得" : "估算";
  const right = isUnknown(l) ? `<span class="badge">${badge}</span>` : ring(l);
  return `<button class="lrow status-${l.status}" data-limit="${escapeHtml(l.id)}">
    <span class="lrow-pct">${pct}</span>
    <span class="lrow-label">${escapeHtml(displayName(l))}</span>
    ${right}
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
  return groups || `<div class="empty-note">工具目前未在執行</div>`;
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
      <div>TokenBar 叫不動 claude。請在終端機自己執行：</div>
      <div class="relogin-cmd">
        <code>${escapeHtml(MANUAL_LOGIN_CMD)}</code>
        <button class="relogin-copy" data-relogin-copy>${copied ? "已複製" : "複製"}</button>
      </div>
    </div>`;
  }
  if (state === "ok") {
    return `<div class="relogin-note">已開啟登入視窗。完成後按上方 ⟳ 重新整理。</div>`;
  }
  const busy = state === "launching";
  return `<button class="relogin" data-relogin ${busy ? "disabled" : ""}>${
    busy ? "開啟中…" : "重新登入 Claude"
  }</button>`;
}

function detail(l: Limit, opts: PanelOpts): string {
  const unknown = isUnknown(l);
  const left = pctLeft(l.util);

  // Status line: LOCKED / 估算 / pace + runway.
  let sub = "";
  if (l.status === "locked") {
    const reset = l.resets_at > 0 ? ` resets ${fmtDur(l.resets_at - nowSecs())}` : "";
    sub = `<span class="lock">LOCKED</span>${reset}`;
  } else if (l.status === "source_failed") {
    // No "估算" badge: nothing is estimated — the backend sends 0% placeholders.
    // Show the real reason instead of implying the 0% is a computed estimate.
    // The fallback stays provider-neutral: Codex's live degradation carries no
    // hint, and naming Claude there would just be a different lie.
    sub = `<span class="badge">無法取得</span> ${escapeHtml(l.hint ?? "暫時取不到用量資料")}`;
  } else if (l.status === "stale") {
    sub = `<span class="badge">stale</span> 上次執行時的數據，可能已變動`;
  } else if (l.status === "idle") {
    sub = `視窗已重置 · 工具目前未在執行`;
  } else {
    const parts: string[] = [];
    if (l.pace) {
      parts.push(
        l.pace.in_deficit
          ? `<span class="deficit">${Math.round(l.pace.deficit)}% in deficit</span>`
          : `<span class="onpace">On pace</span>`,
      );
    }
    if (l.runway_secs != null) parts.push(`empty in ~${fmtDur(l.runway_secs)}`);
    sub = parts.join(" · ");
  }

  // Gated on the backend's decision, never on what `hint` happens to say.
  const action =
    l.status === "source_failed" && l.action === "relogin"
      ? reloginBlock(opts.relogin ?? "idle", opts.copied ?? false)
      : "";

  const absLine = l.absolute
    ? `<div class="detail-abs">${fmtTokens(l.absolute[0])} / ${fmtTokens(l.absolute[1])} tokens</div>`
    : "";
  const reset =
    l.resets_at > 0 && l.status !== "locked"
      ? `resets ~${fmtClock(l.resets_at)} · ${fmtDur(l.resets_at - nowSecs())}`
      : "";

  return `<div class="detail status-${l.status}">
    <div class="detail-head">
      <button class="back" data-back title="Back">‹</button>
      <span class="detail-title">${escapeHtml(displayName(l))}</span>
    </div>
    <div class="detail-pct">${unknown ? "—" : `${left}%`}<small>left</small></div>
    <div class="dmeter"><div class="dmeter-fill" style="width:${unknown ? 0 : left}%"></div></div>
    ${sub ? `<div class="detail-sub">${sub}</div>` : ""}
    ${action}
    ${absLine}
    ${reset ? `<div class="detail-reset">${reset}</div>` : ""}
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
