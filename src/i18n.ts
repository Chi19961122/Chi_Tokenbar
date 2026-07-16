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

  // Analytics subtabs
  "subtab.overview": "Overview",
  "subtab.daily": "Daily",
  "subtab.hourly": "Hourly",
  "subtab.models": "Models",
  "subtab.agents": "Agents",
  "subtab.stats": "Stats",

  // Analytics toggles
  "toggle.today": "Today",
  "toggle.week": "Week",
  "toggle.tokens": "Tokens",
  "toggle.price": "Price",
  "toggle.model": "Model",
  "toggle.agent": "Agent",

  // Analytics content
  "analytics.tokens": "tokens",
  "analytics.estCost": "est. cost",
  "analytics.peak": "peak",
  "analytics.activeDays": "active days",
  "analytics.tokPerMin": "tok/min",
  "analytics.sessionsThisWeek": "sessions this week",
  "analytics.input": "input",
  "analytics.cached": "cached",
  "analytics.output": "output",
  "analytics.reasoning": "reasoning",

  // Limit display names
  "limit.cc5h": "5h session",
  "limit.ccWeek": "Weekly · all models",
  "limit.ccOpus": "Weekly · Opus",
  "limit.ccExtra": "Extra usage",
  "limit.codex5h": "5h window",
  "limit.codexWeek": "Weekly window",
  "limit.codexCredits": "Credits",
  "limit.weeklyModel": "Weekly · {name}",

  // Badges
  "badge.unavailable": "Unavailable",
  "badge.estimate": "Estimate",
  "badge.stale": "Stale",

  // Row note lines (pace copy removed in 階段 B — kept rough for now)
  "note.locked": "Locked",
  "note.lockedResetsIn": "Locked · resets in {d}",
  "note.onPace": "On pace",
  "note.overPace": "{n}% over pace",
  "note.resets": "Resets {r}",
  "note.pacedResets": "{pace} · resets {r}",

  // Detail view
  "detail.locked": "LOCKED",
  "detail.resetsIn": "resets in {d}",
  "detail.unavailableFallback": "Usage data temporarily unavailable",
  "detail.staleNote": "From the last run; may have changed",
  "detail.idle": "Window reset · tool not running",
  "detail.left": "~{d} left",
  "detail.back": "Back",
  "detail.leftLabel": "left",
  "detail.tokens": "{a} / {b} tokens",
  "detail.resetsAt": "resets {t} · {d}",

  // Re-login affordance
  "relogin.cantLaunch": "TokenBar can't launch claude. Run this in your terminal:",
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
} as const;

// ── 繁體中文 (checked exhaustively against `en`) ───────────────────────
const zhTW = {
  "tab.limits": "限額",
  "tab.usage": "分析",
  "header.refreshTitle": "重新整理",
  "header.settingsTitle": "設定",
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

  "subtab.overview": "總覽",
  "subtab.daily": "每日",
  "subtab.hourly": "每時",
  "subtab.models": "模型",
  "subtab.agents": "工具",
  "subtab.stats": "統計",

  "toggle.today": "今日",
  "toggle.week": "本週",
  "toggle.tokens": "Token",
  "toggle.price": "花費",
  "toggle.model": "模型",
  "toggle.agent": "工具",

  "analytics.tokens": "token",
  "analytics.estCost": "估算成本",
  "analytics.peak": "尖峰",
  "analytics.activeDays": "使用天數",
  "analytics.tokPerMin": "tok/分",
  "analytics.sessionsThisWeek": "本週工作階段",
  "analytics.input": "輸入",
  "analytics.cached": "快取",
  "analytics.output": "輸出",
  "analytics.reasoning": "推理",

  "limit.cc5h": "5 小時區間",
  "limit.ccWeek": "每週 · 全部模型",
  "limit.ccOpus": "每週 · Opus",
  "limit.ccExtra": "額外用量",
  "limit.codex5h": "5 小時視窗",
  "limit.codexWeek": "每週視窗",
  "limit.codexCredits": "點數",
  "limit.weeklyModel": "每週 · {name}",

  "badge.unavailable": "無法取得",
  "badge.estimate": "估算",
  "badge.stale": "過期",

  "note.locked": "已鎖定",
  "note.lockedResetsIn": "已鎖定 · {d} 後重置",
  "note.onPace": "跟上進度",
  "note.overPace": "超前 {n}%",
  "note.resets": "{r} 重置",
  "note.pacedResets": "{pace} · {r} 重置",

  "detail.locked": "已鎖定",
  "detail.resetsIn": "{d} 後重置",
  "detail.unavailableFallback": "用量資料暫時無法取得",
  "detail.staleNote": "來自上次執行,可能已變動",
  "detail.idle": "視窗已重置 · 工具未執行",
  "detail.left": "剩 ~{d}",
  "detail.back": "返回",
  "detail.leftLabel": "剩餘",
  "detail.tokens": "{a} / {b} token",
  "detail.resetsAt": "{t} 重置 · {d}",

  "relogin.cantLaunch": "TokenBar 無法啟動 claude,請在終端機執行:",
  "relogin.copy": "複製",
  "relogin.copied": "已複製",
  "relogin.ok": "已開啟登入視窗,完成後請按上方 ⟳ 重新整理。",
  "relogin.opening": "開啟中…",
  "relogin.button": "重新登入 Claude",

  "list.noTools": "沒有執行中的工具",

  "island.hideAria": "隱藏到系統匣",
  "island.hideTitle": "隱藏到系統匣(從系統匣圖示還原)",
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

/** Translate `key`, interpolating `{name}`-style placeholders from `vars`. */
export function t(key: I18nKey, vars?: Record<string, string | number>): string {
  const dict = (current === "zh-TW" ? zhTW : en) as Record<I18nKey, string>;
  let s = dict[key];
  if (vars) {
    for (const [k, v] of Object.entries(vars)) {
      s = s.split(`{${k}}`).join(String(v));
    }
  }
  return s;
}
