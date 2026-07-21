// Type-safe i18n dictionary + tiny runtime (決策 D1: 雙語回歸).
//
// `en` is the source of truth for the key set; `zhTW` is checked against it with
// `satisfies Record<keyof typeof en, string>`, so a missing OR an extra key is a
// compile error — the two dictionaries can never drift apart.
//
// The island's short quota labels (5h / wk / model short-names) are deliberately
// NOT in here — they stay fixed English (D1).

export type Locale = "en" | "zh-TW";

// ── English (source of truth for the key set) ────────────────────────
const en = {
  // Header tabs (static in index.html, set via applyStaticI18n)
  "tab.limits": "Limits",
  "tab.usage": "Usage",
  "header.refreshTitle": "Refresh",
  "header.settingsTitle": "Settings",
  "header.shareTitle": "Share",
  "header.collapseTitle": "Collapse",
  "header.refreshIn": "Refresh in {v}",

  // Settings form
  "settings.startupWindow": "Startup & Window",
  "settings.launchAtStartup": "Launch at startup",
  "settings.alwaysOnTop": "Always on top",
  "settings.alwaysOnTopNote": "When off, other windows can cover it. Restore from the tray.",
  "settings.displayNotifications": "Display & Notifications",
  "settings.language": "Language",
  "settings.localeSystem": "Follow system",
  "settings.providers": "Providers",
  "settings.providersBoth": "Both",
  "settings.providersClaude": "Claude only",
  "settings.providersCodex": "Codex only",
  "settings.sources": "Sources",
  "settings.sourcesNote": "Which tools to track. Quota for Claude/Codex; context fill for Grok.",
  "settings.notifyAt": "Notify at",
  "settings.notifyNote": "Sends a system notification when usage crosses the threshold.",
  "settings.warn": "warn",
  "settings.crit": "crit",
  "settings.dataSources": "Data Sources",
  "settings.claudeRefresh": "Claude token refresh",
  "settings.claudeRefreshWarn": "May affect Claude Code login.",
  "settings.refreshOff": "Off (estimates)",
  "settings.refreshOn": "On (auto-renew)",
  "settings.codexSource": "Codex usage source",
  "settings.codexSourceNote": "Live and Auto run read-only queries on the signed-in account.",
  "settings.codexLive": "Live",
  "settings.codexAuto": "Auto (live first)",
  "settings.codexLocal": "Local session snapshot",
  "settings.refreshInterval": "Refresh interval",
  "settings.refreshIntervalNote": "Faster polling may hit rate limits; backs off automatically.",
  "settings.refreshInterval30": "30s",
  "settings.refreshInterval1m": "1 min",
  "settings.refreshInterval3m": "3 min",

  // Settings — 階段 B rows (island / expand / reset display)
  "settings.island": "Island",
  "settings.expandDefault": "Tap island opens",
  "settings.expandCompact": "Limits",
  "settings.expandUsage": "Usage",
  "settings.pinClaude": "Island · Claude",
  "settings.pinCodex": "Island · Codex",
  "settings.pinAuto": "Auto",
  "settings.pin5h": "5h",
  "settings.pinWeek": "Week",
  "settings.islandAux": "Island aux",
  "settings.auxOff": "Off",
  "settings.auxTokPerMin": "tok/min",
  "settings.auxCostToday": "Today's cost",
  "settings.resetDisplay": "Reset time",
  "settings.resetRelative": "Countdown",
  "settings.resetClock": "Clock",
  "settings.theme": "Theme",
  "settings.themeSystem": "Follow system",
  "settings.themeLight": "Light",
  "settings.themeDark": "Dark",
  "settings.accounts": "Accounts",

  // Analytics lens captions (T-ui-301: two stacked lenses, no sub-tab switcher)
  "subtab.trends": "Trends",
  "subtab.breakdown": "Breakdown",

  // Analytics toggles
  "toggle.today": "Today",
  "toggle.week": "Week",
  "toggle.month": "Month",
  "toggle.tokens": "Tokens",
  "toggle.price": "Cost",
  "toggle.model": "By model",
  "toggle.agent": "By agent",
  "toggle.daily": "Daily",
  "toggle.hourly": "Hourly",

  // Analytics content
  "analytics.tokens": "tokens",
  "analytics.estCost": "est. cost",
  "analytics.peak": "peak",
  // T-ui-301 two-lens copy
  "analytics.trendsEyebrow": "This period, total",
  "analytics.trendsKick": "When the work happened — by day, and by hour.",
  "analytics.trendsSub": "Est. {cost} · {days}d streak",
  "analytics.trendsChartLabel": "Usage over time",
  "analytics.footPeak": "Peak day {date}",
  "analytics.footBusiest": "Busiest hour {hour}",
  "analytics.breakdownKick": "Where it went — which models, tools, and projects.",
  "analytics.leadingModel": "Leading model",
  "analytics.leadingAgent": "Leading agent",
  "analytics.breakdownSub": "{value} · {pct}% this period",
  "analytics.compositionTitle": "Token composition",
  "analytics.activeDays": "active days",
  "analytics.maxDay": "Max day",
  "analytics.maxHour": "Max hour",
  "analytics.streak": "days streak",
  "analytics.prNow": "PR NOW",
  "analytics.tokPerMin": "tok/min",
  "analytics.since": "from {date}",
  "analytics.sessionsThisWeek": "sessions this week",
  "analytics.input": "input",
  "analytics.cached": "cached",
  "analytics.output": "output",
  "analytics.reasoning": "reasoning",

  // Analytics — 階段 C+ advanced dimensions
  "analytics.heatmapTitle": "Daily activity",
  "analytics.activityTitle": "Activity type",
  "analytics.projectsTitle": "Projects",
  "analytics.projectsOther": "Other projects",
  "analytics.kindEdit": "Edit",
  "analytics.kindRead": "Read",
  "analytics.kindSearch": "Search",
  "analytics.kindRun": "Run",
  "analytics.kindWeb": "Web",
  "analytics.kindAgent": "Agent",
  "analytics.kindMcp": "MCP",
  "analytics.kindOther": "Other",
  "analytics.less": "less",
  "analytics.more": "more",

  // Limit display names
  "limit.cc5h": "5h session",
  "limit.ccWeek": "Weekly · all models",
  "limit.ccOpus": "Weekly · Opus",
  "limit.ccExtra": "Extra usage",
  "limit.codex5h": "5h window",
  "limit.codexWeek": "Weekly window",
  "limit.codexCredits": "Credits",
  "limit.grokCtx": "Context window",
  "limit.weeklyModel": "Weekly · {name}",

  // Badges
  "badge.unavailable": "Unavailable",
  "badge.estimate": "Estimate",
  "badge.stale": "Stale",

  // Row note lines — reset time only (pace copy removed in 階段 B)
  "note.locked": "Locked",
  "note.lockedResetsIn": "Locked · resets in {d}",
  "note.lockedResets": "Locked · resets {r}",
  "note.resets": "Resets {r}",
  "note.resetsIn": "Resets in {d}",
  // Grok context-fill note — no reset schedule; empties on a new session (T-917).
  "note.grokSession": "This conversation's memory; a new chat starts at 0%",
  // Historical-pace runway line (T-feat-007) — shown only at ≥2 complete cycles.
  "note.histRunway": "~empty in {d}",
  "note.histBadge": "hist",
  "note.histTooltip": "Estimated from your past cycles' median usage curve (≥2 cycles).",

  // Island context menu (D4)
  "menu.pinClaude": "Pin Claude",
  "menu.pinCodex": "Pin Codex",
  "menu.provider": "Provider",
  "menu.settings": "Settings",
  "menu.hide": "Hide island",

  // Re-login affordance
  "relogin.cantLaunch": "Atoll can't launch claude. Run this in your terminal:",
  "relogin.copy": "Copy",
  "relogin.copied": "Copied",
  "relogin.ok": "Login window opened. Refresh with ⟳ above when done.",
  "relogin.opening": "Opening…",
  "relogin.button": "Re-login to Claude",

  // Limits list
  "list.noTools": "No tools running",

  // Island
  "island.hideAria": "Hide to tray",
  "island.hideTitle": "Hide to tray (restore from the tray icon)",

  // 階段 D 戰報 Share — range words used inside periodLabel (dates formatted
  // in share.ts) and card labels. "TokenBar" and wa's 量/CUMULATIVE LEDGER
  // stay untranslated by design.
  "share.periodToday": "Today",
  "share.periodWeek": "This week",
  "share.periodMonth": "Last 30 days",
  "share.now": "Now",
  "share.left": "left",
  "share.totalTokens": "Total Tokens",
  "share.estCost": "Est. Cost",
  "share.estUsd": "est. usd",
  "share.est": "est.",
  "share.cumulativeUsage": "Cumulative usage",
  "share.usageStatement": "Usage Statement",
  "share.cumulativeForPeriod": "Cumulative for period",
  "share.tokensAcrossAgents": "tokens across {n} agents",
  "share.streakDays": "{n}d streak",
  "share.peakTokens": "peak {tokens}",
  "share.tokens": "tokens",
  "share.agent": "Agent",
  "share.model": "Model",
  "share.agents": "AGENTS",
  "share.models": "MODELS",
  "share.tokensShare": "Tokens / Share",
  "share.share": "share",
  "share.pumpTotal": "PUMP TOTAL",
  "share.usageReport": "Usage report",
  "share.generatedBy": "Generated by Atoll",
  "share.generated": "Generated",
  "share.acrossAgents": "across {n} agents",
  "share.sessions": "{n} sessions",
  "share.sessionsLabel": "sessions",
  "share.streakInline": "streak {n}d",
  "share.peakPerDay": "peak {tokens}/day",
  "share.peakAt": "peak {hour}",
  "share.week": "week",
  "share.used": "used",
  "share.quotaUsed": "Quota used",
  "share.thisCycle": "this cycle",
  "share.lagoonDepth": "lagoon depth",
  "share.quotaUsedCycle": "Quota used · this cycle",
  "share.fuelDispensed": "FUEL DISPENSED",
  "share.totalSale": "TOTAL SALE",
  "share.cumulativeLedger": "Cumulative Ledger",
  "share.shareReport": "Share Report",
  "share.copyFailed": "Copy failed",
  "share.saved": "Saved to {path}",
  "share.savedShort": "Saved",
  "share.exportPng": "Save PNG",
  "share.copyImage": "Copy",
  "share.style": "Style",
  "share.range": "Range",
  "share.quotaNote": "Quota line",
  "share.previewTitle": "Open large preview",
  "share.previewHint": "Esc / click to close",
  "share.previewGenerating": "Rendering\u2026",
  "share.previewFailed": "Preview failed",
} as const;

// ── 繁體中文 (checked exhaustively against `en`) ───────────────────────
const zhTW = {
  "tab.limits": "限額",
  "tab.usage": "分析",
  "header.refreshTitle": "重新整理",
  "header.settingsTitle": "設定",
  "header.shareTitle": "分享",
  "header.collapseTitle": "收合",
  "header.refreshIn": "{v} 後更新",

  "settings.startupWindow": "啟動與視窗",
  "settings.launchAtStartup": "開機時啟動",
  "settings.alwaysOnTop": "永遠置頂",
  "settings.alwaysOnTopNote": "關閉時其他視窗可蓋住它,可從系統匣還原。",
  "settings.displayNotifications": "顯示與通知",
  "settings.language": "語言",
  "settings.localeSystem": "跟隨系統",
  "settings.providers": "供應商",
  "settings.providersBoth": "兩者",
  "settings.providersClaude": "僅 Claude",
  "settings.providersCodex": "僅 Codex",
  "settings.sources": "供應商",
  "settings.sourcesNote": "要追蹤哪些工具。Claude/Codex 顯示額度,Grok 顯示 context 填充。",
  "settings.notifyAt": "通知門檻",
  "settings.notifyNote": "用量超過門檻時發送系統通知。",
  "settings.warn": "警告",
  "settings.crit": "嚴重",
  "settings.dataSources": "資料來源",
  "settings.claudeRefresh": "Claude token 更新",
  "settings.claudeRefreshWarn": "可能影響 Claude Code 登入。",
  "settings.refreshOff": "關閉(估算)",
  "settings.refreshOn": "開啟(自動更新)",
  "settings.codexSource": "Codex 用量來源",
  "settings.codexSourceNote": "Live 與 Auto 會對已登入帳號執行唯讀查詢。",
  "settings.codexLive": "即時",
  "settings.codexAuto": "自動(優先即時)",
  "settings.codexLocal": "本機工作階段快照",
  "settings.refreshInterval": "更新頻率",
  "settings.refreshIntervalNote": "更快=更容易被限流,遇 429 會自動放慢。",
  "settings.refreshInterval30": "30秒",
  "settings.refreshInterval1m": "1 分",
  "settings.refreshInterval3m": "3 分",

  "settings.island": "島嶼",
  "settings.expandDefault": "點島嶼展開到",
  "settings.expandCompact": "限額精簡",
  "settings.expandUsage": "分析",
  "settings.pinClaude": "島嶼 · Claude",
  "settings.pinCodex": "島嶼 · Codex",
  "settings.pinAuto": "自動",
  "settings.pin5h": "5 小時",
  "settings.pinWeek": "週",
  "settings.islandAux": "島嶼副指標",
  "settings.auxOff": "關",
  "settings.auxTokPerMin": "tok/分",
  "settings.auxCostToday": "今日成本",
  "settings.resetDisplay": "重置時間顯示",
  "settings.resetRelative": "倒數",
  "settings.resetClock": "時刻",
  "settings.theme": "主題",
  "settings.themeSystem": "跟隨系統",
  "settings.themeLight": "亮色",
  "settings.themeDark": "暗色",
  "settings.accounts": "帳號",

  "subtab.trends": "趨勢",
  "subtab.breakdown": "拆分",

  "toggle.today": "今日",
  "toggle.week": "本週",
  "toggle.month": "本月",
  "toggle.tokens": "Token",
  "toggle.price": "花費",
  "toggle.model": "依模型",
  "toggle.agent": "依工具",
  "toggle.daily": "每日",
  "toggle.hourly": "每時",

  "analytics.tokens": "token",
  "analytics.estCost": "估算成本",
  "analytics.peak": "尖峰",
  "analytics.trendsEyebrow": "本期總計",
  "analytics.trendsKick": "用量發生在何時 —— 逐日、逐時。",
  "analytics.trendsSub": "估算 {cost} · 連續 {days} 天",
  "analytics.trendsChartLabel": "用量趨勢",
  "analytics.footPeak": "尖峰日 {date}",
  "analytics.footBusiest": "尖峰時段 {hour}",
  "analytics.breakdownKick": "用量流向何處 —— 模型、工具與專案。",
  "analytics.leadingModel": "主力模型",
  "analytics.leadingAgent": "主力工具",
  "analytics.breakdownSub": "{value} · 本期 {pct}%",
  "analytics.compositionTitle": "Token 組成",
  "analytics.activeDays": "使用天數",
  "analytics.maxDay": "單日最高",
  "analytics.maxHour": "單時最高",
  "analytics.streak": "天連勝",
  "analytics.prNow": "目前新紀錄",
  "analytics.tokPerMin": "tok/分",
  "analytics.since": "自 {date} 起",
  "analytics.sessionsThisWeek": "本週工作階段",
  "analytics.input": "輸入",
  "analytics.cached": "快取",
  "analytics.output": "輸出",
  "analytics.reasoning": "推理",

  "analytics.heatmapTitle": "每日活動",
  "analytics.activityTitle": "活動類型",
  "analytics.projectsTitle": "專案",
  "analytics.projectsOther": "其他專案",
  "analytics.kindEdit": "編輯",
  "analytics.kindRead": "讀取",
  "analytics.kindSearch": "搜尋",
  "analytics.kindRun": "執行",
  "analytics.kindWeb": "網路",
  "analytics.kindAgent": "代理",
  "analytics.kindMcp": "MCP",
  "analytics.kindOther": "其他",
  "analytics.less": "少",
  "analytics.more": "多",

  "limit.cc5h": "5 小時區間",
  "limit.ccWeek": "每週 · 全部模型",
  "limit.ccOpus": "每週 · Opus",
  "limit.ccExtra": "額外用量",
  "limit.codex5h": "5 小時視窗",
  "limit.codexWeek": "每週視窗",
  "limit.codexCredits": "點數",
  "limit.grokCtx": "Context 視窗",
  "limit.weeklyModel": "每週 · {name}",

  "badge.unavailable": "無法取得",
  "badge.estimate": "估算",
  "badge.stale": "過期",

  "note.locked": "已鎖定",
  "note.lockedResetsIn": "已鎖定 · {d} 後重置",
  "note.lockedResets": "已鎖定 · {r} 重置",
  "note.resets": "{r} 重置",
  "note.resetsIn": "{d} 後重置",
  "note.grokSession": "目前對話的記憶容量,開新對話從 0% 開始",
  "note.histRunway": "約 {d} 後見底",
  "note.histBadge": "hist",
  "note.histTooltip": "依你過去完整週期的用量中位數曲線推估(≥2 週期)。",

  "menu.pinClaude": "釘選 Claude",
  "menu.pinCodex": "釘選 Codex",
  "menu.provider": "供應商",
  "menu.settings": "設定",
  "menu.hide": "隱藏島嶼",

  "relogin.cantLaunch": "Atoll 無法啟動 claude,請在終端機執行:",
  "relogin.copy": "複製",
  "relogin.copied": "已複製",
  "relogin.ok": "已開啟登入視窗,完成後請按上方 ⟳ 重新整理。",
  "relogin.opening": "開啟中…",
  "relogin.button": "重新登入 Claude",

  "list.noTools": "沒有執行中的工具",

  "island.hideAria": "隱藏到系統匣",
  "island.hideTitle": "隱藏到系統匣(從系統匣圖示還原)",

  "share.periodToday": "今日",
  "share.periodWeek": "本週",
  "share.periodMonth": "近 30 天",
  "share.now": "目前",
  "share.left": "剩",
  "share.totalTokens": "總 Token",
  "share.estCost": "估算成本",
  "share.estUsd": "估算美元",
  "share.est": "估算",
  "share.cumulativeUsage": "累計用量",
  "share.usageStatement": "用量結算單",
  "share.cumulativeForPeriod": "本期累計",
  "share.tokensAcrossAgents": "分佈於 {n} 個工具",
  "share.streakDays": "{n} 天連勝",
  "share.peakTokens": "尖峰 {tokens}",
  "share.tokens": "token",
  "share.agent": "工具",
  "share.model": "模型",
  "share.agents": "工具",
  "share.models": "模型",
  "share.tokensShare": "Token / 佔比",
  "share.share": "佔比",
  "share.pumpTotal": "累計總量",
  "share.usageReport": "用量戰報",
  "share.generatedBy": "由 Atoll 產生",
  "share.generated": "產生",
  "share.acrossAgents": "分佈於 {n} 個工具",
  "share.sessions": "{n} 場次",
  "share.sessionsLabel": "場次",
  "share.streakInline": "連續 {n} 天",
  "share.peakPerDay": "尖峰 {tokens}/日",
  "share.peakAt": "尖峰 {hour}",
  "share.week": "週",
  "share.used": "已用",
  "share.quotaUsed": "已用額度",
  "share.thisCycle": "本週期",
  "share.lagoonDepth": "礁湖水深",
  "share.quotaUsedCycle": "本週期已用額度",
  "share.fuelDispensed": "本期加注",
  "share.totalSale": "總計金額",
  "share.cumulativeLedger": "累計帳簿",
  "share.shareReport": "用量戰報",
  "share.copyFailed": "複製失敗",
  "share.saved": "已存至 {path}",
  "share.savedShort": "已儲存",
  "share.exportPng": "存 PNG",
  "share.copyImage": "複製",
  "share.style": "樣式",
  "share.range": "範圍",
  "share.quotaNote": "額度行",
  "share.previewTitle": "開啟大圖預覽",
  "share.previewHint": "Esc / 點擊關閉",
  "share.previewGenerating": "\u7522\u751f\u4e2d\u2026",
  "share.previewFailed": "預覽失敗",
} satisfies Record<keyof typeof en, string>;

export type I18nKey = keyof typeof en;

let current: Locale = "en";

/**
 * Resolve a raw `locale` setting to a concrete UI locale.
 *   "en" / "zh-TW"  → used directly
 *   "system" / other → follow `navigator.language` (zh* → zh-TW, else en)
 */
export function resolveLocale(setting: string): Locale {
  if (setting === "en") return "en";
  if (setting === "zh-TW") return "zh-TW";
  const lang =
    (typeof navigator !== "undefined" && navigator.language) || "";
  return lang.toLowerCase().startsWith("zh") ? "zh-TW" : "en";
}

/** Set the active locale and mirror it onto <html lang>. Callers re-render. */
export function setLocale(l: Locale): void {
  current = l;
  if (typeof document !== "undefined") {
    document.documentElement.lang = l;
  }
}

export function getLocale(): Locale {
  return current;
}

/**
 * Translate `key` for an *explicit* `locale`, interpolating `{name}`-style
 * placeholders. Unlike `t`, this reads the named dict rather than the module
 * global — so a caller can render for a locale it was handed (階段 D share.ts
 * renders cards for a passed-in locale, and that must be pure/testable).
 */
export function tl(
  locale: Locale,
  key: I18nKey,
  vars?: Record<string, string | number>,
): string {
  const dict = (locale === "zh-TW" ? zhTW : en) as Record<I18nKey, string>;
  let s = dict[key];
  if (vars) {
    for (const [k, v] of Object.entries(vars)) {
      s = s.split(`{${k}}`).join(String(v));
    }
  }
  return s;
}

/** Translate `key` in the active locale, interpolating `{name}` placeholders. */
export function t(key: I18nKey, vars?: Record<string, string | number>): string {
  return tl(current, key, vars);
}
