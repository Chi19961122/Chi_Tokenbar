// Atoll 戰報 Share — pure data layer + six share-card renderers.
//
// This module is deliberately pure and side-effect free (beyond importing its
// own CSS): buildShareData() and renderShareCard() take an explicit locale and
// never touch the DOM's global i18n state, so both are unit-testable and can be
// driven for either locale. All DOM/IO (mounting, export, clipboard, settings)
// lives in share-panel.ts.
//
// §0 HARD RULE: the share layer must NEVER reference Analytics.byProject, project
// names, host names, or conversation content. Only totalTokens, totalCostUsd,
// byAgent, byModel, hourly, records, sessionsThisWeek, daily (for dates), limits
// and range are read here. See the comment on `byProject` in types.ts.
//
// T-922: the six renderers are reimplemented for the Atoll (ring / lagoon) visual
// direction, ported 1:1 from design/refs/atoll-share-preview.html under the repo
// `.shXX-card` class prefixes, wired to the real T-915 analytics pipeline. The six
// ShareStyle keys are STABLE (persisted in `share_style`); only what each renders
// + its display label changed:
//   island_card → Atoll ring gauge (flagship)   statement → Ledger
//   diagnostics → Terminal (atoll --report)      minimal   → Minimal
//   fuel        → Sounding (lagoon depth)         wa        → Seal (◎ ring seal)
// All copy: zero em/en dashes (hyphen only), rationed middots, one accent (#EC4899),
// unified ◎ ring-mark signature + "Atoll" + mono genMonthYear.

import "./share.css";
import type { Analytics, AnalyticsRange, Limit } from "./types";
import type { Locale } from "./i18n";
import { tl } from "./i18n";
import { fmtTokens } from "./format";

export type ShareStyle =
  | "statement"
  | "diagnostics"
  | "minimal"
  | "fuel"
  | "island_card"
  | "wa";

/** Share-card aspect: "auto" is the original 1200×675 landscape; "story" is the
 *  9:16 portrait (360×640 CSS px) social-story variant. Toggled in the report
 *  panel and persisted as `share_size`. Adds the `sh-916` class to the card. */
export type ShareSize = "auto" | "story";

export interface ShareSplit {
  name: string;
  tokens: number;
  pct: number; // round(tokens / totalTokens * 100) — share of THIS period's total
}

/** One structured quota row for the island_card (Atoll ring) gauge. `util` is the
 *  USED % (0-100) — the ONE sanctioned exposure of a subscription %, deliberately
 *  "used" (opposite the app's `% left` convention). `label` is "Brand · window". */
export interface QuotaGaugeRow {
  label: string;
  util: number;
}

export interface ShareData {
  totalTokens: number;
  totalCostUsd: number; // always displayed labeled "est."
  streakDays: number;
  maxDayTokens: number;
  sessionCount: number; // ← Analytics.sessionsThisWeek (week-scoped; labeled neutrally)
  hourly: number[]; // ← Analytics.hourly (24 buckets), sparkline + depth profile
  peakHour: number; // ← Analytics.records.maxHour.hour (0-23), rendered "HH:00"
  byAgent: ShareSplit[]; // from Analytics.byAgent, only tokens>0, sorted desc
  byModel: ShareSplit[]; // from Analytics.byModel, only tokens>0, sorted desc
  agentCount: number; // byAgent.length (tokens>0)
  periodLabel: string; // locale-aware, date-embedded, built here
  genMonthYear?: string; // uppercase "MON YYYY" from the period's last day; omit if unknown
  docNo?: string; // "AT-YYYY-MMDD" from the period's last day (statement doc number)
  quotaGauge?: QuotaGaugeRow[]; // island_card only; present only when includeQuotaNote && limits
}

export interface BuildOpts {
  range: AnalyticsRange;
  locale: Locale;
  limits?: Limit[];
  includeQuotaNote?: boolean;
}

// ── period label (fixed month table — NEVER toLocale*, per fmtReset lesson) ──

/** Fixed English month labels, so a zh-TW machine can never leak an OS-localized
 *  month into a share card (the v0.2.1 fmtReset lesson — see format.ts header). */
const MONTHS_EN = [
  "Jan", "Feb", "Mar", "Apr", "May", "Jun",
  "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/** Parse a YYYY-MM-DD bucket into numbers via string split (tz-safe: no Date). */
function parseYmd(date: string): { y: number; m: number; d: number } {
  const [y, m, d] = date.split("-").map((s) => Number(s));
  return { y, m, d };
}

type Ymd = { y: number; m: number; d: number };

/** A single date, per locale: en "Jul 17", zh "7月17日". */
function fmtOneDate(p: Ymd, locale: Locale): string {
  if (locale === "zh-TW") return `${p.m}月${p.d}日`;
  return `${MONTHS_EN[p.m - 1] ?? ""} ${p.d}`;
}

/** A date span, per locale, collapsing a shared month:
 *   en same month  "Jul 10 - 16"     · cross month "Jun 28 - Jul 4"
 *   zh same month  "7月10日 - 16日"   · cross month "6月28日 - 7月4日" */
function fmtSpan(a: Ymd, b: Ymd, locale: Locale): string {
  const sameDay = a.y === b.y && a.m === b.m && a.d === b.d;
  if (sameDay) return fmtOneDate(a, locale);
  const sameMonth = a.y === b.y && a.m === b.m;
  if (locale === "zh-TW") {
    return sameMonth
      ? `${a.m}月${a.d}日 - ${b.d}日`
      : `${a.m}月${a.d}日 - ${b.m}月${b.d}日`;
  }
  return sameMonth
    ? `${MONTHS_EN[a.m - 1]} ${a.d} - ${b.d}`
    : `${MONTHS_EN[a.m - 1]} ${a.d} - ${MONTHS_EN[b.m - 1]} ${b.d}`;
}

/** "This week · Jul 10 - 16" / "本週 · 7月10日 - 16日". Range WORD from the i18n
 *  dict; the dates formatted here with the fixed month table. */
function buildPeriodLabel(a: Analytics, locale: Locale): string {
  const word = tl(
    locale,
    a.range === "today"
      ? "share.periodToday"
      : a.range === "month"
      ? "share.periodMonth"
      : "share.periodWeek",
  );
  if (a.daily.length === 0) return word;
  const first = parseYmd(a.daily[0].date);
  const last = parseYmd(a.daily[a.daily.length - 1].date);
  return `${word} · ${fmtSpan(first, last, locale)}`;
}

/** The period's last daily date, or undefined if there is no daily data. Used for
 *  both the uppercase "JUL 2026" signature date and the "AT-YYYY-MMDD" doc number,
 *  which must both derive from the same fixed month table (never toLocale*). */
function lastDay(a: Analytics): Ymd | undefined {
  if (a.daily.length === 0) return undefined;
  return parseYmd(a.daily[a.daily.length - 1].date);
}

/** Uppercase "MON YYYY" from a date, e.g. "JUL 2026". Fixed month table. */
function fmtMonthYear(p: Ymd): string | undefined {
  const mon = MONTHS_EN[p.m - 1];
  if (!mon) return undefined;
  return `${mon.toUpperCase()} ${p.y}`;
}

/** Statement doc number "AT-2026-0718" from a date (zero-padded month+day). */
function fmtDocNo(p: Ymd): string {
  const mm = String(p.m).padStart(2, "0");
  const dd = String(p.d).padStart(2, "0");
  return `AT-${p.y}-${mm}${dd}`;
}

// ── quota gauge (the ONLY place a subscription-limit % may appear) ───────────

/** Up to 3 structured gauge rows for the island_card: Claude 5h, Claude week,
 *  Codex week — whichever exist, in that order. `util` is the USED % (0-100),
 *  the sanctioned single exposure of a subscription % (decision #1). The window
 *  descriptor follows locale (5h fixed English; week → "week"/"週"); the brand is
 *  fixed English. Returns undefined when no windowed limit is present. */
function buildQuotaGauge(limits: Limit[], locale: Locale): QuotaGaugeRow[] | undefined {
  const weekWord = tl(locale, "share.week");
  const pick = (provider: Limit["provider"], win: "5h" | "week"): Limit | undefined =>
    limits.find((l) => l.provider === provider && l.id.endsWith(`.${win}`));

  const rows: QuotaGaugeRow[] = [];
  const c5 = pick("anthropic", "5h");
  const cw = pick("anthropic", "week");
  const xw = pick("codex", "week");
  if (c5) rows.push({ label: "Claude · 5h", util: c5.util });
  if (cw) rows.push({ label: `Claude · ${weekWord}`, util: cw.util });
  if (xw) rows.push({ label: `Codex · ${weekWord}`, util: xw.util });
  return rows.length > 0 ? rows.slice(0, 3) : undefined;
}

// ── buildShareData ───────────────────────────────────────────────────────────

/** Whole-number share of the *period total* for one entry. */
function splitsFrom(rec: Record<string, number>, totalTokens: number): ShareSplit[] {
  return Object.entries(rec)
    .filter(([, v]) => v > 0)
    .sort((a, b) => b[1] - a[1])
    .map(([name, tokens]) => ({
      name,
      tokens,
      pct: totalTokens > 0 ? Math.round((tokens / totalTokens) * 100) : 0,
    }));
}

export function buildShareData(a: Analytics, opts: BuildOpts): ShareData {
  const byAgent = splitsFrom(a.byAgent, a.totalTokens);
  const byModel = splitsFrom(a.byModel, a.totalTokens);
  const last = lastDay(a);
  const quotaGauge =
    opts.includeQuotaNote && opts.limits && opts.limits.length > 0
      ? buildQuotaGauge(opts.limits, opts.locale)
      : undefined;
  return {
    totalTokens: a.totalTokens,
    totalCostUsd: a.totalCostUsd,
    streakDays: a.records.streakDays,
    maxDayTokens: a.records.maxDay.tokens,
    sessionCount: a.sessionsThisWeek,
    hourly: a.hourly.length === 24 ? a.hourly : new Array(24).fill(0),
    peakHour: a.records.maxHour.hour,
    byAgent,
    byModel,
    agentCount: byAgent.length,
    periodLabel: buildPeriodLabel(a, opts.locale),
    genMonthYear: last ? fmtMonthYear(last) : undefined,
    docNo: last ? fmtDocNo(last) : undefined,
    quotaGauge,
  };
}

// ── card rendering ───────────────────────────────────────────────────────────

/** Escape text before interpolating into innerHTML (agent/model names are
 *  data-derived; keep the card injection-safe even though they're normally
 *  clean identifiers). */
function esc(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

/** Comma-grouped full digits, locale-safe (fixed en-US grouping). */
function grouped(n: number): string {
  return n.toLocaleString("en-US");
}

/** "$47.20". Cost is always labeled "est." at the call site. */
function money(n: number): string {
  return `$${n.toFixed(2)}`;
}

/** Split an abbreviated token count ("8.2M") into number + unit for the concepts
 *  that render the suffix smaller. */
function splitAbbrev(n: number): { num: string; unit: string } {
  const s = fmtTokens(n);
  const m = /^([\d.]+)([A-Za-z]*)$/.exec(s);
  return m ? { num: m[1], unit: m[2] } : { num: s, unit: "" };
}

/** Bar width (%) of one split relative to the largest split shown. */
function barPct(tokens: number, maxTokens: number): number {
  return maxTokens > 0 ? (tokens / maxTokens) * 100 : 0;
}

/** "14:00" — zero-padded hour, fixed (never toLocale*). */
function fmtHour(hour: number): string {
  return `${String(hour).padStart(2, "0")}:00`;
}

const TOP_N = 5;

/** The ◎ quota-arc mark: faint full track + ~240° bright arc (gap upper-left,
 *  round caps) + centre dot, `currentColor`. Matches the installer / tray app
 *  icon (the magenta quota gauge), the canonical brand glyph. Sized by CSS per
 *  slot. */
const RING_MARK =
  `<svg class="rm" viewBox="0 0 24 24" fill="none">` +
  `<circle cx="12" cy="12" r="9.5" stroke="currentColor" stroke-width="1.7" opacity="0.22"/>` +
  `<path d="M9.54 2.82 A9.5 9.5 0 1 1 5.28 18.72" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"/>` +
  `<circle cx="12" cy="12" r="3.6" fill="currentColor"/></svg>`;

/** The larger ◎ seal glyph (two concentric rings + centre dot) for the Seal card's
 *  ring-seal badge. `currentColor` (white on the accent badge). */
const SEAL_MARK =
  `<svg viewBox="0 0 24 24" fill="none">` +
  `<circle cx="12" cy="12" r="9.5" stroke="currentColor" stroke-width="1.6"/>` +
  `<circle cx="12" cy="12" r="5.4" stroke="currentColor" stroke-width="1.6"/>` +
  `<circle cx="12" cy="12" r="1.6" fill="currentColor"/></svg>`;

const BRAND = "Atoll";

/** Unified signature block: ◎ ring mark + "Atoll". The date suffix (mono
 *  "JUL 2026") lives in each card's `.sig-r` slot, composed per-template. */
function sig(): string {
  return `<div class="sig">${RING_MARK}<span class="bn">${BRAND}</span></div>`;
}

/** Hero subline shared by Ledger: "across N agents · K sessions · streak Nd ·
 *  peak X/day", each segment dropped when its value is absent. */
function heroSubline(data: ShareData, T: TFn): string {
  const parts: string[] = [T("share.acrossAgents", { n: data.agentCount })];
  if (data.sessionCount > 0) parts.push(T("share.sessions", { n: data.sessionCount }));
  if (data.streakDays > 0) parts.push(T("share.streakInline", { n: data.streakDays }));
  if (data.maxDayTokens > 0)
    parts.push(T("share.peakPerDay", { tokens: fmtTokens(data.maxDayTokens) }));
  return parts.join(" · ");
}

/** Build the `-card` root element for a style. `<div class="…-card">` with the
 *  concept markup as innerHTML; the caller sizes/mounts it. */
export function renderShareCard(
  style: ShareStyle,
  data: ShareData,
  locale: Locale,
  opts?: { fuelGroup?: "model" | "agent"; size?: ShareSize },
): HTMLElement {
  const T = (key: Parameters<typeof tl>[1], vars?: Record<string, string | number>) =>
    tl(locale, key, vars);

  const card = (() => {
    switch (style) {
      case "statement":
        return ledgerCard(data, T);
      case "diagnostics":
        return terminalCard(data);
      case "minimal":
        return minimalCard(data, T);
      case "fuel":
        // Sounding's depth-layer legend honours the fuelGroup toggle that the
        // report panel still shows for this slot; a bare call defaults to agent
        // (the mock/brief "layers = byAgent top-2"). See soundingCard.
        return soundingCard(data, T, opts?.fuelGroup ?? "agent");
      case "island_card":
        return atollCard(data, T);
      case "wa":
        return sealCard(data, T);
    }
  })();

  // Portrait 9:16 story variant: the same markup re-lays-out under the `sh-916`
  // class (see share.css). Landscape ("auto") keeps the original geometry.
  if ((opts?.size ?? "auto") === "story") card.classList.add("sh-916");
  return card;
}

type TFn = (key: Parameters<typeof tl>[1], vars?: Record<string, string | number>) => string;

function el(cls: string, html: string): HTMLElement {
  const d = document.createElement("div");
  d.className = cls;
  d.innerHTML = html;
  return d;
}

// ── island_card → Atoll ring gauge (flagship; the ONLY quota exposure) ────────
// Three concentric coral-ring arcs = quotaGauge (USED %), centre lagoon holds a
// static "Quota used / this cycle" label, big total + cost + sessions + streak on
// the left. Arcs use SVG pathLength="100" + stroke-dasharray="{util} 100".
const RING_RADII = [150, 125, 100];
const RING_TRACK = "#F0F0F1";
const RING_FILLS = ["#18181B", "#52525B", "#71717A"]; // primary / secondary / muted (pinned)

function atollCard(data: ShareData, T: TFn): HTMLElement {
  const total = splitAbbrev(data.totalTokens);
  const gauge = (data.quotaGauge ?? []).slice(0, 3);

  // Always draw the three faint track rings (the atoll motif); draw a value arc +
  // legend row only for gauge rows that exist (so <3 rows renders a partial target
  // and 0 rows a decorative empty target — never fabricated quota).
  const arcs = RING_RADII.map((r, i) => {
    const track = `<circle cx="170" cy="170" r="${r}" stroke="${RING_TRACK}"/>`;
    if (i >= gauge.length) return track;
    const util = Math.max(0, Math.min(100, Math.round(gauge[i].util)));
    return (
      track +
      `<circle cx="170" cy="170" r="${r}" stroke="${RING_FILLS[i]}" ` +
      `pathLength="100" stroke-dasharray="${util} 100"/>`
    );
  }).join("");

  const legend = gauge
    .map((g, i) => {
      const [brand, ...rest] = g.label.split(" · ");
      const desc = rest.join(" · ");
      const util = Math.max(0, Math.min(100, Math.round(g.util)));
      return (
        `<div class="at-lrow"><span class="dot" style="background:${RING_FILLS[i]}"></span>` +
        `<span class="nm">${esc(brand)}${desc ? ` <small>· ${esc(desc)}</small>` : ""}</span>` +
        `<span class="pv tnum">${util}%<small> ${T("share.used")}</small></span></div>`
      );
    })
    .join("");

  const subParts: string[] = [
    `<span class="cost tnum">${money(data.totalCostUsd)}<u>${T("share.est")}</u></span>`,
  ];
  if (data.sessionCount > 0) subParts.push(`<span>${T("share.sessions", { n: data.sessionCount })}</span>`);
  if (data.streakDays > 0) subParts.push(`<span>${T("share.streakInline", { n: data.streakDays })}</span>`);
  const sub = subParts.join(`<span class="sp"></span>`);
  const genDate = data.genMonthYear ?? "";

  return el(
    "shic-card",
    `
    <div class="at-l">
      <div class="at-top">
        <div class="at-pill">${RING_MARK}<b>${BRAND}</b><span class="sep"></span>` +
      `<span class="liv">LIVE</span></div>
        <div class="at-period">${esc(data.periodLabel)}</div>
      </div>
      <div class="at-hero">
        <div class="at-eyebrow">${T("share.cumulativeUsage")}</div>
        <div class="at-big tnum">${total.num}<em>${total.unit}</em></div>
        <div class="at-sub">${sub}</div>
      </div>
    </div>
    <div class="at-r">
      <div class="at-ring">
        <svg viewBox="0 0 340 340" width="100%" height="100%">` +
      `<g fill="none" stroke-width="15" stroke-linecap="round">${arcs}</g></svg>
        <div class="lag"><div class="k">${T("share.quotaUsed")}</div>` +
      `<div class="v tnum">${T("share.thisCycle")}</div></div>
      </div>
      <div class="at-legend">${legend}</div>
    </div>
    <div class="at-foot">
      ${sig()}
      <div class="sig-r">${genDate}</div>
    </div>`,
  );
}

// ── statement → Ledger (byAgent) ──────────────────────────────────────────────
// Finance-statement authority: one serif masthead, 78px total, cost in the right
// cell, dotted-lead agent rows onto a shared token axis.
function ledgerCard(data: ShareData, T: TFn): HTMLElement {
  const total = splitAbbrev(data.totalTokens);
  const rows = data.byAgent
    .slice(0, TOP_N)
    .map(
      (s) =>
        `<div class="lg-lrow"><span class="nm">${esc(s.name)}</span>` +
        `<span class="lead"></span><span class="pct">${s.pct}%</span>` +
        `<span class="val tnum"><span class="full">${grouped(s.tokens)}</span>` +
        `<span class="abbr">${fmtTokens(s.tokens)}</span></span></div>`,
    )
    .join("");
  const docNo = data.docNo ? `<div class="no">NO. ${esc(data.docNo)}</div>` : "";
  const genDate = data.genMonthYear
    ? `${T("share.generated")} · ${data.genMonthYear}`
    : T("share.generated");
  return el(
    "shst-card",
    `
    <div class="lg-mast">
      <div><div class="ttl">${T("share.usageStatement")}</div>` +
      `<div class="sub">${T("share.cumulativeForPeriod")}</div></div>
      <div class="meta"><div class="pd">${esc(data.periodLabel)}</div>${docNo}</div>
    </div>
    <div class="lg-hero">
      <div class="cell"><div class="lg-lbl">${T("share.totalTokens")}</div>` +
      `<div class="lg-tok tnum">${total.num}<em>${total.unit}</em></div>` +
      `<div class="lg-tsub">${heroSubline(data, T)}</div></div>
      <div class="cell cost"><div class="lg-lbl">${T("share.estCost")}</div>` +
      `<div class="lg-cost tnum"><i>$</i>${data.totalCostUsd.toFixed(2)}</div>` +
      `<small>${T("share.estUsd")}</small></div>
    </div>
    <div class="lg-ledger">
      <div class="lg-lhead"><span>${T("share.agent")}</span><span>${T("share.share")}</span>` +
      `<span>${T("share.tokens")}</span></div>
      ${rows}
    </div>
    <div class="lg-foot">
      ${sig()}
      <div class="sig-r">${genDate}</div>
    </div>`,
  );
}

// ── diagnostics → Terminal (byAgent + 24h sparkline) ──────────────────────────
// Entirely mono literals (TOTAL_TOKENS / EST_COST_USD / SESSIONS / EOF / column
// headers / `atoll --report`) around the already-localized periodLabel, so it
// takes no TFn — nothing here is translated (per the i18n literal rule).
function terminalCard(data: ShareData): HTMLElement {
  const total = splitAbbrev(data.totalTokens);
  const top = data.byAgent.slice(0, TOP_N);
  const maxTok = top[0]?.tokens ?? 0;
  const rows = top
    .map(
      (s, i) =>
        `<div class="tr${i > 0 ? " dim" : ""}"><span class="g">&gt;</span>` +
        `<span class="nm">${esc(s.name)}</span>` +
        `<span class="barcell"><i style="width:${barPct(s.tokens, maxTok).toFixed(0)}%"></i></span>` +
        `<span class="num tnum">${fmtTokens(s.tokens)}</span>` +
        `<span class="pc tnum">${s.pct}%</span></div>`,
    )
    .join("");

  const maxH = Math.max(0, ...data.hourly);
  const peakIdx = maxH > 0 ? data.hourly.indexOf(maxH) : -1;
  const bars = data.hourly
    .map((v, i) => {
      const h = maxH > 0 ? (v / maxH) * 100 : 0;
      return `<i class="${i === peakIdx ? "pk" : ""}" style="height:${h.toFixed(1)}%"></i>`;
    })
    .join("");

  // Terminal comment: period (localized) + English terminal literals.
  const streak = data.streakDays > 0 ? ` · streak ${data.streakDays}d` : "";
  const peakDay = data.maxDayTokens > 0 ? ` · peak ${fmtTokens(data.maxDayTokens)}/day` : "";
  const genDate = data.genMonthYear ?? "";
  return el(
    "shdx-card",
    `
    <div class="tm-bar"><div class="dots"><i></i><i></i><i></i></div>` +
      `<div class="win">atoll - report - 80x24</div></div>
    <div class="tm-body">
      <div class="tm-cmd"><span class="p">$ </span>atoll <span class="fl">--report</span>` +
      `<span class="cur"></span></div>
      <div class="tm-cmt"># ${esc(data.periodLabel)}${streak}${peakDay}</div>
      <div class="tm-focal">
        <div class="big"><div class="k tnum">TOTAL_TOKENS</div>` +
      `<div class="v tnum">${total.num}<em>${total.unit}</em></div></div>
        <div class="kv">
          <div><div class="k">EST_COST_USD</div><div class="v tnum">${data.totalCostUsd.toFixed(
            2,
          )} <small>est</small></div></div>
          <div><div class="k">SESSIONS</div><div class="v tnum">${data.sessionCount}</div></div>
        </div>
      </div>
      <div class="tm-spark">
        <div class="lbl"><span>HOURLY_LOAD [00-23]</span><span>peak <b>${fmtHour(
          data.peakHour,
        )}</b></span></div>
        <div class="bars">${bars}</div>
      </div>
      <div class="tm-tbl">
        <div class="hd"><span></span><span>agent</span><span>load</span><span>tokens</span><span>%</span></div>
        ${rows}
      </div>
      <div class="tm-foot">
        <div class="eof">- EOF -</div>
        <div class="sig">${RING_MARK}<span class="bn">${BRAND}</span>` +
      `<span class="sig-r">${genDate}</span></div>
      </div>
    </div>`,
  );
}

// ── minimal → Minimal (byAgent) ───────────────────────────────────────────────
// Big enough to read at thumbnail size; the M suffix is the one accent event.
function minimalCard(data: ShareData, T: TFn): HTMLElement {
  const total = splitAbbrev(data.totalTokens);
  const max = data.byAgent[0]?.tokens ?? 0;
  const rows = data.byAgent
    .slice(0, TOP_N)
    .map(
      (s) =>
        `<div class="mn-brow"><span class="nm">${esc(s.name)}</span>` +
        `<span class="tr"><i style="width:${barPct(s.tokens, max).toFixed(1)}%"></i></span>` +
        `<span class="tk tnum">${fmtTokens(s.tokens)}</span></div>`,
    )
    .join("");
  const streak =
    data.streakDays > 0
      ? `<span class="sp"></span>${T("share.streakInline", { n: data.streakDays })}`
      : "";
  const genSuffix = data.genMonthYear ? ` · ${data.genMonthYear}` : "";
  return el(
    "shmn-card",
    `
    <div class="mn-top">
      <div class="tag">${T("share.usageReport")}</div>
      <div class="rt"><b>${money(data.totalCostUsd)}</b> ${T("share.est")} · ${T(
      "share.sessions",
      { n: data.sessionCount },
    )}</div>
    </div>
    <div class="mn-hero">
      <div class="mn-big tnum">${total.num}<span>${total.unit}</span></div>
      <div class="mn-cap"><b>${T("share.tokens")}</b><span class="sp"></span>${esc(
      data.periodLabel,
    )}${streak}</div>
      <div class="mn-split">${rows}</div>
    </div>
    <div class="mn-foot">
      <div class="sig-r">${T("share.peakAt", { hour: fmtHour(data.peakHour) })}</div>
      <div class="sig">${RING_MARK}<span class="bn">${BRAND}${genSuffix}</span></div>
    </div>`,
  );
}

// ── fuel → Sounding (24h lagoon depth profile) ────────────────────────────────
// The 24h load becomes a lagoon depth profile (deepest at peak); the depth layers
// legend at the bottom is the top-2 splits with %. Honours `group` (model/agent).
const SOUND_W = 1000; // depth-chart viewBox width; svg stretches to either aspect
const SOUND_TOP = 8; // surface y at the deepest (peak) hour
const SOUND_BAND = 200; // surface travel from deepest to shallowest
const SOUND_FLOOR = 320; // viewBox height / lagoon floor
const SOUND_LAYER_FILLS = ["#18181B", "#71717A"]; // primary / muted (pinned)

function soundingCard(data: ShareData, T: TFn, group: "model" | "agent"): HTMLElement {
  const total = splitAbbrev(data.totalTokens);
  const maxH = Math.max(0, ...data.hourly);

  // 24 hourly buckets -> a smoothed straight-segment surface line; higher load =
  // deeper lagoon = surface nearer the top. Area fills from the surface to floor.
  const pts = data.hourly.map((v, i) => {
    const x = (i / 23) * SOUND_W;
    const n = maxH > 0 ? v / maxH : 0;
    const y = SOUND_TOP + (1 - n) * SOUND_BAND;
    return { x, y };
  });
  const surfaceD = "M " + pts.map((p) => `${p.x.toFixed(1)},${p.y.toFixed(1)}`).join(" L ");
  const areaD = `${surfaceD} L ${SOUND_W},${SOUND_FLOOR} L 0,${SOUND_FLOOR} Z`;

  // Peak marker on the plotted curve's deepest point (hourly argmax), so the
  // dashed sounding line always lands on the surface it is annotating.
  const peakIdx = maxH > 0 ? data.hourly.indexOf(maxH) : -1;
  const marker =
    peakIdx >= 0
      ? `<line x1="${pts[peakIdx].x.toFixed(1)}" y1="0" x2="${pts[peakIdx].x.toFixed(1)}" ` +
        `y2="290" stroke="#EC4899" stroke-width="2" stroke-dasharray="4 4"/>` +
        `<circle cx="${pts[peakIdx].x.toFixed(1)}" cy="${pts[peakIdx].y.toFixed(1)}" r="5" fill="#EC4899"/>`
      : "";
  const peakLabel =
    peakIdx >= 0 ? `DEEPEST ${fmtHour(peakIdx)} · ${fmtTokens(maxH)}` : "NO PEAK";

  const layerSrc = (group === "agent" ? data.byAgent : data.byModel).slice(0, 2);
  const layers = layerSrc
    .map(
      (s, i) =>
        `<div class="sd-lay"><span class="bar" style="background:${
          SOUND_LAYER_FILLS[i] ?? "#71717A"
        }"></span>` +
        `<span class="t">${esc(s.name)} <b>${s.pct}%</b></span></div>`,
    )
    .join("");
  const genDate = data.genMonthYear ?? "";

  return el(
    "shfl-card",
    `
    <div class="sd-top">
      <div>
        <div class="sd-eyebrow">${T("share.cumulativeUsage")} · ${T("share.lagoonDepth")}</div>
        <div class="sd-big tnum">${total.num}<em>${total.unit}</em></div>
        <div class="sd-sub"><span class="cost tnum">${money(data.totalCostUsd)}</span> ${T(
      "share.est",
    )} · ${T("share.sessions", { n: data.sessionCount })}</div>
      </div>
      <div class="sd-meta"><div class="pd">${esc(data.periodLabel)}</div>` +
      `<div class="peak">${peakLabel}</div></div>
    </div>
    <div class="sd-chart">
      <svg viewBox="0 0 ${SOUND_W} ${SOUND_FLOOR}" preserveAspectRatio="none">
        <defs><linearGradient id="atoll-lagoon" x1="0" y1="0" x2="0" y2="1">` +
      `<stop offset="0" stop-color="#E9E9EB"/><stop offset="1" stop-color="#F7F7F8"/></linearGradient></defs>
        <path d="${areaD}" fill="url(#atoll-lagoon)"/>
        <path d="${surfaceD}" fill="none" stroke="#A1A1AA" stroke-width="2"/>
        ${marker}
      </svg>
    </div>
    <div class="sd-axis"><span>00</span><span>06</span><span>12</span><span>18</span><span>23</span></div>
    <div class="sd-foot">
      <div class="sd-layers">${layers}</div>
      <div class="sig">${RING_MARK}<span class="bn">${BRAND}</span>` +
      `<span class="sig-r">${genDate}</span></div>
    </div>`,
  );
}

// ── wa → Seal (byAgent hairline ledger; ◎ ring seal + serif column) ───────────
function sealCard(data: ShareData, T: TFn): HTMLElement {
  const total = splitAbbrev(data.totalTokens);
  const max = data.byAgent[0]?.tokens ?? 0;
  const rows = data.byAgent
    .slice(0, TOP_N)
    .map(
      (s) =>
        `<div class="sl-srow"><span class="sl-slbl">${esc(s.name)}</span>` +
        `<span class="sl-strack"><span class="sl-sfill" style="width:${barPct(
          s.tokens,
          max,
        ).toFixed(1)}%"></span></span>` +
        `<span class="sl-sval tnum">${fmtTokens(s.tokens)}</span></div>`,
    )
    .join("");
  const vsub = data.genMonthYear ? `<div class="sl-vsub">${data.genMonthYear}</div>` : "";
  const footParts: string[] = [BRAND, T("share.shareReport")];
  if (data.sessionCount > 0) footParts.push(T("share.sessions", { n: data.sessionCount }));
  if (data.streakDays > 0) footParts.push(T("share.streakInline", { n: data.streakDays }));
  return el(
    "shwa-card",
    `
    <div class="sl-rule"></div>
    <div class="sl-side">
      <div class="sl-brand">${RING_MARK}<b>${BRAND}</b></div>
      <div class="sl-vert">${T("share.cumulativeLedger")}</div>
      ${vsub}
    </div>
    <div class="sl-in">
      <div><div class="sl-kicker">${T("share.totalTokens")}</div>` +
      `<div class="sl-period">${esc(data.periodLabel)}</div></div>
      <div class="sl-main">
        <div class="sl-num tnum">${total.num}<em>${total.unit}</em></div>
        <div class="sl-cost tnum">${money(data.totalCostUsd)}<u>${T("share.estUsd")}</u></div>
        <div class="sl-split">${rows}</div>
      </div>
    </div>
    <div class="sl-foot"><span class="sig-r">${footParts.join(" · ")}</span></div>
    <div class="sl-seal">${SEAL_MARK}</div>`,
  );
}
