// Shared frontend types — mirror the Rust `model.rs` shapes plus analytics.

export type Status =
  | "normal"
  | "near"
  | "locked"
  | "stale"
  | "insufficient_data"
  | "source_failed"
  | "idle";

export type Provider = "anthropic" | "codex";

/**
 * A remedy the *backend* decided is applicable (model.rs `LimitAction`).
 *
 * Only login-class failures ever carry "relogin". Never infer this by matching
 * on `hint` text: the copy is display-layer prose that changes freely, and a
 * button that launches a login flow must not hinge on a phrase — offering it
 * for a network/AV block would send the user down a dead end.
 */
export type LimitAction = "relogin";

export interface Pace {
  deficit: number;
  in_deficit: boolean;
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
/** Island quota pin per provider: "auto" | "5h" | "week" | "model:<id>". */
export type IslandPin = string;

export interface Settings {
  allow_token_refresh: boolean;
  autostart: boolean;
  warn_pct: number;
  crit_pct: number;
  compact: boolean;
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
  /** 階段 D 戰報 Share: last-used share-card style. Defaults to "statement". */
  share_style: string;
  /** 階段 D 戰報 Share: last-used range ("today"|"week"|"month"). Defaults to "week". */
  share_range: string;
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
  daily: DayPoint[];
  hourly: number[]; // 24 buckets, tokens
  byModel: Record<string, number>;
  byAgent: Record<string, number>;
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
