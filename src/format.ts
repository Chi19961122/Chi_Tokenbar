// Display formatting helpers.

import type { Locale } from "./i18n";

export const nowSecs = () => Math.floor(Date.now() / 1000);

/** Compact duration: "2d 4h", "3h 12m", "25m", "45s". */
export function fmtDur(secs: number): string {
  if (secs < 0) secs = 0;
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  if (h >= 24) {
    const d = Math.floor(h / 24);
    return `${d}d ${h % 24}h`;
  }
  if (h >= 1) return m > 0 ? `${h}h ${m}m` : `${h}h`;
  if (m >= 1) return `${m}m`;
  return `${Math.floor(secs)}s`;
}

// ── Reset-time formatting (階段 B, settings.reset_display) ─────────────
//
// Two shapes: "relative" (a countdown to reset) and "clock" (the absolute
// wall-clock time). Both are hand-built with a fixed locale — never a bare
// toLocale* — so a zh-TW Windows machine can never leak "週日"/"下午" where we
// mean a fixed English weekday or a locale we control (the v0.2.1 fmtReset
// lesson). The island and the panel both call these, so the two stay in step.

/** Weekday labels, indexed by Date.getDay() (0 = Sunday). Fixed per locale so
 *  the clock format never falls back to the OS locale's words. */
const WEEKDAY: Record<Locale, readonly string[]> = {
  en: ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"],
  "zh-TW": ["週日", "週一", "週二", "週三", "週四", "週五", "週六"],
};
const TOMORROW: Record<Locale, string> = { en: "Tmrw", "zh-TW": "明" };

/** Calendar-day index (local time), so "tomorrow" is a date boundary, not a
 *  24-hour offset — 23:00 → 01:00 is one day away even though it is two hours. */
function calDay(d: Date): number {
  return Math.floor(
    (Date.UTC(d.getFullYear(), d.getMonth(), d.getDate()) - 0) / 86_400_000,
  );
}

/** Wall-clock time following the locale's convention: 12h "2:30 PM" for en,
 *  24h "14:30" for zh-TW. Hand-built, fixed strings. */
function clockHM(d: Date, locale: Locale): string {
  const m = String(d.getMinutes()).padStart(2, "0");
  if (locale === "en") {
    const h = d.getHours();
    const h12 = h % 12 === 0 ? 12 : h % 12;
    return `${h12}:${m} ${h < 12 ? "AM" : "PM"}`;
  }
  return `${String(d.getHours()).padStart(2, "0")}:${m}`;
}

/** Relative reset: a countdown to the reset instant ("3h 12m", "22m"). */
export function fmtResetRel(epochSecs: number, nowSecs: number): string {
  return fmtDur(epochSecs - nowSecs);
}

/**
 * Absolute reset wall-clock, following `locale`:
 *   today      → "14:30" / "2:30 PM"
 *   tomorrow   → "明 14:30" / "Tmrw 2:30 PM"
 *   later      → "週日 14:30" / "Sun 2:30 PM"
 * The day marker is what makes a weekly window's time unambiguous — a bare
 * "09:00" could be days away.
 */
export function fmtResetClock(epochSecs: number, nowSecs: number, locale: Locale): string {
  const d = new Date(epochSecs * 1000);
  const now = new Date(nowSecs * 1000);
  const hm = clockHM(d, locale);
  const dayDiff = calDay(d) - calDay(now);
  if (dayDiff <= 0) return hm;
  if (dayDiff === 1) return `${TOMORROW[locale]} ${hm}`;
  return `${WEEKDAY[locale][d.getDay()]} ${hm}`;
}

/** 1_234_567 -> "1.2M", 12_300 -> "12.3K". */
export function fmtTokens(n: number): string {
  if (n >= 1e9) return `${(n / 1e9).toFixed(2)}B`;
  if (n >= 1e6) return `${(n / 1e6).toFixed(1)}M`;
  if (n >= 1e3) return `${(n / 1e3).toFixed(1)}K`;
  return `${Math.round(n)}`;
}

export function fmtUsd(n: number): string {
  if (n >= 1000) return `$${(n / 1000).toFixed(2)}K`;
  return `$${n.toFixed(2)}`;
}

export const pctLeft = (util: number) => Math.max(0, Math.round(100 - util));
