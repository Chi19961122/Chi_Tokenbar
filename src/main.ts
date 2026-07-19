import "./fonts.css";
import "./styles.css";
import type { Analytics, Limit, Snapshot } from "./types";
import type { ReloginState } from "./panel";
import { MANUAL_LOGIN_CMD } from "./panel";
import type { AnalyticsOpts, Granularity, Group, Metric } from "./analytics";
import type { Account, Settings } from "./types";
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
import { islandAuxNeedsAnalytics } from "./island-aux";
import { createAnalyticsRequestGate } from "./analytics-request";
import { renderPanel } from "./panel";
import { showIslandMenu } from "./contextmenu";
import { renderAnalytics } from "./analytics";
import { renderSharePanel } from "./share-panel";
import type { ShareStyle, ShareSize } from "./share";
import { fmtTokens, nowSecs } from "./format";
import { getLocale, resolveLocale, setLocale, t } from "./i18n";
import { applyTheme, watchSystemTheme } from "./theme";
import {
  activateSegment,
  readSegMultiValue,
  readSegmentValue,
  segmentHtml,
  segMultiHtml,
  toggleSegMulti,
} from "./settings-controls";
import { ALL_SOURCES, hasSource, islandMode, sourcesKey } from "./sources";
import { analyticsHeight } from "./analytics-height";
import { bootSharePreview, isSharePreviewHash } from "./share-preview";

const $ = (id: string) => document.getElementById(id)!;

/** True while the Share full page (T-914) is the open page. Read from the DOM —
 *  like the settings `hidden` check — so open/close routing has one source of
 *  truth and no separate boolean to keep in sync. */
const shareOpen = () => !$("share").hasAttribute("hidden");

const ui = {
  expanded: false,
  compact: false,
  // T-ui-301: both lenses render together, so there is no sub-tab. The Trends
  // granularity (daily/hourly) took over what the overview↔hourly tabs did;
  // metric/group/range are the remaining slices. All session-only.
  granularity: "daily" as Granularity,
  metric: "tokens" as Metric,
  group: "agent" as Group,
  range: "week" as "today" | "week" | "month",
  // 階段 D 戰報 Share (report subtab). style/range persist to settings; the fuel
  // model/agent sub-toggle and the quota-note override are session-only.
  // shareQuotaNote null → follow the style default (island_card on, else off).
  shareStyle: "statement" as ShareStyle,
  shareRange: "week" as "today" | "week" | "month",
  // T-905 戰報尺寸: "auto" (1200×675) or "story" (9:16 portrait). Persisted.
  shareSize: "auto" as ShareSize,
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

// Analytics payloads, cached per data *slice* (range|provider-filter) so range
// hopping repaints from what's on hand instead of dead-waiting a log scan
// (single-entry meant today↔week↔month evicted each other). Each entry keeps
// its full fetch key; an entry whose trailing snapshot generation is behind is
// stale — still painted immediately, then revalidated behind itself. subtab/
// metric/group are deliberately absent from keys: they re-slice the same data.
const analyticsCache = new Map<string, { key: string; data: Analytics }>();
/** Fetch keys in flight — a duplicate request for the same key is folded. */
const analyticsInflight = new Set<string>();
/** Generation gate: only the newest request may commit cache / paint. */
const analyticsGate = createAnalyticsRequestGate();

// ── rendering ────────────────────────────────────────────────────────

/** The global range control (Today|Week|Month) that sits above both lenses —
 *  the only persistent top toggle now. Granularity/metric/group moved inline into
 *  each lens header (rendered by renderAnalytics), because range is the one input
 *  that re-slices the payload for BOTH lenses, while the others just re-slice the
 *  already-fetched data. */
function renderToggles() {
  $("toggles").innerHTML = `
    <div class="seg" data-seg="range">
      <button data-range="today" class="${ui.range === "today" ? "on" : ""}">${t("toggle.today")}</button>
      <button data-range="week" class="${ui.range === "week" ? "on" : ""}">${t("toggle.week")}</button>
      <button data-range="month" class="${ui.range === "month" ? "on" : ""}">${t("toggle.month")}</button>
    </div>`;
}

function renderIslandNow() {
  // Island layout derives from the selected quota providers (T-916). "none"
  // (neither selected) renders like an empty pill — the limits are already empty
  // — so it maps to any single mode; "both" is harmless there.
  const mode = islandMode(settings?.sources);
  renderIsland($("island"), lastSnap, {
    mode: mode === "none" ? "both" : mode,
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

/** Frontend mirror of the backend's provider display filter: the backend only
 *  applies it on the next scheduler round, so the cached snapshot can carry
 *  the deselected provider for minutes — filter at render so a settings
 *  change shows immediately (使用者回饋 2026-07-18). */
function visibleLimits(): Limit[] {
  const src = settings?.sources ?? ALL_SOURCES;
  const limits = lastSnap?.limits ?? [];
  // Gate each limit on its own source: Claude/Codex quota + Grok's context-fill
  // limit (T-917). Grok limits reach the panel/digest but never the island (the
  // island renderer filters to the two quota providers on its own).
  return limits.filter((l) =>
    l.provider === "anthropic"
      ? hasSource(src, "claude")
      : l.provider === "codex"
        ? hasSource(src, "codex")
        : hasSource(src, "grok"),
  );
}

function renderCards() {
  // The Usage tab leads with a one-line quota digest (階段 C); the full list
  // shows only in Limits (compact). Settings is now a full page swap that hides
  // the list entirely (T-902), so it no longer forces the full variant.
  const variant: "full" | "summary" = ui.compact ? "full" : "summary";
  renderPanel($("cards"), lastSnap && { ...lastSnap, limits: visibleLimits() }, {
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

type AnalyticsRange = "today" | "week" | "month";

/** Full fetch key for one range: the inputs that change the payload. The source
 *  slice is the sorted selection (T-916) — replaces the old providers segment. */
function analyticsKeyFor(range: AnalyticsRange): string {
  return `${range}|${sourcesKey(settings?.sources)}|${lastSnap?.updated_at ?? 0}`;
}

/** The range|filter prefix of an analytics key — what selects the data *slice*.
 *  The trailing snapshot generation only dates it: a slice-equal, older payload
 *  is still the right chart, just slightly stale. */
function analyticsSliceOf(key: string): string {
  return key.split("|").slice(0, 2).join("|");
}

/** The range the visible pane reads from: the Share full page (T-914) has its
 *  own range (ui.shareRange); the analytics sub-tabs use ui.range. */
function currentAnalyticsRange(): AnalyticsRange {
  return shareOpen() ? ui.shareRange : ui.range;
}

/** Paint the analytics layer from an already-fetched payload (no IPC). */
function renderAnalyticsInto(a: Analytics): void {
  $("rate").textContent = `${fmtTokens(a.tokPerMin)} ${t("analytics.tokPerMin")}`;
  const opts: AnalyticsOpts = { metric: ui.metric, group: ui.group, granularity: ui.granularity };
  const box = $("analytics");
  // The innerHTML swap resets the fixed-height box's scroll; a background
  // revalidate repaint must not yank the reader back to the top.
  const scroll = box.scrollTop;
  renderAnalytics(box, a, opts);
  box.scrollTop = scroll;
}

/** Glass placeholder sized to the mode-locked #analytics box, shown while the
 *  first get_analytics for a key is in flight so the window measures its final
 *  height in one fitWindow() and never jumps a second time. */
function showAnalyticsSkeleton(): void {
  $("analytics").innerHTML =
    `<div class="analytics-skeleton"><div class="tiles">` +
    `<div class="tile sk"></div>`.repeat(4) +
    `</div><div class="chart-wrap"><div class="sk sk-chart"></div></div></div>`;
}

// ── 階段 D 分享 Share (T-914: header full page, was the "report" sub-tab) ──
// The share page uses its own range (ui.shareRange) but shares the sliced
// analytics cache above — same payload, same staleness rules.

function persistShare(): void {
  if (!settings) return;
  settings.share_style = ui.shareStyle;
  settings.share_range = ui.shareRange;
  settings.share_size = ui.shareSize;
  void setSettings(settings);
}

/** Paint the share panel from an already-fetched payload (no IPC). Mounts into
 *  the #share full-page container (T-914), not the analytics box. */
function paintShare(a: Analytics): void {
  $("rate").textContent = `${fmtTokens(a.tokPerMin)} ${t("analytics.tokPerMin")}`;
  const style = ui.shareStyle;
  const quotaNote = ui.shareQuotaNote ?? style === "island_card";
  renderSharePanel($("share"), {
    analytics: a,
    limits: visibleLimits(),
    locale: getLocale(),
    style,
    range: ui.shareRange,
    size: ui.shareSize,
    fuelGroup: ui.shareFuelGroup,
    quotaNote,
    setStyle: (s) => {
      ui.shareStyle = s;
      ui.shareQuotaNote = null; // reset override so the new style's default applies
      persistShare();
      paintShare(a);
    },
    setRange: (r) => {
      ui.shareRange = r;
      persistShare();
      void renderAnalyticsNow();
    },
    setSize: (s) => {
      ui.shareSize = s;
      persistShare();
      paintShare(a);
    },
    setFuelGroup: (g) => {
      ui.shareFuelGroup = g;
      paintShare(a);
    },
    setQuotaNote: (on) => {
      ui.shareQuotaNote = on;
      paintShare(a);
    },
  });
}

/** Repaint the visible pane (usage charts or report) from the cache, iff the
 *  slice that just landed is the one on screen. The single landing point for
 *  every fetch — user-initiated, warming, or the island's 60s today refresh —
 *  so a fetch that got folded as a duplicate still paints when its twin lands. */
function paintIfShowing(slice: string): void {
  if (!ui.expanded || ui.compact) return;
  if (analyticsSliceOf(analyticsKeyFor(currentAnalyticsRange())) !== slice) return;
  const entry = analyticsCache.get(slice);
  if (!entry) return;
  if (shareOpen()) paintShare(entry.data);
  else renderAnalyticsInto(entry.data);
}

/** Fetch one range into the cache and repaint whatever pane shows its slice.
 *  Folded (→ null) when that exact key is already in flight.
 *  Superseded/cancelled backend outcomes return null without cache/paint.
 *  Operational errors preserve the last valid view (do not write mock data). */
async function fetchAnalytics(range: AnalyticsRange): Promise<Analytics | null> {
  const key = analyticsKeyFor(range);
  // Gate + inflight use the *stable slice* (range|sources), not key-with-
  // updated_at — otherwise every snapshot tick opens a new generation and an
  // older IPC response can overwrite a newer one.
  const slice = analyticsSliceOf(key);
  if (analyticsInflight.has(slice)) return null;
  analyticsInflight.add(slice);
  const gen = analyticsGate.begin(slice);
  try {
    const a = await getAnalytics(range);
    if (a == null) return null; // superseded / cancelled
    if (analyticsGate.decide(slice, gen) !== "commit") return null;
    analyticsCache.set(slice, { key, data: a });
    paintIfShowing(slice);
    return a;
  } catch {
    // Operational failure: leave cache and UI as-is.
    return null;
  } finally {
    analyticsInflight.delete(slice);
  }
}

/** Paint the analytics layer, fetching if needed — synchronously before its
 *  first await, so callers can fitWindow() right after without waiting on IPC:
 *    exact cache hit → paint (no fetch); stale slice hit → paint the dated
 *    payload now, revalidate behind it; cold miss → skeleton until the fetch
 *    lands (paintIfShowing then draws whichever pane is current).
 *
 *  Stage 1A: does *not* background-warm other ranges. Warming today+week+month
 *  forced three full log scans (~hundreds of MB cumulative I/O). Other ranges
 *  load on first visit. */
async function renderAnalyticsNow(): Promise<void> {
  const range = currentAnalyticsRange();
  const key = analyticsKeyFor(range);
  const entry = analyticsCache.get(analyticsSliceOf(key));
  if (!entry) {
    if (!shareOpen()) showAnalyticsSkeleton();
  } else if (shareOpen()) paintShare(entry.data);
  else renderAnalyticsInto(entry.data);
  if (!entry || entry.key !== key) await fetchAnalytics(range);
}

/** Non-blocking entry used on mode entry (expand / compact toggle). */
function beginAnalytics(): void {
  void renderAnalyticsNow();
}

// ── window sizing (locked per display mode, bottom-right anchored) ───
// The window is resized ONLY when a mode is entered (expand, compact toggle,
// settings open/close) — never on subtab clicks or the 1s tick, so page
// switches stay jank-free. #analytics gets a screen-sized fixed height once at
// each Usage-mode entry for the same reason: every subtab uses the same box.

/** Natural panel height at mode entry: children sum. */
/** Desired analytics-box height, used ONLY for window sizing. The box itself
 *  flex-fills the panel (T-908), so any gap between this number and the real
 *  window height — DPI rounding, the OS clamping the resize, content growing
 *  after entry (quota list expand) — is absorbed by the box instead of pushing
 *  its bottom edge out of the clipped panel (拆分頁「其他專案」被擋 bug). */
let analyticsDesiredH = 300;

function contentHeight(): number {
  let h = 14; // panel top margin (6) + border (2) + breathing room
  for (const el of $("panel").children) {
    const child = el as HTMLElement;
    // The analytics box is flex-filled, so its rendered height tracks the
    // CURRENT window — measuring it would echo the old size back. Use the
    // desired height instead (0 while hidden: compact / settings page).
    if (child.id === "analytics") {
      if (getComputedStyle(child).display !== "none") h += analyticsDesiredH;
    } else {
      h += child.offsetHeight;
    }
  }
  return Math.max(h, 120);
}

/** Size the shared analytics box once per Usage-mode entry. The previous
 *  desired height is subtracted back out of the measurement so it cannot feed
 *  into the next calculation. Subtab clicks and ticks never call it. */
function sizeAnalytics(): void {
  const box = $("analytics");
  const visible = getComputedStyle(box).display !== "none";
  const otherPanelHeight = Math.max(0, contentHeight() - (visible ? analyticsDesiredH : 0));
  analyticsDesiredH = analyticsHeight(window.screen?.availHeight ?? Number.NaN, otherPanelHeight);
}

/** Collapsed island width depends on layout (dual providers need more room).
 *  Mirrors island.ts's branching: only an exact claude/codex renders one
 *  group, so only those get the narrow width — an unknown value shows both.
 *  "both" now stacks the two providers vertically (2026-07-18 試行), so it is
 *  the narrow-but-tall case and the single-provider pill keeps the flat one. */
function collapsedSize(): { w: number; h: number } {
  // "both" stacks the two providers (tall+narrow); a single provider — or the
  // empty "none" state — is the flat pill. Derived from the source selection.
  const mode = islandMode(settings?.sources);
  if (mode === "both") return { w: 250, h: 72 };
  return { w: 270, h: SIZE.collapsed.h };
}

/** Resize the OS window for the current mode (no-op in browser). */
function fitWindow() {
  const { w, h } = ui.expanded
    ? { w: SIZE.expanded.w, h: contentHeight() }
    : collapsedSize();
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
  $("share-btn").title = t("header.shareTitle");
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
      renderToggles();
      beginAnalytics();
      sizeAnalytics();
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

/** Accounts to show on the Settings page (moved here from the old analytics
 *  "stats" sub-tab, T-ui-301 §3). Lowest-coupling source: the already-fetched
 *  analytics payload — accounts are range-invariant, so any cached slice serves.
 *  On a cold Settings open (nothing warmed yet) fall back to one fetch. */
function cachedAccounts(): Account[] {
  for (const entry of analyticsCache.values()) {
    if (entry.data.accounts?.length) return entry.data.accounts;
  }
  return [];
}
async function accountsForSettings(): Promise<Account[]> {
  const cached = cachedAccounts();
  if (cached.length) return cached;
  const a = await fetchAnalytics(currentAnalyticsRange());
  return a?.accounts ?? cachedAccounts();
}

async function renderSettings() {
  const s = await getSettings();
  const accounts = await accountsForSettings();
  const accountsGroup = accounts.length
    ? `<div class="sgroup">
      <div class="lsec-head">${t("settings.accounts")}</div>
      ${accounts
        .map(
          (ac) =>
            `<div class="srow"><span class="slabel">${escAttr(ac.client)}<span class="snote">${escAttr(
              ac.account,
            )} · ${escAttr(ac.plan)}</span></span></div>`,
        )
        .join("")}
    </div>`
    : "";
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
      <div class="srow srow-seg">
        <span class="slabel">${t("settings.language")}</span>
        ${segmentHtml("s-locale", s.locale === "en" || s.locale === "zh-TW" ? s.locale : "system", [
          ["system", t("settings.localeSystem")],
          ["zh-TW", "中文"],
          ["en", "English"],
        ])}
      </div>
      <div class="srow srow-seg">
        <span class="slabel">${t("settings.theme")}</span>
        ${segmentHtml("s-theme", s.theme === "light" || s.theme === "dark" ? s.theme : "system", [
          ["system", t("settings.themeSystem")],
          ["light", t("settings.themeLight")],
          ["dark", t("settings.themeDark")],
        ])}
      </div>
      <div class="srow srow-seg">
        <span class="slabel">${t("settings.sources")}<span class="snote">${t("settings.sourcesNote")}</span></span>
        ${segMultiHtml("s-sources", s.sources ?? [], [
          ["claude", "Claude"],
          ["codex", "Codex"],
          ["grok", "Grok"],
        ])}
      </div>
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
      <div class="srow srow-seg">
        <span class="slabel">${t("settings.expandDefault")}</span>
        ${segmentHtml("s-expand", s.expand_default === "usage" ? "usage" : "compact", [
          ["compact", t("settings.expandCompact")],
          ["usage", t("settings.expandUsage")],
        ])}
      </div>
      <label class="srow">
        <span class="slabel">${t("settings.pinClaude")}</span>
        <select id="s-pin-claude">${pinOptionsHtml("anthropic", s.island_pin_claude)}</select>
      </label>
      <label class="srow">
        <span class="slabel">${t("settings.pinCodex")}</span>
        <select id="s-pin-codex">${pinOptionsHtml("codex", s.island_pin_codex)}</select>
      </label>
      <div class="srow srow-seg">
        <span class="slabel">${t("settings.islandAux")}</span>
        ${segmentHtml(
          "s-aux",
          s.island_aux === "off" || s.island_aux === "cost_today" ? s.island_aux : "tok_per_min",
          [
            ["off", t("settings.auxOff")],
            ["tok_per_min", t("settings.auxTokPerMin")],
            ["cost_today", t("settings.auxCostToday")],
          ],
        )}
      </div>
      <div class="srow srow-seg">
        <span class="slabel">${t("settings.resetDisplay")}</span>
        ${segmentHtml("s-reset", s.reset_display === "clock" ? "clock" : "relative", [
          ["relative", t("settings.resetRelative")],
          ["clock", t("settings.resetClock")],
        ])}
      </div>
    </div>

    <div class="sgroup">
      <div class="lsec-head">${t("settings.dataSources")}</div>
      <div class="srow srow-seg">
        <span class="slabel">${t("settings.refreshInterval")}<span class="snote">${t("settings.refreshIntervalNote")}</span></span>
        ${segmentHtml("s-refresh-secs", [30, 60, 180].includes(s.refresh_secs) ? String(s.refresh_secs) : "180", [
          ["30", t("settings.refreshInterval30")],
          ["60", t("settings.refreshInterval1m")],
          ["180", t("settings.refreshInterval3m")],
        ])}
      </div>
      <div class="srow srow-seg">
        <span class="slabel">${t("settings.claudeRefresh")}<span class="warn">${t("settings.claudeRefreshWarn")}</span></span>
        ${segmentHtml("s-refresh", s.allow_token_refresh ? "on" : "off", [
          ["off", t("settings.refreshOff")],
          ["on", t("settings.refreshOn")],
        ])}
      </div>
      <div class="srow srow-seg">
        <span class="slabel">${t("settings.codexSource")}<span class="snote">${t("settings.codexSourceNote")}</span></span>
        ${segmentHtml(
          "s-codex-source",
          s.codex_usage_source === "live" || s.codex_usage_source === "auto" ? s.codex_usage_source : "local",
          [
            ["live", t("settings.codexLive")],
            ["auto", t("settings.codexAuto")],
            ["local", t("settings.codexLocal")],
          ],
        )}
      </div>
    </div>
    ${accountsGroup}`;
}

function readSettingsForm(): Settings {
  const v = (id: string) => $(id) as HTMLInputElement;
  const segVal = (id: string) => readSegmentValue($("settings"), id, "");
  // Merge the form fields over the cached settings so fields with no form
  // control (階段 D share_style / share_range, and any future non-form setting)
  // are preserved rather than silently dropped on an unrelated settings change.
  return {
    ...(settings ?? ({} as Settings)),
    autostart: v("s-autostart").checked,
    always_on_top: v("s-always-on-top").checked,
    allow_token_refresh: segVal("s-refresh") === "on",
    warn_pct: +v("s-warn").value || 75,
    crit_pct: +v("s-crit").value || 90,
    compact: ui.compact,
    // T-916: the multi-select is the source of truth. `providers`/`tool_*` are
    // preserved from the spread above and re-derived by the backend on save for
    // downgrade safety (config.rs `sync_legacy_from_sources`).
    sources: readSegMultiValue($("settings"), "s-sources"),
    codex_usage_source: (segVal("s-codex-source") || "local") as Settings["codex_usage_source"],
    locale: segVal("s-locale") || "system",
    expand_default: (segVal("s-expand") || "compact") as Settings["expand_default"],
    island_pin_claude: ($("s-pin-claude") as HTMLSelectElement).value || "auto",
    island_pin_codex: ($("s-pin-codex") as HTMLSelectElement).value || "auto",
    island_aux: (segVal("s-aux") || "tok_per_min") as Settings["island_aux"],
    reset_display: (segVal("s-reset") || "relative") as Settings["reset_display"],
    theme: (segVal("s-theme") || "system") as Settings["theme"],
    refresh_secs: Number(segVal("s-refresh-secs")) || 180,
  };
}

/** Open the settings panel, expanding the window first if it is collapsed.
 *  Shared by the gear button and the island context menu's "Settings" item. */
async function openSettingsPanel(): Promise<void> {
  const el = $("settings");
  if (!el.hasAttribute("hidden")) return; // already open
  // Mutual exclusion (T-914): Settings and Share are both full pages and must
  // never stack — drop Share's page/class before opening Settings.
  if (shareOpen()) {
    $("share").setAttribute("hidden", "");
    document.body.classList.remove("share-open");
  }
  if (!ui.expanded) setExpanded(true);
  // Render the settings form BEFORE fitWindow measures — it is the only visible
  // content on the settings page, so its natural height is the window height.
  await renderSettings();
  el.removeAttribute("hidden");
  document.body.classList.add("settings-open");
  fitWindow();
}

/** Leave the settings page back to whichever tab (Limits/Usage) is active.
 *  Re-render the about-to-be-visible tab content BEFORE fitWindow measures it —
 *  the hidden→visible children were display:none while settings was open, so
 *  their height must be repainted before the resize (F-06 ordering lesson). */
function closeSettings(): void {
  $("settings").setAttribute("hidden", "");
  document.body.classList.remove("settings-open");
  renderCards();
  if (!ui.compact) {
    renderToggles();
    beginAnalytics();
    sizeAnalytics();
  }
  fitWindow();
}

/** Open the Share full page (T-914) — architecturally identical to
 *  openSettingsPanel: render the share panel BEFORE fitWindow measures (the
 *  share panel is then the only visible content, so its natural height is the
 *  window height — the F-06 render-before-measure lesson). Settings and Share
 *  are mutually exclusive, so close Settings first if it is open. */
async function openSharePanel(): Promise<void> {
  const el = $("share");
  if (!el.hasAttribute("hidden")) return; // already open
  if (!$("settings").hasAttribute("hidden")) {
    $("settings").setAttribute("hidden", "");
    document.body.classList.remove("settings-open");
  }
  if (!ui.expanded) setExpanded(true);
  // Reveal + flag BEFORE painting so shareOpen() reads true inside the render
  // path (currentAnalyticsRange → ui.shareRange, paintIfShowing → paintShare).
  el.removeAttribute("hidden");
  document.body.classList.add("share-open");
  // Paint from cache (exact or stale) synchronously; on a cold miss await the
  // fetch so fitWindow measures a populated panel, not an empty one.
  const range = ui.shareRange;
  const key = analyticsKeyFor(range);
  const entry = analyticsCache.get(analyticsSliceOf(key));
  if (entry) paintShare(entry.data);
  else {
    await fetchAnalytics(range);
    const fresh = analyticsCache.get(analyticsSliceOf(key));
    if (fresh) paintShare(fresh.data);
  }
  fitWindow();
  // Revalidate a stale slice behind the painted one (paintIfShowing → paintShare
  // when it lands, since share-open is already set). Stage 1A: no multi-range warm.
  if (entry && entry.key !== key) void fetchAnalytics(range);
}

/** Leave the Share page back to whichever tab (Limits/Usage) is active — mirror
 *  of closeSettings: re-render the about-to-be-visible tab BEFORE fitWindow. */
function closeShare(): void {
  $("share").setAttribute("hidden", "");
  document.body.classList.remove("share-open");
  renderCards();
  if (!ui.compact) {
    renderToggles();
    beginAnalytics();
    sizeAnalytics();
  }
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
      renderToggles();
      // Non-blocking: paint from cache or drop a fixed-height skeleton, so
      // fitWindow() below measures the final height and resizes exactly once —
      // never waiting on the get_analytics IPC.
      beginAnalytics();
      sizeAnalytics();
    }
  }
  // Size the window immediately (analytics height is locked for this mode, so
  // its content arriving later never changes the measured height).
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

  // Settings is a full page, not an overlay (T-902): opening it swaps the whole
  // panel body out for the settings form (see .settings-open in styles.css);
  // the gear toggles it closed, restoring the previously-active tab. fitWindow()
  // re-measures on each transition, as it already does per mode.
  $("gear").addEventListener("click", async () => {
    if ($("settings").hasAttribute("hidden")) await openSettingsPanel();
    else closeSettings();
  });

  // Share (T-914): a header icon next to the gear opens the share/report panel
  // as its own full page (same page-swap model as settings, mutually exclusive
  // with it). The icon toggles it closed, restoring the previously-active tab.
  $("share-btn").addEventListener("click", async () => {
    if ($("share").hasAttribute("hidden")) await openSharePanel();
    else closeShare();
  });
  const commitSettings = async () => {
    const prevLocale = getLocale();
    settings = readSettingsForm();
    applyTheme(settings.theme); // re-apply before any re-render below
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
  };
  $("settings").addEventListener("change", commitSettings);
  $("settings").addEventListener("click", async (event) => {
    // Single-select segments and the multi-select chip row both commit on click
    // (T-916: chips toggle independently via the .seg-multi variant).
    if (!activateSegment(event.target) && !toggleSegMulti(event.target)) return;
    await commitSettings();
  });

  // Header tabs = the compact/analytics display switch (was the ⊟/⊞ button).
  // While settings is open they double as its exit: users instinctively tap
  // 限額/分析 to leave. Same tab → just close back to it; different tab → drop
  // the page-swap class first so setCompact measures the visible tab, not the
  // hidden one, then switch (which renders the target tab + re-measures).
  const onTab = async (compact: boolean) => {
    const settingsOpen = !$("settings").hasAttribute("hidden");
    const shareOpenNow = shareOpen();
    // Same tab while a full page is open → just close back to it (re-renders +
    // re-measures for the restored tab). Different tab → strip the page class so
    // setCompact measures the visible tab, not the hidden page, then switch.
    if ((settingsOpen || shareOpenNow) && ui.compact === compact) {
      if (settingsOpen) closeSettings();
      else closeShare();
      return;
    }
    if (settingsOpen) {
      $("settings").setAttribute("hidden", "");
      document.body.classList.remove("settings-open");
    }
    if (shareOpenNow) {
      $("share").setAttribute("hidden", "");
      document.body.classList.remove("share-open");
    }
    await setCompact(compact);
  };
  $("tab-limits").addEventListener("click", () => void onTab(true));
  $("tab-analytics").addEventListener("click", () => void onTab(false));

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

  // Range is the global top toggle (re-slices the payload for both lenses).
  $("toggles").addEventListener("click", (e) => {
    const t = (e.target as HTMLElement).closest("[data-range]");
    if (!t) return;
    ui.range = t.getAttribute("data-range") as "today" | "week" | "month";
    renderToggles();
    renderAnalyticsNow();
  });

  // The per-lens toggles live inside #analytics (granularity/metric in Trends,
  // group/metric in Breakdown). None re-slice the payload — they just re-render
  // both lenses from the already-fetched data, so scroll + stale-while-revalidate
  // are preserved by renderAnalyticsInto.
  $("analytics").addEventListener("click", (e) => {
    const t = (e.target as HTMLElement).closest("[data-metric],[data-group],[data-granularity]");
    if (!t) return;
    if (t.hasAttribute("data-metric")) ui.metric = t.getAttribute("data-metric") as Metric;
    if (t.hasAttribute("data-group")) ui.group = t.getAttribute("data-group") as Group;
    if (t.hasAttribute("data-granularity"))
      ui.granularity = t.getAttribute("data-granularity") as Granularity;
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
  if (isSharePreviewHash(location.hash)) {
    await bootSharePreview();
    return;
  }

  wireEvents();
  wireDevBar();
  setupEdgeSnap();

  settings = await getSettings();
  // Apply the theme before the first panel paint so there is no light→dark flash
  // when a dark-preferring user expands the panel. The island pill is opaque and
  // theme-invariant, so the collapsed default needs no earlier hook.
  applyTheme(settings.theme);
  // Re-apply on OS scheme changes, but only while the setting follows the system.
  watchSystemTheme(() => settings?.theme ?? "system");
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
  ui.shareSize = settings.share_size === "story" ? "story" : "auto";
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

  // Today's burn rate + est. cost for the island aux readout (60s cache).
  // Stage 1A: only scan when the island aux actually needs analytics data.
  // `off` must not trigger a full today log scan every minute.
  const refreshToday = async () => {
    if (!islandAuxNeedsAnalytics(settings?.island_aux)) {
      todayRate = null;
      todayCost = null;
      return;
    }
    try {
      // Routed through the shared cache: keeps the "today" slice warm (and any
      // on-screen today charts fresh) as a side effect of the island readout.
      // A folded duplicate fetch (null) falls back to whatever is cached.
      const a =
        (await fetchAnalytics("today")) ??
        analyticsCache.get(analyticsSliceOf(analyticsKeyFor("today")))?.data ??
        null;
      todayRate = a?.tokPerMin ?? null;
      todayCost = a?.totalCostUsd ?? null;
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
