// Mock data for running/verifying the UI in a plain browser (no Tauri backend).
// Lets me preview every island/card state without live quotas.

import type { Analytics, Limit, Snapshot } from "./types";
import { nowSecs } from "./format";

function limit(p: Partial<Limit> & { id: string; label: string; util: number }): Limit {
  const now = nowSecs();
  return {
    provider: "codex",
    resets_at: now + 3600,
    window_secs: 5 * 3600,
    status: "normal",
    absolute: null,
    pace: null,
    runway_secs: null,
    ...p,
  };
}

function snap(limits: Limit[], worst?: string): Snapshot {
  return {
    limits,
    worst_id: worst ?? (limits[0]?.id || null),
    updated_at: nowSecs(),
    next_fetch_in: 180, // matches the backend REFRESH_SECS cadence
  };
}

const now = nowSecs();

export const SCENARIOS: Record<string, Snapshot> = {
  safe: snap([
    limit({ id: "codex.5h", label: "Codex·5h", util: 12, provider: "codex", status: "normal", resets_at: now + 4 * 3600, window_secs: 5 * 3600, absolute: [120_000, 1_000_000] }),
    limit({ id: "codex.week", label: "Codex·Weekly", util: 22, provider: "codex", status: "normal", resets_at: now + 5 * 86400, window_secs: 7 * 86400, absolute: [4_400_000, 20_000_000] }),
    limit({ id: "cc.5h", label: "Claude·5h", util: 30, provider: "anthropic", status: "normal", resets_at: now + 2 * 3600, window_secs: 5 * 3600 }),
    limit({ id: "cc.week", label: "Claude·Weekly", util: 41, provider: "anthropic", status: "normal", resets_at: now + 5 * 86400, window_secs: 7 * 86400 }),
    limit({ id: "cc.w.fable", label: "Claude·Fable", util: 5, provider: "anthropic", status: "normal", resets_at: now + 5 * 86400, window_secs: 7 * 86400 }),
    limit({ id: "cc.opus", label: "Claude·Opus", util: 18, provider: "anthropic", status: "normal", resets_at: now + 5 * 86400, window_secs: 7 * 86400 }),
  ], "cc.week"),

  near: snap([
    limit({ id: "codex.5h", label: "Codex·5h", util: 88, provider: "codex", status: "near", resets_at: now + 25 * 60, window_secs: 5 * 3600, runway_secs: 22 * 60, pace: { deficit: 38, in_deficit: true }, absolute: [880_000, 1_000_000] }),
    limit({ id: "codex.week", label: "Codex·Weekly", util: 61, provider: "codex", status: "normal", resets_at: now + 5 * 86400, window_secs: 7 * 86400, pace: { deficit: 24, in_deficit: true }, runway_secs: 38 * 3600 }),
    limit({ id: "cc.5h", label: "Claude·5h", util: 55, provider: "anthropic", status: "normal", resets_at: now + 2 * 3600, window_secs: 5 * 3600 }),
    limit({ id: "cc.week", label: "Claude·Weekly", util: 47, provider: "anthropic", status: "normal", resets_at: now + 3 * 86400, window_secs: 7 * 86400 }),
  ], "codex.5h"),

  locked: snap([
    limit({ id: "codex.5h", label: "Codex·5h", util: 100, provider: "codex", status: "locked", resets_at: now + 80 * 60, window_secs: 5 * 3600, absolute: [1_000_000, 1_000_000] }),
    limit({ id: "codex.week", label: "Codex·Weekly", util: 72, provider: "codex", status: "normal", resets_at: now + 2 * 86400, window_secs: 7 * 86400 }),
    limit({ id: "cc.week", label: "Claude·Weekly", util: 63, provider: "anthropic", status: "normal", resets_at: now + 3 * 86400, window_secs: 7 * 86400 }),
  ], "codex.5h"),

  // Hints and actions mirror anthropic.rs `FailureStage::{user_hint, action}`.
  // The pair is deliberate — it is the case the button must get right:
  //  · cc.5h  = UsageTransport (AV/corporate network). The longest line there
  //    is, and NO action — a "re-login" button here would be a dead end.
  //  · cc.week = UsageHttp(401), a login failure, so it does get the button.
  // Real Claude failures hit both windows at once, but splitting them lets the
  // preview show both branches side by side at the real 380px panel width.
  degraded: snap([
    limit({ id: "cc.5h", label: "Claude·5h", util: 0, provider: "anthropic", status: "source_failed", resets_at: 0, window_secs: 5 * 3600, hint: "Can't reach Claude. Check your network; a corporate network or antivirus may be blocking the connection." }),
    limit({ id: "cc.week", label: "Claude·Weekly", util: 0, provider: "anthropic", status: "source_failed", resets_at: 0, window_secs: 7 * 86400, hint: "Your Claude login is no longer valid. Sign in to Claude Code again.", action: "relogin" }),
    limit({ id: "codex.5h", label: "Codex·5h", util: 34, provider: "codex", status: "normal", resets_at: now + 3 * 3600, window_secs: 5 * 3600 }),
  ], "codex.5h"),

  stale: snap([
    limit({ id: "codex.5h", label: "Codex·5h", util: 0, provider: "codex", status: "idle", resets_at: 0, window_secs: 5 * 3600 }),
    limit({ id: "codex.week", label: "Codex·Weekly", util: 34, provider: "codex", status: "stale", resets_at: now + 2 * 86400, window_secs: 7 * 86400 }),
    limit({ id: "cc.week", label: "Claude·Weekly", util: 41, provider: "anthropic", status: "normal", resets_at: now + 5 * 86400, window_secs: 7 * 86400 }),
  ], "cc.week"),

  empty: snap([], undefined),
};

export function mockAnalytics(range: "today" | "week" | "month"): Analytics {
  const days = range === "month" ? 30 : range === "week" ? 7 : 1;
  const daily = Array.from({ length: days }, (_, i) => {
    const d = new Date();
    d.setDate(d.getDate() - (days - 1 - i));
    const claude = Math.round(20_000_000 + Math.random() * 90_000_000);
    const codex = Math.round(10_000_000 + Math.random() * 60_000_000);
    return {
      date: d.toISOString().slice(0, 10),
      byModel: {
        "opus-4.8": Math.round(claude * 0.6),
        "sonnet-5": Math.round(claude * 0.4),
        "gpt-5-codex": Math.round(codex * 0.8),
        "gpt-5-mini": Math.round(codex * 0.2),
      },
      byAgent: { "Claude Code": claude, "Codex CLI": codex },
      costUsd: (claude / 1e6) * 9 + (codex / 1e6) * 5,
    };
  });

  const byModel: Record<string, number> = {};
  const byAgent: Record<string, number> = {};
  let totalTokens = 0;
  let totalCostUsd = 0;
  let best = { date: daily[0].date, costUsd: 0 };
  for (const d of daily) {
    for (const [k, v] of Object.entries(d.byModel)) byModel[k] = (byModel[k] || 0) + v;
    for (const [k, v] of Object.entries(d.byAgent)) {
      byAgent[k] = (byAgent[k] || 0) + v;
      totalTokens += v;
    }
    totalCostUsd += d.costUsd;
    if (d.costUsd > best.costUsd) best = { date: d.date, costUsd: d.costUsd };
  }

  return {
    range,
    rangeStartDay: daily[0].date, // mock always has full-window coverage
    totalTokens,
    totalCostUsd,
    bestDay: best,
    activeDays: days,
    records: {
      maxDay: {
        date: daily.reduce((best, d) => d.costUsd > best.costUsd ? d : best).date,
        tokens: Math.max(...daily.map((d) => Object.values(d.byAgent).reduce((sum, n) => sum + n, 0))),
      },
      maxHour: { date: daily[daily.length - 1].date, hour: new Date().getHours(), tokens: 7_800_000 },
      streakDays: days,
      prNow: true,
    },
    daily,
    hourly: Array.from({ length: 24 }, () => Math.round(Math.random() * 8_000_000)),
    byModel,
    byAgent,
    breakdown: {
      input: Math.round(totalTokens * 0.35),
      cached: Math.round(totalTokens * 0.45),
      output: Math.round(totalTokens * 0.15),
      reasoning: Math.round(totalTokens * 0.05),
    },
    // Activity types (Claude-only in reality) and per-project totals (階段 C+).
    byKind: [
      { kind: "edit", tokens: Math.round(totalTokens * 0.34) },
      { kind: "read", tokens: Math.round(totalTokens * 0.28) },
      { kind: "run", tokens: Math.round(totalTokens * 0.22) },
      { kind: "other", tokens: Math.round(totalTokens * 0.16) },
    ],
    byProject: [
      { name: "tokenbar", tokens: Math.round(totalTokens * 0.4) },
      { name: "webapp", tokens: Math.round(totalTokens * 0.24) },
      { name: "infra", tokens: Math.round(totalTokens * 0.14) },
      { name: "docs-site", tokens: Math.round(totalTokens * 0.08) },
      { name: "__other__", tokens: Math.round(totalTokens * 0.14) },
    ],
    sessionsThisWeek: 18,
    tokPerMin: 608_000,
    accounts: [
      { client: "Claude Code", provider: "anthropic", account: "you@example.com", plan: "Max 5x" },
      { client: "Codex CLI", provider: "codex", account: "you@example.com", plan: "Plus" },
    ],
  };
}
