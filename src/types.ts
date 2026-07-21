// Shared frontend types — mirror the Rust `model.rs` shapes plus analytics.

export type Status =
  | "normal"
  | "near"
  | "locked"
  | "stale"
  | "insufficient_data"
  | "source_failed"
  | "idle";

export type Provider = "anthropic" | "codex" | "grok";

/**
 * A remedy the *backend* decided is applicable (model.rs `LimitAction`).
 *
 * Only login-class failures ever carry "relogin". Never infer this by matching
 * on `hint` text: the copy is display-layer prose that changes freely, and a
 * button that launches a login flow must not hinge on a phrase — offering it
 * for a network/AV block would send the user down a dead end.
 */
export type LimitAction = "relogin";

/** Which curve produced the runway (T-feat-007): "linear" is the recent-slope
 *  projection, "historical" the ≥2-cycle median curve. */
export type PaceBasis = "linear" | "historical";

export interface Pace {
  deficit: number;
  in_deficit: boolean;
  /** T-feat-007 snapshot fields — optional so a snapshot from an older backend
   *  (and the frozen crosscheck fixtures that build `pace: null`) still type. */
  pace_basis?: PaceBasis;
  /** Fraction of historical cycles that hit 100 / locked; absent below the
   *  ≥2-cycle threshold. */
  run_out_probability?: number | null;
}

export interface Limit {
  id: string;
  provider: Provider;
  label: string;
  util: number; // 0..100 (canonical)
  resets_at: number; // epoch secs, 0 if unknown
  window_secs: number;
  status: Status;
  absolute: [number, number] | null; // [used, cap] tokens
  pace: Pace | null;
  runway_secs: number | null;
  /** Plain-language reason shown when status is "source_failed"; absent otherwise. */
  hint?: string;
  /** Backend-decided remedy; only login-class failures carry one. */
  action?: LimitAction;
}

export interface Snapshot {
  limits: Limit[];
  worst_id: string | null;
  updated_at: number;
  /** Seconds until the next backend data fetch, as of `updated_at`. The header
   *  countdown is `max(0, next_fetch_in - (now - updated_at))`. */
  next_fetch_in: number;
}

/**
 * Global display filter: both providers, or only one. Applied once in the
 * backend scheduler, so it scopes the island, panel, tray, notifications and
 * analytics together.
 */
export type ProviderFilter = "both" | "claude" | "codex";
export type CodexUsageSource = "live" | "auto" | "local";

/** Which tab a press on the island opens (settings.expand_default). */
export type ExpandDefault = "compact" | "usage";
/** Island right-side aux readout (settings.island_aux). */
export type IslandAux = "off" | "tok_per_min" | "cost_today";
/** How reset times render (settings.reset_display). */
export type ResetDisplay = "relative" | "clock";
/** UI theme (settings.theme): follow the OS, or force light/dark. */
export type ThemeMode = "system" | "light" | "dark";
/** Island quota pin per provider: "auto" | "5h" | "week" | "model:<id>". */
export type IslandPin = string;

export interface Settings {
  allow_token_refresh: boolean;
  autostart: boolean;
  warn_pct: number;
  crit_pct: number;
  compact: boolean;
  /** The unified multi-select of sources (T-916, slimmed T-917) — read this, not
   *  the deprecated `providers` field below. Any of "claude" | "codex" | "grok";
   *  empty means nothing shown (honest empty UI). Mirrors config.rs
   *  `Settings::sources`. */
  sources: string[];
  /** DEPRECATED (T-916): folded into `sources`. Still present because the
   *  backend writes it for one-version downgrade safety; not read at runtime. */
  providers: ProviderFilter;
  codex_usage_source: CodexUsageSource;
  /** Keep the island above other windows. Defaults to true (matches tauri.conf.json). */
  always_on_top: boolean;
  /** UI language: "system" (follow navigator.language), "en", or "zh-TW".
   *  Defaults to "system" (matches config.rs `Settings::default()`). */
  locale: string;
  /** Which tab a press on the island opens. Defaults to "compact". */
  expand_default: ExpandDefault;
  /** Island quota pin for Claude / Codex. Defaults to "auto". */
  island_pin_claude: IslandPin;
  island_pin_codex: IslandPin;
  /** Island right-side aux readout. Defaults to "tok_per_min". */
  island_aux: IslandAux;
  /** Reset-time display style. Defaults to "relative". */
  reset_display: ResetDisplay;
  /** UI theme: "system" (follow prefers-color-scheme), "light", or "dark".
   *  Defaults to "system" (matches config.rs `Settings::default()`). */
  theme: ThemeMode;
  /** 階段 D 戰報 Share: last-used share-card style. Defaults to "statement". */
  share_style: string;
  /** 階段 D 戰報 Share: last-used range ("today"|"week"|"month"). Defaults to "week". */
  share_range: string;
  /** T-905 戰報尺寸: last-used share-card size ("auto"|"story"). "story" is the
   *  9:16 portrait. Defaults to "auto". */
  share_size: string;
  /** T-910 更新頻率: quota-API poll cadence in seconds. One of 30 | 60 | 180.
   *  Defaults to 180 (mirrors config.rs `Settings::default()`); the backend
   *  clamps to those three and applies 429 backoff on the Anthropic side. */
  refresh_secs: number;
}

// ── Layer ③ analytics (UX Spec v3 §11) ──────────────────────────────

export interface DayPoint {
  date: string; // YYYY-MM-DD
  byModel: Record<string, number>; // model -> tokens
  byAgent: Record<string, number>; // client -> tokens
  costUsd: number;
}

export interface Account {
  client: string; // "Claude Code", "Codex CLI"
  provider: Provider;
  account: string;
  plan: string;
}

export type AnalyticsRange = "today" | "week" | "month";

/** Activity-type slice (階段 C+). `kind` is a stable id ("edit" | "read" | "run"
 *  | "other") the UI localizes. Claude-only — Codex tokens aren't per-tool
 *  attributable, so this section is absent when nothing is classifiable. */
export interface KindCount {
  kind: string;
  tokens: number;
}

/** Per-project token total (階段 C+). `name === "__other__"` marks the merged
 *  remainder beyond the top 8. */
export interface ProjectCount {
  name: string;
  tokens: number;
}

export interface Analytics {
  range: AnalyticsRange;
  /** Earliest day actually shown (backend `range_start_day`). Equals the window
   *  start unless local logs are shorter than the requested window. */
  rangeStartDay: string;
  totalTokens: number;
  totalCostUsd: number;
  bestDay: { date: string; costUsd: number };
  activeDays: number;
  records: {
    maxDay: { date: string; tokens: number };
    maxHour: { date: string; hour: number; tokens: number };
    streakDays: number;
    prNow: boolean;
  };
  daily: DayPoint[];
  hourly: number[]; // 24 buckets, tokens
  hourlyCost: number[]; // 24 buckets, cost USD (parallels `hourly`)
  byModel: Record<string, number>;
  byAgent: Record<string, number>;
  /** Range-total cost per model / per agent, keyed like `byModel` / `byAgent`.
   *  Drives the metric toggle's price mode on the "share" breakdown. */
  byModelCost: Record<string, number>;
  byAgentCost: Record<string, number>;
  breakdown: { input: number; cached: number; output: number; reasoning: number };
  /** Activity-type breakdown (Claude tool usage). Empty → section omitted. */
  byKind: KindCount[];
  /** Per-project token totals, top 8 + "__other__". Usage-only.
   *  不得進戰報(§0):階段 D 的 buildShareData 禁止引用此欄位。 */
  byProject: ProjectCount[];
  sessionsThisWeek: number;
  tokPerMin: number;
  accounts: Account[];
}
