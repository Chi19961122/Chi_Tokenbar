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
}

/**
 * Global display filter: both providers, or only one. Applied once in the
 * backend scheduler, so it scopes the island, panel, tray, notifications and
 * analytics together.
 */
export type ProviderFilter = "both" | "claude" | "codex";
export type CodexUsageSource = "live" | "auto" | "local";

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

export interface Analytics {
  range: "today" | "week";
  totalTokens: number;
  totalCostUsd: number;
  bestDay: { date: string; costUsd: number };
  activeDays: number;
  daily: DayPoint[];
  hourly: number[]; // 24 buckets, tokens
  byModel: Record<string, number>;
  byAgent: Record<string, number>;
  breakdown: { input: number; cached: number; output: number; reasoning: number };
  sessionsThisWeek: number;
  tokPerMin: number;
  accounts: Account[];
}
