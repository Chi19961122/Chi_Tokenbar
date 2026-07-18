import "./fonts.css";
import "./styles.css";
import type { Analytics, Snapshot } from "./types";
import type { ReloginState } from "./panel";
import { MANUAL_LOGIN_CMD } from "./panel";
import type { AnalyticsOpts, Group, Metric, SubTab } from "./analytics";
import type { Settings } from "./types";
import {
  getAnalytics,
  getSettings,
  getSnapshot,
  hideWindow,
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
import { islandIntent, renderIsland, windowShort } from "./island";
import { renderPanel } from "./panel";
import { showIslandMenu } from "./contextmenu";
import { renderAnalytics } from "./analytics";
import { renderSharePanel } from "./share-panel";
import type { ShareStyle } from "./share";
import { fmtTokens, nowSecs } from "./format";
import { getLocale, resolveLocale, setLocale, t } from "./i18n";

const $ = (id: string) => document.getElementById(id)!;

const ui = {
  expanded: false,
  compact: false,
  subtab: "overview" as SubTab,
  metric: "tokens" as Metric,
  group: "agent" as Group,
  range: "week" as "today" | "week" | "month",
  // 階段 D 戰報 Share (report subtab). style/range persist to settings; the fuel
  // model/agent sub-toggle and the quota-note override are session-only.
  // shareQuotaNote null → follow the style default (island_card on, else off).
  shareStyle: "statement" as ShareStyle,
  shareRange: "week" as "today" | "week" | "month",
  shareFuelGroup: "model" as "model" | "agent",
  shareQuotaNote: null as boolean | null,
  // Usage-tab quota summary expanded? Session-only (not persisted): the Usage
  // tab always re-opens onto the collapsed one-line digest.
  quotaExpanded: false,
  // Re-login button state. Held here, not in the DOM: renderCards() runs on
  // every 1s tick and would wipe anything written straight onto the elements.
  relogin: "idle" as ReloginState,
  copied: false,
};

let lastSnap: Snapshot | null = null;
let settings: Settings | null = null; // cached; compact toggle persists through it
let todayRate: number | null = null; // today's tok/min for the island (refreshed every 60s)
let todayCost: number | null = null; // today's est. cost for the island aux (60s cache)

// Last Analytics payload, keyed so an expand can paint from it instantly instead
// of blocking on the get_analytics IPC (100-500ms). The key captures everything
// that changes what the backend returns for a fetch — range, the provider
// filter, and the snapshot generation (updated_at) — but NOT subtab/metric/group,
// which only re-slice the *same* payload at render time. Single-entry on purpose
// (§ expand speed): the common case is re-opening onto the same data.
let cachedAnalytics: { key: string; data: Analytics } | null = null;

// ── rendering ────────────────────────────────────────────────────────

function renderSubtabs() {
  const subs: [SubTab, string][] = [
    ["overview", t("subtab.overview")],
    ["share", t("subtab.share")],
    ["hourly", t("subtab.hourly")],
    ["stats", t("subtab.stats")],
    ["report", t("subtab.report")],
  ];
  $("subtabs").innerHTML = subs
    .map(([id, label]) => `<button data-sub="${id}" class="${ui.subtab === id ? "on" : ""}">${label}</button>`)
    .join("");
}

function renderToggles() {
  // The report panel owns its own controls (style/range/etc), so the shared
  // range/metric/group toggles render nothing there.
  if (ui.subtab === "report") {
    $("toggles").innerHTML = "";
    return;
  }
  // The model/agent group toggle drives both the overview stacks and the
  // "share" breakdown (階段 C: agents folded into this toggle).
  const showGroup = ui.subtab === "overview" || ui.subtab === "share";
  $("toggles").innerHTML = `
    <div class="seg" data-seg="range">
      <button data-range="today" class="${ui.range === "today" ? "on" : ""}">${t("toggle.today")}</button>
      <button data-range="week" class="${ui.range === "week" ? "on" : ""}">${t("toggle.week")}</button>
      <button data-range="month" class="${ui.range === "month" ? "on" : ""}">${t("toggle.month")}</button>
    </div>
    <div class="seg" data-seg="metric">
      <button data-metric="tokens" class="${ui.metric === "tokens" ? "on" : ""}">${t("toggle.tokens")}</button>
      <button data-metric="price" class="${ui.metric === "price" ? "on" : ""}">${t("toggle.price")}</button>
    </div>
    ${
      showGroup
        ? `<div class="seg" data-seg="group">
             <button data-group="model" class="${ui.group === "model" ? "on" : ""}">${t("toggle.model")}</button>
             <button data-group="agent" class="${ui.group === "agent" ? "on" : ""}">${t("toggle.agent")}</button>
           </div>`
        : ""
    }`;
}

function renderIslandNow() {
  renderIsland($("island"), lastSnap, {
    mode: settings?.providers ?? "both",
    pinClaude: settings?.island_pin_claude ?? "auto",
    pinCodex: settings?.island_pin_codex ?? "auto",
    resetDisplay: settings?.reset_display ?? "relative",
    aux: settings?.island_aux ?? "tok_per_min",
    tokPerMin: todayRate,
    costToday: todayCost,
    now: nowSecs(),
    locale: getLocale(),
  });
}

function renderCards() {
  // The Usage tab leads with a one-line quota digest (階段 C). The full list
  // stays when in Limits (compact) or while settings is open — settings tweaks
  // (Providers, thresholds) visibly change the list, so it must stay full there.
  const settingsOpen = !$("settings").hasAttribute("hidden");
  const variant: "full" | "summary" = ui.compact || settingsOpen ? "full" : "summary";
  renderPanel($("cards"), lastSnap, {
    relogin: ui.relogin,
    copied: ui.copied,
    resetDisplay: settings?.reset_display ?? "relative",
    now: nowSecs(),
    locale: getLocale(),
    variant,
    summaryExpanded: ui.quotaExpanded,
  });
}

/** "Refresh in Ns" in the header — a live countdown to the next backend data
 *  fetch. Derived from the snapshot's `next_fetch_in` (measured at `updated_at`),
 *  so it ticks down on the 1s island tick and restarts on its own whenever a
 *  fresh snapshot lands — the scheduler's regular round or a manual refresh. */
function renderRefresh() {
  const el = $("refresh-in");
  if (!lastSnap) {
    el.textContent = "";
    return;
  }
  const remaining = Math.max(0, lastSnap.next_fetch_in - (nowSecs() - lastSnap.updated_at));
  el.innerHTML = t("header.refreshIn", { v: `<span class="num">${remaining}s</span>` });
}

/** Cache key for a get_analytics *fetch* — the inputs that change the payload.
 *  subtab/metric/group are deliberately absent: they re-slice the same data. */
function analyticsDataKey(): string {
  return `${ui.range}|${settings?.providers ?? "both"}|${lastSnap?.updated_at ?? 0}`;
}

/** Paint the analytics layer from an already-fetched payload (no IPC). */
function renderAnalyticsInto(a: Analytics): void {
  $("rate").textContent = `${fmtTokens(a.tokPerMin)} ${t("analytics.tokPerMin")}`;
  const opts: AnalyticsOpts = { subtab: ui.subtab, metric: ui.metric, group: ui.group };
  renderAnalytics($("analytics"), a, opts);
}

/** Glass placeholder sized to the fixed 300px #analytics box, shown while the
 *  first get_analytics for a key is in flight so the window measures its final
 *  height in one fitWindow() and never jumps a second time. */
function showAnalyticsSkeleton(): void {
  $("analytics").innerHTML =
    `<div class="tiles">` +
    `<div class="tile sk"></div>`.repeat(4) +
    `</div><div class="chart-wrap"><div class="sk sk-chart"></div></div>`;
}

// ── 階段 D 戰報 Share (report subtab) ────────────────────────────────
// The report panel uses its own range (ui.shareRange), independent of the usage
// analytics range, so it keeps a separate one-entry cache.
let shareCache: { range: string; data: Analytics } | null = null;

function persistShare(): void {
  if (!settings) return;
  settings.share_style = ui.shareStyle;
  settings.share_range = ui.shareRange;
  void setSettings(settings);
}

/** Paint the share panel from an already-fetched payload (no IPC). */
function paintReport(a: Analytics): void {
  $("rate").textContent = `${fmtTokens(a.tokPerMin)} ${t("analytics.tokPerMin")}`;
  const style = ui.shareStyle;
  const quotaNote = ui.shareQuotaNote ?? style === "island_card";
  renderSharePanel($("analytics"), {
    analytics: a,
    limits: lastSnap?.limits ?? [],
    locale: getLocale(),
    style,
    range: ui.shareRange,
    fuelGroup: ui.shareFuelGroup,
    quotaNote,
    setStyle: (s) => {
      ui.shareStyle = s;
      ui.shareQuotaNote = null; // reset override so the new style's default applies
      persistShare();
      paintReport(a);
    },
    setRange: (r) => {
      ui.shareRange = r;
      persistShare();
      void renderReportNow();
    },
    setFuelGroup: (g) => {
      ui.shareFuelGroup = g;
      paintReport(a);
    },
    setQuotaNote: (on) => {
      ui.shareQuotaNote = on;
      paintReport(a);
    },
  });
}

/** Fetch (or reuse) the share-range analytics and paint the report panel. */
async function renderReportNow(): Promise<void> {
  const range = ui.shareRange;
  if (shareCache && shareCache.range === range) {
    paintReport(shareCache.data);
    return;
  }
  const a = await getAnalytics(range);
  shareCache = { range, data: a };
  // Guard against a rapid range switch / subtab change superseding this fetch.
  if (ui.subtab === "report" && ui.shareRange === range) paintReport(a);
}

/** Fetch (or reuse) the analytics payload and paint it. A cache hit renders
 *  synchronously with no IPC; a miss awaits get_analytics, then paints only if
 *  the key is still current (a rapid range/provider switch can supersede it). */
async function renderAnalyticsNow(): Promise<void> {
  if (ui.subtab === "report") {
    await renderReportNow();
    return;
  }
  const key = analyticsDataKey();
  if (cachedAnalytics && cachedAnalytics.key === key) {
    renderAnalyticsInto(cachedAnalytics.data);
    return;
  }
  const a = await getAnalytics(ui.range);
  cachedAnalytics = { key, data: a };
  if (analyticsDataKey() === key) renderAnalyticsInto(a);
}

/** Non-blocking entry used on mode entry (expand / compact toggle): a cache hit
 *  paints instantly; a miss shows the skeleton immediately and fills it in when
 *  the fetch lands — either way fitWindow() can run right after without waiting. */
function beginAnalytics(): void {
  if (ui.subtab === "report") {
    void renderReportNow();
    return;
  }
  const key = analyticsDataKey();
  if (cachedAnalytics && cachedAnalytics.key === key) {
    renderAnalyticsInto(cachedAnalytics.data);
    return;
  }
  showAnalyticsSkeleton();
  void renderAnalyticsNow();
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

/** Header tabs are the display switch: "Limits" = compact (limits only),
 *  "Analytics" = full (with the analytics layer). Selected state mirrors the
 *  seg controls (§ compact toggle — behavior unchanged, only the affordance). */
function renderTabs() {
  $("tab-limits").classList.toggle("on", ui.compact);
  $("tab-analytics").classList.toggle("on", !ui.compact);
}

/** Localize the strings that live in static index.html (header tabs + button
 *  tooltips). Called on boot and whenever the locale changes — everything else
 *  is re-rendered from JS templates that already call t(). */
function applyStaticI18n() {
  $("tab-limits").textContent = t("tab.limits");
  $("tab-analytics").textContent = t("tab.usage");
  $("refresh").title = t("header.refreshTitle");
  $("gear").title = t("header.settingsTitle");
  $("collapse").title = t("header.collapseTitle");
}

/** Full re-paint after a locale switch. Every render function is idempotent, so
 *  this just re-runs the ones relevant to the current mode. Callers run
 *  fitWindow() afterwards (translated text can change measured heights). */
function rerenderAll() {
  applyStaticI18n();
  renderTabs();
  renderIslandNow();
  if (ui.expanded) {
    renderCards();
    renderRefresh();
    if (!ui.compact) {
      renderSubtabs();
      renderToggles();
      beginAnalytics();
    }
  }
}

/** Switch the display mode from the header tabs, persisting the choice. */
async function setCompact(compact: boolean) {
  if (ui.compact === compact) return;
  ui.compact = compact;
  await applyCompact();
  if (settings) {
    settings.compact = ui.compact;
    setSettings(settings);
  }
}

async function applyCompact() {
  document.body.classList.toggle("compact", ui.compact);
  renderTabs();
  if (ui.expanded) {
    // Re-render the limits before measuring: the variant (full list vs summary
    // digest) follows the tab, and fitWindow() measuring the *old* variant
    // locks the window at the wrong height until the next mode change.
    renderCards();
    if (!ui.compact) {
      renderSubtabs();
      renderToggles();
      beginAnalytics();
    }
  }
  fitWindow();
}

/**
 * Settings, grouped by the question each answer belongs to.
 *
 * The six rows used to sit in one flat list, so finding anything meant reading
 * all of it. They are grouped by *what the user is trying to change*, and the
 * headings reuse `.lsec-head` — the same section marker the limits list uses,
 * so the panel has one vocabulary for "group of things" rather than two.
 *
 * Grouping rationale:
 *  - 啟動與視窗 — when TokenBar appears and whether it stays on top. (Not just
 *    「視窗」: autostart is about launching, not the window, and mislabelling it
 *    is exactly the kind of thing that makes a setting unfindable.)
 *  - 顯示與通知 — what you get told about: which platforms show at all, and how
 *    full is full enough to interrupt you.
 *  - 資料來源 — where the numbers come from. Both rows carry a cost the user
 *    should weigh (token rotation, network queries), which is why they read as
 *    one decision rather than two unrelated dropdowns.
 *
 * Note hierarchy, existing tokens only: `.snote` (dim) explains, `.warn`
 * (amber) is for a row that can bite — currently only token refresh.
 *
 * The `id`s are load-bearing: readSettingsForm() reads the form back by id.
 */
/** Minimal escape for text interpolated into settings <option> labels. */
function escAttr(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
}

/** Island-pin <option>s for one provider: Auto / 5h / Week + any model windows
 *  present in the current snapshot. A stored `model:<id>` whose limit has since
 *  vanished is still listed (selected) so opening settings never silently drops
 *  the pin — it matches the "pinned but no data → —" island behaviour. */
function pinOptionsHtml(provider: "anthropic" | "codex", current: string): string {
  const base: [string, string][] = [
    ["auto", t("settings.pinAuto")],
    ["5h", t("settings.pin5h")],
    ["week", t("settings.pinWeek")],
  ];
  const models = (lastSnap?.limits ?? []).filter(
    (l) => l.provider === provider && !l.id.endsWith(".5h") && !l.id.endsWith(".week"),
  );
  let html = base
    .map(([v, label]) => `<option value="${v}" ${current === v ? "selected" : ""}>${label}</option>`)
    .join("");
  for (const l of models) {
    const v = `model:${l.id}`;
    html += `<option value="${escAttr(v)}" ${current === v ? "selected" : ""}>${escAttr(windowShort(l) || l.label)}</option>`;
  }
  if (current.startsWith("model:") && !models.some((l) => `model:${l.id}` === current)) {
    html += `<option value="${escAttr(current)}" selected>${escAttr(current.slice("model:".length))}</option>`;
  }
  return html;
}

async function renderSettings() {
  const s = await getSettings();
  $("settings").innerHTML = `
    <div class="sgroup">
      <div class="lsec-head">${t("settings.startupWindow")}</div>
      <label class="srow">
        <span class="slabel">${t("settings.launchAtStartup")}</span>
        <input type="checkbox" id="s-autostart" ${s.autostart ? "checked" : ""}/>
      </label>
      <label class="srow">
        <span class="slabel">${t("settings.alwaysOnTop")}<span class="snote">${t("settings.alwaysOnTopNote")}</span></span>
        <input type="checkbox" id="s-always-on-top" ${s.always_on_top ? "checked" : ""}/>
      </label>
    </div>

    <div class="sgroup">
      <div class="lsec-head">${t("settings.displayNotifications")}</div>
      <label class="srow">
        <span class="slabel">${t("settings.language")}</span>
        <select id="s-locale">
          <option value="system" ${s.locale !== "en" && s.locale !== "zh-TW" ? "selected" : ""}>${t("settings.localeSystem")}</option>
          <option value="zh-TW" ${s.locale === "zh-TW" ? "selected" : ""}>中文</option>
          <option value="en" ${s.locale === "en" ? "selected" : ""}>English</option>
        </select>
      </label>
      <label class="srow">
        <span class="slabel">${t("settings.providers")}</span>
        <select id="s-providers">
          <option value="both" ${s.providers !== "claude" && s.providers !== "codex" ? "selected" : ""}>${t("settings.providersBoth")}</option>
          <option value="claude" ${s.providers === "claude" ? "selected" : ""}>${t("settings.providersClaude")}</option>
          <option value="codex" ${s.providers === "codex" ? "selected" : ""}>${t("settings.providersCodex")}</option>
        </select>
      </label>
      <div class="srow">
        <span class="slabel">${t("settings.notifyAt")}<span class="snote">${t("settings.notifyNote")}</span></span>
        <span class="sfields">
          ${t("settings.warn")} <input type="number" id="s-warn" value="${s.warn_pct}" min="1" max="100"/>%
          <span class="sdot">·</span>
          ${t("settings.crit")} <input type="number" id="s-crit" value="${s.crit_pct}" min="1" max="100"/>%
        </span>
      </div>
    </div>

    <div class="sgroup">
      <div class="lsec-head">${t("settings.island")}</div>
      <label class="srow">
        <span class="slabel">${t("settings.expandDefault")}</span>
        <select id="s-expand">
          <option value="compact" ${s.expand_default !== "usage" ? "selected" : ""}>${t("settings.expandCompact")}</option>
          <option value="usage" ${s.expand_default === "usage" ? "selected" : ""}>${t("settings.expandUsage")}</option>
        </select>
      </label>
      <label class="srow">
        <span class="slabel">${t("settings.pinClaude")}</span>
        <select id="s-pin-claude">${pinOptionsHtml("anthropic", s.island_pin_claude)}</select>
      </label>
      <label class="srow">
        <span class="slabel">${t("settings.pinCodex")}</span>
        <select id="s-pin-codex">${pinOptionsHtml("codex", s.island_pin_codex)}</select>
      </label>
      <label class="srow">
        <span class="slabel">${t("settings.islandAux")}</span>
        <select id="s-aux">
          <option value="off" ${s.island_aux === "off" ? "selected" : ""}>${t("settings.auxOff")}</option>
          <option value="tok_per_min" ${s.island_aux !== "off" && s.island_aux !== "cost_today" ? "selected" : ""}>${t("settings.auxTokPerMin")}</option>
          <option value="cost_today" ${s.island_aux === "cost_today" ? "selected" : ""}>${t("settings.auxCostToday")}</option>
        </select>
      </label>
      <label class="srow">
        <span class="slabel">${t("settings.resetDisplay")}</span>
        <select id="s-reset">
          <option value="relative" ${s.reset_display !== "clock" ? "selected" : ""}>${t("settings.resetRelative")}</option>
          <option value="clock" ${s.reset_display === "clock" ? "selected" : ""}>${t("settings.resetClock")}</option>
        </select>
      </label>
    </div>

    <div class="sgroup">
      <div class="lsec-head">${t("settings.dataSources")}</div>
      <label class="srow">
        <span class="slabel">${t("settings.claudeRefresh")}<span class="warn">${t("settings.claudeRefreshWarn")}</span></span>
        <select id="s-refresh">
          <option value="off" ${s.allow_token_refresh ? "" : "selected"}>${t("settings.refreshOff")}</option>
          <option value="on" ${s.allow_token_refresh ? "selected" : ""}>${t("settings.refreshOn")}</option>
        </select>
      </label>
      <label class="srow">
        <span class="slabel">${t("settings.codexSource")}<span class="snote">${t("settings.codexSourceNote")}</span></span>
        <select id="s-codex-source">
          <option value="live" ${s.codex_usage_source === "live" ? "selected" : ""}>${t("settings.codexLive")}</option>
          <option value="auto" ${s.codex_usage_source === "auto" ? "selected" : ""}>${t("settings.codexAuto")}</option>
          <option value="local" ${s.codex_usage_source !== "live" && s.codex_usage_source !== "auto" ? "selected" : ""}>${t("settings.codexLocal")}</option>
        </select>
      </label>
      <label class="srow">
        <span class="slabel">${t("settings.toolOpencode")}<span class="snote">${t("settings.toolNote")}</span></span>
        <input type="checkbox" id="s-tool-opencode" ${s.tool_opencode ? "checked" : ""}/>
      </label>
      <label class="srow">
        <span class="slabel">${t("settings.toolGemini")}<span class="snote">${t("settings.toolNote")}</span></span>
        <input type="checkbox" id="s-tool-gemini" ${s.tool_gemini ? "checked" : ""}/>
      </label>
    </div>`;
}

function readSettingsForm(): Settings {
  const v = (id: string) => $(id) as HTMLInputElement;
  // Merge the form fields over the cached settings so fields with no form
  // control (階段 D share_style / share_range, and any future non-form setting)
  // are preserved rather than silently dropped on an unrelated settings change.
  return {
    ...(settings ?? ({} as Settings)),
    autostart: v("s-autostart").checked,
    always_on_top: v("s-always-on-top").checked,
    allow_token_refresh: ($("s-refresh") as HTMLSelectElement).value === "on",
    warn_pct: +v("s-warn").value || 75,
    crit_pct: +v("s-crit").value || 90,
    compact: ui.compact,
    providers: (($("s-providers") as HTMLSelectElement).value || "both") as Settings["providers"],
    codex_usage_source: (($("s-codex-source") as HTMLSelectElement).value || "local") as Settings["codex_usage_source"],
    locale: ($("s-locale") as HTMLSelectElement).value || "system",
    expand_default: (($("s-expand") as HTMLSelectElement).value || "compact") as Settings["expand_default"],
    island_pin_claude: ($("s-pin-claude") as HTMLSelectElement).value || "auto",
    island_pin_codex: ($("s-pin-codex") as HTMLSelectElement).value || "auto",
    island_aux: (($("s-aux") as HTMLSelectElement).value || "tok_per_min") as Settings["island_aux"],
    reset_display: (($("s-reset") as HTMLSelectElement).value || "relative") as Settings["reset_display"],
    tool_opencode: v("s-tool-opencode").checked,
    tool_gemini: v("s-tool-gemini").checked,
  };
}

/** Open the settings panel, expanding the window first if it is collapsed.
 *  Shared by the gear button and the island context menu's "Settings" item. */
async function openSettingsPanel(): Promise<void> {
  const el = $("settings");
  if (!el.hasAttribute("hidden")) return; // already open
  if (!ui.expanded) setExpanded(true);
  await renderSettings();
  el.removeAttribute("hidden");
  document.body.classList.add("settings-open");
  renderCards(); // settings open forces the full limits list (see renderCards)
  fitWindow();
}

/** Hold the panel's backdrop blur off the first paint. WebView2 rasterizes a
 *  fresh backdrop-filter expensively on the frame the panel becomes visible;
 *  paying that cost while the window is also resizing is the visible hitch on
 *  expand. We paint the glass flat first, then re-enable blur two frames later
 *  so it fades in via the #panel transition (a no-op step under
 *  prefers-reduced-motion, where the transition is disabled but the one-frame
 *  defer still spares the first paint). */
function deferPanelBlur(): void {
  document.body.classList.add("panel-no-blur");
  requestAnimationFrame(() =>
    requestAnimationFrame(() => document.body.classList.remove("panel-no-blur")),
  );
}

function setExpanded(on: boolean): void {
  ui.expanded = on;
  document.body.classList.toggle("expanded", on);
  document.body.classList.toggle("collapsed", !on);
  if (on) {
    deferPanelBlur();
    ui.relogin = "idle";
    ui.copied = false;
    renderTabs();
    renderCards();
    renderRefresh();
    if (!ui.compact) {
      renderSubtabs();
      renderToggles();
      // Non-blocking: paint from cache or drop a fixed-height skeleton, so
      // fitWindow() below measures the final height and resizes exactly once —
      // never waiting on the get_analytics IPC.
      beginAnalytics();
    }
  }
  // Size the window immediately (analytics box is a fixed 300px, so its content
  // arriving later never changes the measured height).
  fitWindow();
}

// ── events ───────────────────────────────────────────────────────────

function wireEvents() {
  // Island: drag to move the window, click (no drag) to expand, hide button to
  // send it to the tray. Routing lives in islandIntent (island.ts) — listeners
  // are delegated because renderIsland rewrites this subtree every second, so
  // anything bound to the button itself would not survive the next tick.
  const island = $("island");
  let downAt: { x: number; y: number } | null = null;
  let dragged = false;
  island.addEventListener("pointerdown", (e) => {
    dragged = false;
    // Arm the drag everywhere except on the hide button: an OS-level drag takes
    // over the pointer and would swallow the click that button exists for.
    downAt = islandIntent(e.target, false) === "hide" ? null : { x: e.clientX, y: e.clientY };
  });
  island.addEventListener("pointermove", (e) => {
    if (!downAt || !(e.buttons & 1) || dragged) return;
    if (Math.abs(e.clientX - downAt.x) > 4 || Math.abs(e.clientY - downAt.y) > 4) {
      dragged = true;
      startWindowDrag();
    }
  });
  island.addEventListener("click", (e) => {
    const intent = islandIntent(e.target, dragged);
    if (intent === "hide") hideWindow();
    else if (intent === "expand") {
      // expand_default picks the entry tab: Limits (compact) or Usage.
      if (settings) {
        ui.compact = settings.expand_default !== "usage";
        document.body.classList.toggle("compact", ui.compact);
      }
      setExpanded(true);
    }
  });

  // Right-click (D4): pin a limit, switch provider, open settings, hide.
  island.addEventListener("contextmenu", (e) => {
    e.preventDefault();
    if (!settings) return;
    void showIslandMenu({
      settings,
      snap: lastSnap,
      x: e.clientX,
      y: e.clientY,
      apply: (patch) => {
        settings = { ...settings!, ...patch };
        void setSettings(settings);
        renderIslandNow();
        if (ui.expanded) renderCards();
      },
      openSettings: () => void openSettingsPanel(),
      hide: () => void hideWindow(),
    });
  });

  $("collapse").addEventListener("click", () => setExpanded(false));

  // Manual refresh: spin until the next snapshot lands (3s safety timeout).
  $("refresh").addEventListener("click", () => {
    $("refresh").classList.add("busy");
    setTimeout(() => $("refresh").classList.remove("busy"), 3000);
    refreshNow();
  });

  // Settings is a mode, not just another band: opening it folds the analytics
  // layer away (see .settings-open in styles.css — the panel is height-locked
  // and clipped, and settings + limits + analytics together overflow a 1080p
  // work area). fitWindow() then re-measures, as it already does per mode.
  $("gear").addEventListener("click", async () => {
    const el = $("settings");
    if (el.hasAttribute("hidden")) {
      await openSettingsPanel();
    } else {
      el.setAttribute("hidden", "");
      document.body.classList.remove("settings-open");
      renderCards(); // back to the Usage-tab summary digest
      fitWindow();
    }
  });
  $("settings").addEventListener("change", async () => {
    const prevLocale = getLocale();
    settings = readSettingsForm();
    await setSettings(settings);

    // Locale changed → re-translate everything, including the open settings
    // panel, then re-measure (zh/en text differ in length).
    const nextLocale = resolveLocale(settings.locale);
    if (nextLocale !== prevLocale) {
      setLocale(nextLocale);
      await renderSettings();
      rerenderAll();
      fitWindow();
      return;
    }

    renderIslandNow(); // island layout may have changed
    // The display filter scopes analytics too, and the backend applies it on
    // demand — so re-pull rather than leave the page stale until the 60s tick.
    // (Limits re-arrive filtered on the scheduler's next round.)
    // The provider filter is part of the analytics cache key, so this misses
    // the stale entry and re-fetches; non-blocking so the settings UI stays live.
    if (ui.expanded && !ui.compact) void renderAnalyticsNow();
  });

  // Header tabs = the compact/analytics display switch (was the ⊟/⊞ button).
  $("tab-limits").addEventListener("click", () => setCompact(true));
  $("tab-analytics").addEventListener("click", () => setCompact(false));

  // Limits list re-login affordance (階段 B removed the detail drill-down; the
  // affordance now lives inline on the failed row).
  $("cards").addEventListener("click", (e) => {
    const el = e.target as HTMLElement;

    // Usage-tab quota digest: expand/collapse the full limits list. Height
    // changes, so re-measure the window after re-rendering.
    if (el.closest("[data-quota-toggle]")) {
      ui.quotaExpanded = !ui.quotaExpanded;
      renderCards();
      fitWindow();
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
    if (t.hasAttribute("data-range")) ui.range = t.getAttribute("data-range") as "today" | "week" | "month";
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
  setLocale(resolveLocale(settings.locale));
  applyStaticI18n();
  // 階段 D: restore the last share style/range, clamping any junk to a default.
  const STYLES: ShareStyle[] = ["statement", "diagnostics", "minimal", "fuel", "island_card", "wa"];
  ui.shareStyle = STYLES.includes(settings.share_style as ShareStyle)
    ? (settings.share_style as ShareStyle)
    : "statement";
  ui.shareRange = (["today", "week", "month"] as const).includes(
    settings.share_range as "today" | "week" | "month",
  )
    ? (settings.share_range as "today" | "week" | "month")
    : "week";
  ui.compact = settings.compact;
  document.body.classList.toggle("compact", ui.compact);
  renderTabs();
  fitWindow(); // collapsed width depends on the display filter

  lastSnap = await getSnapshot();
  renderIslandNow();

  await onSnapshot((s) => {
    lastSnap = s;
    $("refresh").classList.remove("busy");
    renderIslandNow();
    if (ui.expanded) {
      renderCards();
      renderRefresh();
    }
  });

  // Tick once a second from the cached snapshot. Never resizes the window —
  // heights are locked per display mode.
  //
  // The "refresh in Ns" countdown is a cheap targeted textContent update and
  // runs every second. The heavy rebuilds (island + the whole Limits panel via
  // innerHTML) only run when their *visible* output would actually change: a
  // new snapshot, a UI-state change, or a minute rolling over (reset countdowns
  // are minute-granular). Rebuilding the entire DOM every second restarted the
  // gauge CSS transitions and re-laid-out the editorial type each frame, which
  // read as the whole app being laggy.
  let lastRenderSig = "";
  setInterval(() => {
    if (ui.expanded) renderRefresh();
    // Reset countdowns are minute-granular (fmtDur → "3h 12m") EXCEPT the final
    // minute, which renders as "Ns" and must tick every second. Drop the time
    // bucket to per-second only while a reset is that close (relative mode);
    // otherwise a per-minute bucket keeps the heavy rebuild rare.
    const now = nowSecs();
    const relative = (settings?.reset_display ?? "relative") !== "clock";
    const imminentReset =
      relative &&
      (lastSnap?.limits ?? []).some((l) => {
        const d = l.resets_at - now;
        return d > 0 && d < 60;
      });
    const timeBucket = imminentReset ? now : Math.floor(now / 60);
    const sig = JSON.stringify([
      lastSnap?.updated_at ?? 0,
      ui.expanded,
      ui.compact,
      ui.relogin,
      ui.copied,
      ui.quotaExpanded,
      !$("settings").hasAttribute("hidden"),
      timeBucket,
      todayRate,
      todayCost,
      settings,
    ]);
    if (sig === lastRenderSig) return;
    lastRenderSig = sig;
    renderIslandNow();
    if (ui.expanded) renderCards();
  }, 1000);

  // Today's burn rate + est. cost for the island aux readout (60s cache). On a
  // fetch failure both go null so the aux shows nothing rather than a fake 0.
  const refreshToday = async () => {
    try {
      const a = await getAnalytics("today");
      todayRate = a.tokPerMin;
      todayCost = a.totalCostUsd;
    } catch {
      todayRate = null;
      todayCost = null;
    }
  };
  await refreshToday();
  renderIslandNow();
  setInterval(refreshToday, 60_000);
}

window.addEventListener("DOMContentLoaded", boot);
