import "./fonts.css";
import "./styles.css";
import type { Snapshot } from "./types";
import type { PanelView, ReloginState } from "./panel";
import { MANUAL_LOGIN_CMD } from "./panel";
import type { AnalyticsOpts, Group, Metric, SubTab } from "./analytics";
import type { Settings } from "./types";
import {
  getAnalytics,
  getSettings,
  getSnapshot,
  isTauri,
  mockScenarioNames,
  onSnapshot,
  refreshNow,
  relogin,
  resizeAnchored,
  setMockScenario,
  setSettings,
  setupEdgeSnap,
  SIZE,
  startWindowDrag,
} from "./datasource";
import { renderIsland } from "./island";
import { renderPanel } from "./panel";
import { renderAnalytics } from "./analytics";
import { fmtDur, fmtTokens, nowSecs } from "./format";

const $ = (id: string) => document.getElementById(id)!;

const ui = {
  expanded: false,
  compact: false,
  view: { kind: "list" } as PanelView,
  subtab: "overview" as SubTab,
  metric: "tokens" as Metric,
  group: "agent" as Group,
  range: "week" as "today" | "week",
  // Re-login button state. Held here, not in the DOM: renderCards() runs on
  // every 1s tick and would wipe anything written straight onto the elements.
  relogin: "idle" as ReloginState,
  copied: false,
};

let lastSnap: Snapshot | null = null;
let settings: Settings | null = null; // cached; compact toggle persists through it
let todayRate: number | null = null; // today's tok/min for the island (refreshed every 60s)

// ── rendering ────────────────────────────────────────────────────────

function renderSubtabs() {
  const subs: [SubTab, string][] = [
    ["overview", "Overview"],
    ["daily", "Daily"],
    ["hourly", "Hourly"],
    ["models", "Models"],
    ["agents", "Agents"],
    ["stats", "Stats"],
  ];
  $("subtabs").innerHTML = subs
    .map(([id, label]) => `<button data-sub="${id}" class="${ui.subtab === id ? "on" : ""}">${label}</button>`)
    .join("");
}

function renderToggles() {
  const showGroup = ui.subtab === "overview" || ui.subtab === "daily";
  $("toggles").innerHTML = `
    <div class="seg" data-seg="range">
      <button data-range="today" class="${ui.range === "today" ? "on" : ""}">Today</button>
      <button data-range="week" class="${ui.range === "week" ? "on" : ""}">Week</button>
    </div>
    <div class="seg" data-seg="metric">
      <button data-metric="tokens" class="${ui.metric === "tokens" ? "on" : ""}">Tokens</button>
      <button data-metric="price" class="${ui.metric === "price" ? "on" : ""}">Price</button>
    </div>
    ${
      showGroup
        ? `<div class="seg" data-seg="group">
             <button data-group="model" class="${ui.group === "model" ? "on" : ""}">Model</button>
             <button data-group="agent" class="${ui.group === "agent" ? "on" : ""}">Agent</button>
           </div>`
        : ""
    }`;
}

function renderIslandNow() {
  renderIsland($("island"), lastSnap, {
    mode: settings?.providers ?? "both",
    tokPerMin: todayRate,
  });
}

function renderCards() {
  renderPanel($("cards"), lastSnap, ui.view, { relogin: ui.relogin, copied: ui.copied });
}

/** Leaving a limit's detail view drops any re-login state with it, so the
 *  button never re-appears mid-flow on an unrelated limit. */
function setView(view: PanelView) {
  ui.view = view;
  ui.relogin = "idle";
  ui.copied = false;
  renderCards();
}

/** "X 前更新" in the panel header, from the snapshot's updated_at. */
function renderUpdated() {
  const el = $("updated");
  if (!lastSnap) {
    el.textContent = "";
    return;
  }
  const secs = Math.max(0, nowSecs() - lastSnap.updated_at);
  el.textContent = `${fmtDur(secs)} 前更新`;
}

async function renderAnalyticsNow() {
  const a = await getAnalytics(ui.range);
  $("rate").textContent = `${fmtTokens(a.tokPerMin)} tok/min`;
  const opts: AnalyticsOpts = { subtab: ui.subtab, metric: ui.metric, group: ui.group };
  renderAnalytics($("analytics"), a, opts);
}

// ── window sizing (locked per display mode, bottom-right anchored) ───
// The window is resized ONLY when a mode is entered (expand, compact toggle,
// settings open/close) — never on subtab clicks or the 1s tick, so page
// switches stay jank-free. #analytics has a fixed CSS height for the same
// reason: every subtab renders into the same box.

/** Natural panel height at mode entry: children sum. */
function contentHeight(): number {
  let h = 14; // panel top margin (6) + border (2) + breathing room
  for (const el of $("panel").children) h += (el as HTMLElement).offsetHeight;
  return Math.max(h, 120);
}

/** Collapsed island width depends on layout (dual providers need more room).
 *  Mirrors island.ts's branching: only an exact claude/codex renders one
 *  group, so only those get the narrow width — an unknown value shows both
 *  and must keep the wide one. */
function collapsedW(): number {
  const p = settings?.providers ?? "both";
  return p === "claude" || p === "codex" ? 270 : SIZE.collapsed.w;
}

/** Resize the OS window for the current mode (no-op in browser). */
function fitWindow() {
  const { w, h } = ui.expanded
    ? { w: SIZE.expanded.w, h: contentHeight() }
    : { w: collapsedW(), h: SIZE.collapsed.h };
  resizeAnchored(w, h);
}

/** ⊟/⊞ toggle between compact (limits only) and full (with analytics). */
function renderModeBtn() {
  const btn = $("mode");
  btn.textContent = ui.compact ? "⊞" : "⊟";
  btn.title = ui.compact ? "顯示分析" : "精簡模式";
}

async function applyCompact() {
  document.body.classList.toggle("compact", ui.compact);
  renderModeBtn();
  if (ui.expanded && !ui.compact) {
    renderSubtabs();
    renderToggles();
    await renderAnalyticsNow();
  }
  fitWindow();
}

async function renderSettings() {
  const s = await getSettings();
  $("settings").innerHTML = `
    <label class="srow"><input type="checkbox" id="s-autostart" ${s.autostart ? "checked" : ""}/> 開機自動啟動</label>
    <div class="srow">Claude 權杖更新 <select id="s-refresh">
      <option value="off" ${s.allow_token_refresh ? "" : "selected"}>關閉（過期時顯示估算）</option>
      <option value="on" ${s.allow_token_refresh ? "selected" : ""}>開啟（過期自動換新）</option>
    </select><span class="warn">可能影響 Claude Code 登入</span></div>
    <div class="srow">警戒 <input type="number" id="s-warn" value="${s.warn_pct}" min="1" max="100"/>% · 危險 <input type="number" id="s-crit" value="${s.crit_pct}" min="1" max="100"/>%</div>
    <div class="srow">顯示平台 <select id="s-providers">
      <option value="both" ${s.providers !== "claude" && s.providers !== "codex" ? "selected" : ""}>兩個都顯示</option>
      <option value="claude" ${s.providers === "claude" ? "selected" : ""}>只顯示 Claude</option>
      <option value="codex" ${s.providers === "codex" ? "selected" : ""}>只顯示 Codex</option>
    </select></div>
    <div class="srow">Codex 用量來源 <select id="s-codex-source">
      <option value="live" ${s.codex_usage_source === "live" ? "selected" : ""}>即時帳號用量</option>
      <option value="auto" ${s.codex_usage_source === "auto" ? "selected" : ""}>自動（即時優先）</option>
      <option value="local" ${s.codex_usage_source !== "live" && s.codex_usage_source !== "auto" ? "selected" : ""}>本機 session 快照</option>
    </select></div>`;
}

function readSettingsForm(): Settings {
  const v = (id: string) => $(id) as HTMLInputElement;
  return {
    autostart: v("s-autostart").checked,
    allow_token_refresh: ($("s-refresh") as HTMLSelectElement).value === "on",
    warn_pct: +v("s-warn").value || 75,
    crit_pct: +v("s-crit").value || 90,
    compact: ui.compact,
    providers: (($("s-providers") as HTMLSelectElement).value || "both") as Settings["providers"],
    codex_usage_source: (($("s-codex-source") as HTMLSelectElement).value || "local") as Settings["codex_usage_source"],
  };
}

async function setExpanded(on: boolean) {
  ui.expanded = on;
  document.body.classList.toggle("expanded", on);
  document.body.classList.toggle("collapsed", !on);
  if (on) {
    ui.view = { kind: "list" };
    ui.relogin = "idle";
    ui.copied = false;
    renderModeBtn();
    renderCards();
    renderUpdated();
    if (!ui.compact) {
      renderSubtabs();
      renderToggles();
      await renderAnalyticsNow();
    }
  }
  fitWindow();
}

// ── events ───────────────────────────────────────────────────────────

function wireEvents() {
  // Island: drag to move the window, click (no drag) to expand.
  const island = $("island");
  let downAt: { x: number; y: number } | null = null;
  let dragged = false;
  island.addEventListener("pointerdown", (e) => {
    downAt = { x: e.clientX, y: e.clientY };
    dragged = false;
  });
  island.addEventListener("pointermove", (e) => {
    if (!downAt || !(e.buttons & 1) || dragged) return;
    if (Math.abs(e.clientX - downAt.x) > 4 || Math.abs(e.clientY - downAt.y) > 4) {
      dragged = true;
      startWindowDrag();
    }
  });
  island.addEventListener("click", () => {
    if (!dragged) setExpanded(true);
  });

  $("collapse").addEventListener("click", () => setExpanded(false));

  // Manual refresh: spin until the next snapshot lands (3s safety timeout).
  $("refresh").addEventListener("click", () => {
    $("refresh").classList.add("busy");
    setTimeout(() => $("refresh").classList.remove("busy"), 3000);
    refreshNow();
  });

  $("gear").addEventListener("click", async () => {
    const el = $("settings");
    if (el.hasAttribute("hidden")) {
      await renderSettings();
      el.removeAttribute("hidden");
    } else {
      el.setAttribute("hidden", "");
    }
    fitWindow();
  });
  $("settings").addEventListener("change", async () => {
    settings = readSettingsForm();
    setSettings(settings);
    renderIslandNow(); // island layout may have changed
    // The display filter scopes analytics too, and the backend applies it on
    // demand — so re-pull rather than leave the page stale until the 60s tick.
    // (Limits re-arrive filtered on the scheduler's next round.)
    if (ui.expanded && !ui.compact) await renderAnalyticsNow();
  });

  $("mode").addEventListener("click", async () => {
    ui.compact = !ui.compact;
    await applyCompact();
    if (settings) {
      settings.compact = ui.compact;
      setSettings(settings);
    }
  });

  // Limits list ↔ per-limit detail drill-down, plus the re-login affordance.
  $("cards").addEventListener("click", (e) => {
    const el = e.target as HTMLElement;
    if (el.closest("[data-back]")) {
      setView({ kind: "list" });
      return;
    }

    // Hand off to the official `claude auth login`. Any failure (usually:
    // claude isn't on TokenBar's PATH) becomes the manual-command fallback —
    // never a dead end.
    if (el.closest("[data-relogin]")) {
      ui.relogin = "launching";
      renderCards();
      relogin().then(
        () => {
          ui.relogin = "ok";
          renderCards();
        },
        () => {
          ui.relogin = "failed";
          renderCards();
        },
      );
      return;
    }

    if (el.closest("[data-relogin-copy]")) {
      // Best-effort: the <code> is selectable too, so a blocked clipboard
      // still leaves the user a way to copy the command by hand.
      navigator.clipboard?.writeText(MANUAL_LOGIN_CMD).then(
        () => {
          ui.copied = true;
          renderCards();
          setTimeout(() => {
            ui.copied = false;
            renderCards();
          }, 1500);
        },
        () => {},
      );
      return;
    }

    const rowEl = el.closest("[data-limit]");
    if (rowEl) {
      setView({ kind: "detail", id: rowEl.getAttribute("data-limit")! });
    }
  });

  $("subtabs").addEventListener("click", (e) => {
    const t = (e.target as HTMLElement).closest("[data-sub]");
    if (!t) return;
    ui.subtab = t.getAttribute("data-sub") as SubTab;
    renderSubtabs();
    renderToggles();
    renderAnalyticsNow();
  });

  $("toggles").addEventListener("click", (e) => {
    const el = e.target as HTMLElement;
    const t = el.closest("[data-range],[data-metric],[data-group]");
    if (!t) return;
    if (t.hasAttribute("data-range")) ui.range = t.getAttribute("data-range") as "today" | "week";
    if (t.hasAttribute("data-metric")) ui.metric = t.getAttribute("data-metric") as Metric;
    if (t.hasAttribute("data-group")) ui.group = t.getAttribute("data-group") as Group;
    renderToggles();
    renderAnalyticsNow();
  });
}

function wireDevBar() {
  if (isTauri()) return;
  const bar = $("devbar");
  bar.style.display = "flex";
  bar.innerHTML =
    `<span class="dev-label">mock:</span>` +
    mockScenarioNames()
      .map((n) => `<button data-scn="${n}">${n}</button>`)
      .join("");
  bar.addEventListener("click", (e) => {
    const t = (e.target as HTMLElement).closest("[data-scn]");
    if (t) setMockScenario(t.getAttribute("data-scn")!);
  });
}

// ── boot ─────────────────────────────────────────────────────────────

async function boot() {
  wireEvents();
  wireDevBar();
  setupEdgeSnap();

  settings = await getSettings();
  ui.compact = settings.compact;
  document.body.classList.toggle("compact", ui.compact);
  renderModeBtn();
  fitWindow(); // collapsed width depends on the display filter

  lastSnap = await getSnapshot();
  renderIslandNow();

  await onSnapshot((s) => {
    lastSnap = s;
    $("refresh").classList.remove("busy");
    renderIslandNow();
    if (ui.expanded) {
      renderCards();
      renderUpdated();
    }
  });

  // Tick countdowns once a second from the cached snapshot. Never resizes
  // the window — heights are locked per display mode.
  setInterval(() => {
    renderIslandNow();
    if (ui.expanded) {
      renderCards();
      renderUpdated();
    }
  }, 1000);

  // Today's burn rate for the island aux readout.
  const refreshToday = async () => {
    try {
      todayRate = (await getAnalytics("today")).tokPerMin;
    } catch {
      todayRate = null;
    }
  };
  await refreshToday();
  renderIslandNow();
  setInterval(refreshToday, 60_000);
}

window.addEventListener("DOMContentLoaded", boot);
