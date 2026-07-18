// 階段 D 戰報 Share — pure data layer + six share-card renderers.
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
// T-915: the six renderers are ported 1:1 from design/refs/share-redesign-preview
// .html under the repo `.shXX-card` class prefixes, wired to real analytics.

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

/** One structured quota row for the island_card gauge. `util` is the USED % (0-100)
 *  — the ONE sanctioned exposure of a subscription %, deliberately "used" (opposite
 *  the app's `% left` convention). `label` is "Brand · window" (e.g. "Claude · 5h"). */
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
  hourly: number[]; // ← Analytics.hourly (24 buckets), diagnostics sparkline
  peakHour: number; // ← Analytics.records.maxHour.hour (0-23), rendered "HH:00"
  byAgent: ShareSplit[]; // from Analytics.byAgent, only tokens>0, sorted desc
  byModel: ShareSplit[]; // from Analytics.byModel, only tokens>0, sorted desc
  agentCount: number; // byAgent.length (tokens>0)
  periodLabel: string; // locale-aware, date-embedded, built here
  genMonthYear?: string; // uppercase "MON YYYY" from the period's last day; omit if unknown
  docNo?: string; // "TB-YYYY-MMDD" from the period's last day (statement doc number)
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
 *  both the uppercase "JUL 2026" signature date and the "TB-YYYY-MMDD" doc number,
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

/** Statement doc number "TB-2026-0718" from a date (zero-padded month+day). */
function fmtDocNo(p: Ymd): string {
  const mm = String(p.m).padStart(2, "0");
  const dd = String(p.d).padStart(2, "0");
  return `TB-${p.y}-${mm}${dd}`;
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
        return statementCard(data, T);
      case "diagnostics":
        return diagnosticsCard(data);
      case "minimal":
        return minimalCard(data, T);
      case "fuel":
        return fuelCard(data, T, opts?.fuelGroup ?? "model");
      case "island_card":
        return islandCard(data, T);
      case "wa":
        return waCard(data, T);
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

/** The battery mark, at the requested pixel box (viewBox is fixed). */
function battSvg(w = 26, h = 15): string {
  return (
    `<svg class="batt" width="${w}" height="${h}" viewBox="0 0 28 16" fill="none">` +
    `<rect x="1" y="2" width="22" height="12" rx="3" stroke="currentColor" stroke-width="1.6"/>` +
    `<rect x="24" y="6" width="2.6" height="4" rx="1" fill="currentColor"/>` +
    `<rect x="3.4" y="4.4" width="12" height="7.2" rx="1.4" fill="currentColor"/></svg>`
  );
}

/** Unified signature block: battery mark + "TokenBar". The date suffix (mono
 *  "JUL 2026") lives in each card's `.sig-r` slot, composed per-template. */
function sig(w = 26, h = 15): string {
  return `<div class="sig"><span class="mk">${battSvg(w, h)}</span><span class="bn">TokenBar</span></div>`;
}

/** Hero subline shared by statement: "across N agents · K sessions · streak Nd ·
 *  peak X/day", each segment dropped when its value is absent. */
function heroSubline(data: ShareData, T: TFn): string {
  const parts: string[] = [T("share.acrossAgents", { n: data.agentCount })];
  if (data.sessionCount > 0) parts.push(T("share.sessions", { n: data.sessionCount }));
  if (data.streakDays > 0) parts.push(T("share.streakInline", { n: data.streakDays }));
  if (data.maxDayTokens > 0)
    parts.push(T("share.peakPerDay", { tokens: fmtTokens(data.maxDayTokens) }));
  return parts.join(" · ");
}

// ── statement 用量結算單 (byAgent) ───────────────────────────────────────────
function statementCard(data: ShareData, T: TFn): HTMLElement {
  const total = splitAbbrev(data.totalTokens);
  const rows = data.byAgent
    .slice(0, TOP_N)
    .map(
      (s) =>
        `<div class="st-lrow"><span class="nm">${esc(s.name)}</span>` +
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
    <div class="st-mast">
      <div><div class="ttl">${T("share.usageStatement")}</div><div class="sub">${T(
      "share.cumulativeForPeriod",
    )}</div></div>
      <div class="meta"><div class="pd">${esc(data.periodLabel)}</div>${docNo}</div>
    </div>
    <div class="st-hero">
      <div class="cell"><div class="st-lbl">${T("share.totalTokens")}</div>` +
      `<div class="st-tok tnum">${total.num}<em>${total.unit}</em></div>` +
      `<div class="st-tsub">${heroSubline(data, T)}</div></div>
      <div class="cell cost"><div class="st-lbl">${T("share.estCost")}</div>` +
      `<div class="st-cost tnum"><i>$</i>${data.totalCostUsd.toFixed(2)}</div>` +
      `<small>${T("share.estUsd")}</small></div>
    </div>
    <div class="st-ledger">
      <div class="st-lhead"><span>${T("share.agent")}</span><span>${T(
      "share.share",
    )}</span><span>${T("share.tokens")}</span></div>
      ${rows}
    </div>
    <div class="st-foot">
      ${sig()}
      <div class="sig-r">${genDate}</div>
    </div>`,
  );
}

// ── diagnostics 系統診斷 (byAgent + 24h sparkline) ────────────────────────────
// The terminal card is entirely mono literals (TOTAL_TOKENS / EST_COST_USD /
// SESSIONS / EOF / column headers) around the already-localized periodLabel, so
// it takes no TFn — nothing here is translated (per the i18n literal rule).
function diagnosticsCard(data: ShareData): HTMLElement {
  const total = splitAbbrev(data.totalTokens);
  const top = data.byAgent.slice(0, TOP_N);
  const maxTok = top[0]?.tokens ?? 0;
  const rows = top
    .map(
      (s, i) =>
        `<div class="tr${i >= 3 ? " dim" : ""}"><span class="g">&gt;</span>` +
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
    <div class="dx-bar"><div class="dots"><i></i><i></i><i></i></div>` +
      `<div class="win">tokenbar — report — 80×24</div></div>
    <div class="dx-body">
      <div class="dx-cmd"><span class="p">$ </span>tokenbar <span class="fl">--report</span>` +
      `<span class="cur"></span></div>
      <div class="dx-cmt"># ${esc(data.periodLabel)}${streak}${peakDay}</div>
      <div class="dx-focal">
        <div class="big"><div class="k tnum">TOTAL_TOKENS</div>` +
      `<div class="v tnum">${total.num}<em>${total.unit}</em></div></div>
        <div class="kv">
          <div><div class="k">EST_COST_USD</div><div class="v tnum">${data.totalCostUsd.toFixed(
            2,
          )} <small>est</small></div></div>
          <div><div class="k">SESSIONS</div><div class="v tnum">${data.sessionCount}</div></div>
        </div>
      </div>
      <div class="dx-spark">
        <div class="lbl"><span>HOURLY_LOAD [00–23]</span><span>peak <b>${fmtHour(
          data.peakHour,
        )}</b></span></div>
        <div class="bars">${bars}</div>
      </div>
      <div class="dx-tbl">
        <div class="hd"><span></span><span>agent</span><span>load</span><span>tokens</span><span>%</span></div>
        ${rows}
      </div>
      <div class="dx-foot">
        <div class="eof">— EOF —</div>
        <div class="sig"><span class="mk">${battSvg(22, 13)}</span><span class="bn">TokenBar</span>` +
      `<span class="sig-r">${genDate}</span></div>
      </div>
    </div>`,
  );
}

// ── minimal 極簡 (byAgent) ────────────────────────────────────────────────────
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
  const streak = data.streakDays > 0 ? `<span class="dot"></span>${T("share.streakInline", {
    n: data.streakDays,
  })}` : "";
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
      <div class="mn-cap"><b>${T("share.tokens")}</b><span class="dot"></span>${esc(
      data.periodLabel,
    )}${streak}</div>
      <div class="mn-split">${rows}</div>
    </div>
    <div class="mn-foot">
      <div class="sig-r">${T("share.peakAt", { hour: fmtHour(data.peakHour) })}</div>
      <div class="sig"><span class="mk">${battSvg(24, 14)}</span>` +
      `<span class="bn">TokenBar${genSuffix}</span></div>
    </div>`,
  );
}

// ── fuel AI 加油站 (byModel default, byAgent when fuelGroup==="agent") ─────────
function fuelCard(data: ShareData, T: TFn, group: "model" | "agent"): HTMLElement {
  const splits = (group === "agent" ? data.byAgent : data.byModel).slice(0, 4);
  const rows = splits
    .map((s, i) => {
      const t = splitAbbrev(s.tokens);
      const gr = String(i + 1).padStart(2, "0");
      return (
        `<div class="fl-row"><span class="gr">${gr}</span>` +
        `<span class="fl-mdl">${esc(s.name.toUpperCase())}</span><span class="fl-dots"></span>` +
        `<span class="fl-tok tnum">${t.num}<em>${t.unit}</em></span>` +
        `<span class="fl-pct tnum">${s.pct}%</span></div>`
      );
    })
    .join("");
  const modelsOn = group === "model" ? " class=\"on\"" : "";
  const agentsOn = group === "agent" ? " class=\"on\"" : "";
  const total = splitAbbrev(data.totalTokens);
  const depot = data.genMonthYear
    ? `TOKENBAR FUEL DEPOT · ${data.genMonthYear}`
    : "TOKENBAR FUEL DEPOT";
  const unit = `${T("share.tokens").toUpperCase()} · ${esc(data.periodLabel.toUpperCase())}`;
  const footParts: string[] = [];
  if (data.sessionCount > 0) footParts.push(T("share.sessions", { n: data.sessionCount }));
  if (data.streakDays > 0) footParts.push(T("share.streakInline", { n: data.streakDays }));
  return el(
    "shfl-card",
    `
    <div class="fl-canopy"></div>
    <div class="fl-grate"></div>
    <div class="fl-in">
      <div class="fl-head">
        <div class="fl-brand"><div class="fl-pump"><i></i></div>` +
      `<div class="fl-nm"><b>TOKEN STATION</b><span>${depot}</span></div></div>
        <div class="fl-chip"><span${modelsOn}>${T("share.models")}</span>` +
      `<span${agentsOn}>${T("share.agents")}</span></div>
      </div>
      <div class="fl-display">
        <div class="grp"><div class="k">${T("share.fuelDispensed")}</div>` +
      `<div class="v tnum">${total.num}<em>${total.unit}</em></div><div class="u">${unit}</div></div>
        <div class="grp cost"><div class="k">${T("share.totalSale")}</div>` +
      `<div class="v tnum">${money(data.totalCostUsd)}</div>` +
      `<div class="u">${T("share.estUsd").toUpperCase()}</div></div>
      </div>
      <div class="fl-board">${rows}</div>
      <div class="fl-foot">
        <div class="sig-r">${footParts.join(" · ")}</div>
        <div class="sig"><span class="mk">${battSvg(22, 13)}</span><span class="bn">TokenBar</span></div>
      </div>
    </div>`,
  );
}

// ── island_card 島嶼卡 (byAgent hero + quota gauge — the ONLY quota exposure) ──
const QUOTA_FILLS = ["#18181B", "#52525B", "#71717A"];

function islandCard(data: ShareData, T: TFn): HTMLElement {
  const total = splitAbbrev(data.totalTokens);
  const gauge = data.quotaGauge ?? [];
  const qrows = gauge
    .map((g, i) => {
      const [brand, ...rest] = g.label.split(" · ");
      const desc = rest.join(" · ");
      const w = Math.max(0, Math.min(100, Math.round(g.util)));
      return (
        `<div class="ic-qrow"><span class="ic-qlbl">${esc(brand)}` +
        (desc ? ` <small>· ${esc(desc)}</small>` : "") +
        `</span><span class="ic-track"><span class="ic-fill" style="width:${w}%;background:${
          QUOTA_FILLS[i % QUOTA_FILLS.length]
        }"></span></span>` +
        `<span class="ic-qval tnum">${w}%<small> ${T("share.used")}</small></span></div>`
      );
    })
    .join("");
  const quota = gauge.length
    ? `<div class="ic-quota"><div class="ic-qhead">${T(
        "share.quotaUsedCycle",
      )}</div>${qrows}</div>`
    : `<div class="ic-quota"></div>`;
  const genDate = data.genMonthYear ?? "";
  const streak = data.streakDays > 0 ? ` · streak ${data.streakDays}d` : "";
  return el(
    "shic-card",
    `
    <div class="ic-glow"></div>
    <div class="ic-top">
      <div class="ic-pill"><span class="pb"><i></i></span><b>TokenBar</b>` +
      `<span class="sep"></span><span class="liv">LIVE</span></div>
      <div class="ic-period">${esc(data.periodLabel)}</div>
    </div>
    <div class="ic-hero">
      <div class="ic-htag">${T("share.cumulativeUsage")}</div>
      <div class="ic-big">
        <b class="tnum">${total.num}<em>${total.unit}</em></b>
        <span class="cost tnum">${money(data.totalCostUsd)}<u>${T("share.est")}</u></span>
        <span class="sess"><div class="n tnum">${data.sessionCount}</div>` +
      `<div class="l">${T("share.sessionsLabel")}</div></span>
      </div>
    </div>
    ${quota}
    <div class="ic-foot">
      ${sig(24, 14)}
      <div class="sig-r">${genDate}${streak}</div>
    </div>`,
  );
}

// ── wa 和 (byAgent hairline ledger; 量 seal & serif column stay) ───────────────
function waCard(data: ShareData, T: TFn): HTMLElement {
  const total = splitAbbrev(data.totalTokens);
  const max = data.byAgent[0]?.tokens ?? 0;
  const rows = data.byAgent
    .slice(0, TOP_N)
    .map(
      (s) =>
        `<div class="wa-srow"><span class="wa-slbl">${esc(s.name)}</span>` +
        `<span class="wa-strack"><span class="wa-sfill" style="width:${barPct(
          s.tokens,
          max,
        ).toFixed(1)}%"></span></span>` +
        `<span class="wa-sval tnum">${fmtTokens(s.tokens)}</span></div>`,
    )
    .join("");
  const vsub = data.genMonthYear ? `<div class="wa-vsub">${data.genMonthYear}</div>` : "";
  const footParts: string[] = ["TokenBar", T("share.shareReport")];
  if (data.sessionCount > 0) footParts.push(T("share.sessions", { n: data.sessionCount }));
  if (data.streakDays > 0) footParts.push(T("share.streakInline", { n: data.streakDays }));
  return el(
    "shwa-card",
    `
    <div class="wa-rule"></div>
    <div class="wa-side">
      <div class="wa-brand"><span class="pb"><i></i></span><b>TokenBar</b></div>
      <div class="wa-vert">${T("share.cumulativeLedger")}</div>
      ${vsub}
    </div>
    <div class="wa-in">
      <div class="wa-head"><div class="wa-kicker">${T("share.totalTokens")}</div>` +
      `<div class="wa-period">${esc(data.periodLabel)}</div></div>
      <div class="wa-main">
        <div class="wa-num tnum">${total.num}<em>${total.unit}</em></div>
        <div class="wa-cost tnum">${money(data.totalCostUsd)}<u>${T("share.estUsd")}</u></div>
        <div class="wa-split">${rows}</div>
      </div>
    </div>
    <div class="wa-foot"><span class="sig-r">${footParts.join(" · ")}</span></div>
    <div class="wa-seal"><span>量</span></div>`,
  );
}
