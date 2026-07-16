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
// byAgent, byModel, daily (for dates) and range are read here. See the comment
// on `byProject` in types.ts.

import "./share.css";
import type { Analytics, AnalyticsRange, Limit } from "./types";
import type { Locale } from "./i18n";
import { tl } from "./i18n";
import { fmtTokens, pctLeft } from "./format";

export type ShareStyle =
  | "statement"
  | "diagnostics"
  | "minimal"
  | "fuel"
  | "island_card"
  | "wa";

export interface ShareSplit {
  name: string;
  tokens: number;
  pct: number; // round(tokens / totalTokens * 100) — share of THIS period's total
}

export interface ShareData {
  totalTokens: number;
  totalCostUsd: number; // always displayed labeled "est."
  byAgent: ShareSplit[]; // from Analytics.byAgent, only tokens>0, sorted desc
  byModel: ShareSplit[]; // from Analytics.byModel, only tokens>0, sorted desc
  agentCount: number; // byAgent.length (tokens>0)
  periodLabel: string; // locale-aware, date-embedded, built here
  quotaNote?: string; // present only when includeQuotaNote && limits given
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

// ── quota note (the ONLY place a subscription-limit % may appear) ────────────

/** One non-main line summarizing current limits, e.g.
 *    en "Now · Claude 5h 28% left · wk 59% left"
 *    zh "目前 · Claude 5h 剩 28% · 週 剩 59%"
 *  "left"/"剩" is appended so it can never be confused with a split share %.
 *  5h stays fixed English; the week window follows locale (wk/週) per concept.
 *  Shows at most the 5h + week window of the FIRST provider that has either. */
function buildQuotaNote(limits: Limit[], locale: Locale): string | undefined {
  const winOf = (l: Limit): "5h" | "week" | null =>
    l.id.endsWith(".5h") ? "5h" : l.id.endsWith(".week") ? "week" : null;
  const relevant = limits.filter((l) => winOf(l) !== null);
  if (relevant.length === 0) return undefined;

  const provider = relevant[0].provider;
  const provLimits = relevant.filter((l) => l.provider === provider);
  const five = provLimits.find((l) => winOf(l) === "5h");
  const week = provLimits.find((l) => winOf(l) === "week");

  const provLabel = provider === "anthropic" ? "Claude" : "Codex"; // brand, fixed
  const now = tl(locale, "share.now");
  const leftWord = tl(locale, "share.left");
  const seg = (winLabel: string, util: number): string => {
    const p = pctLeft(util);
    return locale === "zh-TW"
      ? `${winLabel} ${leftWord} ${p}%`
      : `${winLabel} ${p}% ${leftWord}`;
  };

  const parts: string[] = [];
  if (five) parts.push(seg("5h", five.util));
  if (week) parts.push(seg(locale === "zh-TW" ? "週" : "wk", week.util));
  if (parts.length === 0) return undefined;
  return `${now} · ${provLabel} ${parts.join(" · ")}`;
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
  const quotaNote =
    opts.includeQuotaNote && opts.limits && opts.limits.length > 0
      ? buildQuotaNote(opts.limits, opts.locale)
      : undefined;
  return {
    totalTokens: a.totalTokens,
    totalCostUsd: a.totalCostUsd,
    byAgent,
    byModel,
    agentCount: byAgent.length,
    periodLabel: buildPeriodLabel(a, opts.locale),
    quotaNote,
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

const TOP_N = 5;

/** Build the `-card` root element for a style. `<div class="…-card">` with the
 *  concept markup as innerHTML; the caller sizes/mounts it. */
export function renderShareCard(
  style: ShareStyle,
  data: ShareData,
  locale: Locale,
  opts?: { fuelGroup?: "model" | "agent" },
): HTMLElement {
  const T = (key: Parameters<typeof tl>[1], vars?: Record<string, string | number>) =>
    tl(locale, key, vars);

  switch (style) {
    case "statement":
      return statementCard(data, T);
    case "diagnostics":
      return diagnosticsCard(data, T);
    case "minimal":
      return minimalCard(data, T);
    case "fuel":
      return fuelCard(data, T, opts?.fuelGroup ?? "model");
    case "island_card":
      return islandCard(data, T);
    case "wa":
      return waCard(data, T);
  }
}

type TFn = (key: Parameters<typeof tl>[1], vars?: Record<string, string | number>) => string;

function el(cls: string, html: string): HTMLElement {
  const d = document.createElement("div");
  d.className = cls;
  d.innerHTML = html;
  return d;
}

const BATTERY_SVG = `<svg width="30" height="17" viewBox="0 0 28 16" fill="none">
  <rect x="1" y="2" width="22" height="12" rx="3" stroke="currentColor" stroke-width="1.6"/>
  <rect x="24" y="6" width="2.6" height="4" rx="1" fill="currentColor"/>
  <rect x="3.4" y="4.4" width="12" height="7.2" rx="1.4" fill="currentColor"/></svg>`;

// ── statement 用量結算單 (byAgent) ───────────────────────────────────────────
function statementCard(data: ShareData, T: TFn): HTMLElement {
  const rows = data.byAgent
    .slice(0, TOP_N)
    .map(
      (s) =>
        `<div class="shst-row"><span class="nm">${esc(s.name)}</span><span class="lead"></span>` +
        `<span class="pct">${s.pct}%</span><span class="val">${grouped(s.tokens)}</span></div>`,
    )
    .join("");
  return el(
    "shst-card",
    `
    <div class="shst-top">
      <div class="shst-eyebrow">${T("share.usageStatement")}<small>${T("share.cumulativeForPeriod")}</small></div>
      <div class="shst-period">${esc(data.periodLabel)}</div>
    </div>
    <div class="shst-hero">
      <div class="shst-tokens">
        <div class="lbl">${T("share.totalTokens")}</div>
        <div class="num">${grouped(data.totalTokens)}</div>
        <div class="sub">${T("share.tokensAcrossAgents", { n: data.agentCount })}</div>
      </div>
      <div class="shst-cost">
        <div class="lbl">${T("share.estCost")}</div>
        <div class="num"><i>$</i>${data.totalCostUsd.toFixed(2)}</div>
        <div class="est">${T("share.estUsd")}</div>
      </div>
    </div>
    <div class="shst-table">
      <div class="shst-thead"><span>${T("share.agent")}</span><span>${T("share.tokensShare")}</span></div>
      ${rows}
    </div>
    <div class="shst-foot">
      <div class="shst-brand"><span class="shst-batt">${BATTERY_SVG}</span>TokenBar</div>
      <div class="shst-gen">${T("share.generatedBy")}</div>
    </div>`,
  );
}

// ── diagnostics 系統診斷 (byAgent) ───────────────────────────────────────────
function diagnosticsCard(data: ShareData, T: TFn): HTMLElement {
  const top = data.byAgent.slice(0, TOP_N);
  const rows = top
    .map(
      (s, i) =>
        `<div class="tr${i >= 3 ? " dim" : ""}"><span class="g">&gt;</span><span class="nm">${esc(
          s.name,
        )}</span><span class="bararea"><span class="num">${grouped(
          s.tokens,
        )}</span></span><span class="pc">${s.pct}%</span></div>`,
    )
    .join("");
  return el(
    "shdx-card",
    `
    <div class="shdx-dots"><i></i><i></i><i></i></div>
    <div class="shdx-winlbl">tokenbar :: report</div>
    <div class="shdx-body">
      <div class="shdx-cmd"><span class="p">$ </span>tokenbar --report<span class="cur">&nbsp;</span></div>
      <div class="shdx-comment"># ${esc(data.periodLabel)}</div>
      <div class="shdx-kv">
        <div class="line"><span class="k">TOTAL_TOKENS</span><span class="eq"> = </span><span class="v big">${grouped(
          data.totalTokens,
        )}</span></div>
        <div class="line"><span class="k">EST_COST_USD</span><span class="eq"> = </span><span class="v">${data.totalCostUsd.toFixed(
          2,
        )}</span> <span class="u">${T("share.est")}</span></div>
        <div class="line"><span class="k">AGENTS</span><span class="eq"> = </span><span class="v">${data.agentCount}</span></div>
      </div>
      <div class="shdx-tbl">
        <div class="hd"><span></span><span>${T("share.agent")}</span><span>${T(
      "share.tokens",
    )}</span><span style="text-align:right">${T("share.share")}</span></div>
        ${rows}
      </div>
      <div class="shdx-foot">
        <div class="shdx-eof">EOF</div>
        <div class="shdx-brand">${BATTERY_SVG}TokenBar</div>
      </div>
    </div>`,
  );
}

// ── minimal Linear 極簡 (byAgent) ────────────────────────────────────────────
function minimalCard(data: ShareData, T: TFn): HTMLElement {
  const { num, unit } = splitAbbrev(data.totalTokens);
  const max = data.byAgent[0]?.tokens ?? 0;
  const rows = data.byAgent
    .slice(0, TOP_N)
    .map(
      (s) =>
        `<div class="shmn-brow"><span class="nm">${esc(
          s.name,
        )}</span><span class="shmn-track"><i style="width:${barPct(
          s.tokens,
          max,
        ).toFixed(1)}%"></i></span><span class="tk">${fmtTokens(s.tokens)}</span></div>`,
    )
    .join("");
  return el(
    "shmn-card",
    `
    <div class="shmn-top">
      <div class="shmn-tag">${T("share.usageReport")}</div>
      <div class="shmn-cost"><b>${money(data.totalCostUsd)}</b> ${T("share.est")}</div>
    </div>
    <div class="shmn-hero">
      <div class="shmn-big">${num}<span>${unit}</span></div>
      <div class="shmn-cap"><b>${T("share.tokens")}</b> · ${esc(data.periodLabel)}</div>
      <div class="shmn-split">${rows}</div>
    </div>
    <div class="shmn-foot">
      <div class="shmn-brand"><span class="shmn-batt">${BATTERY_SVG}</span>TokenBar</div>
    </div>`,
  );
}

// ── fuel AI 加油站 (byModel default, byAgent when fuelGroup==="agent") ─────────
function fuelCard(data: ShareData, T: TFn, group: "model" | "agent"): HTMLElement {
  const splits = (group === "agent" ? data.byAgent : data.byModel).slice(0, TOP_N);
  const rows = splits
    .map((s) => {
      const { num, unit } = splitAbbrev(s.tokens);
      return `<div class="shfl-row"><div class="shfl-mdl">${esc(
        s.name.toUpperCase(),
      )}</div><div class="shfl-dots"></div><div class="shfl-nums"><div class="shfl-tok">${num}<em>${unit}</em></div><div class="shfl-usd">${s.pct}%</div></div></div>`;
    })
    .join("");
  const modelsOn = group === "model" ? " class=\"on\"" : "";
  const agentsOn = group === "agent" ? " class=\"on\"" : "";
  return el(
    "shfl-card",
    `
    <div class="shfl-canopy"></div>
    <div class="shfl-grate"></div>
    <div class="shfl-inner">
      <div class="shfl-head">
        <div class="shfl-brand">
          <div class="shfl-batt"><i></i></div>
          <div class="shfl-name"><b>TOKEN STATION</b><span>TOKENBAR FUEL DEPOT</span></div>
        </div>
        <div class="shfl-chip"><span${modelsOn}>${T("share.models")}</span><span${agentsOn}>${T(
      "share.agents",
    )}</span></div>
      </div>
      <div class="shfl-period">${esc(data.periodLabel)}</div>
      <div class="shfl-board">${rows}</div>
      <div class="shfl-total">
        <div class="lbl">${T("share.pumpTotal")}</div>
        <div class="val">${grouped(data.totalTokens)}<i> tok</i> · ${money(
      data.totalCostUsd,
    )}<u>${T("share.est")}</u></div>
      </div>
    </div>`,
  );
}

// ── island_card Liquid 島嶼卡 (byAgent + gem-gradient rows + quotaNote) ────────
const GEM_GRADIENTS = [
  "linear-gradient(90deg,#2fa87e,#37c493)",
  "linear-gradient(90deg,#2b6fb8,#3a8ad8)",
  "linear-gradient(90deg,#7a4fc9,#9566e2)",
  "linear-gradient(90deg,#c2497a,#dd6096)",
  "linear-gradient(90deg,#5b62d4,#767de8)",
];

function islandCard(data: ShareData, T: TFn): HTMLElement {
  const { num, unit } = splitAbbrev(data.totalTokens);
  const max = data.byAgent[0]?.tokens ?? 0;
  const rows = data.byAgent
    .slice(0, TOP_N)
    .map(
      (s, i) =>
        `<div class="shic-brow"><div class="shic-lbl">${esc(
          s.name,
        )}</div><div class="shic-track"><div class="shic-fill" style="width:${barPct(
          s.tokens,
          max,
        ).toFixed(1)}%;background:${GEM_GRADIENTS[i % GEM_GRADIENTS.length]}"></div></div><div class="shic-val">${fmtTokens(
          s.tokens,
        )}</div></div>`,
    )
    .join("");
  const note = data.quotaNote
    ? `<div class="shic-foot">
        <span class="shic-mb g"><i></i></span><span class="shic-mb w"><i></i></span>
        <span class="shic-note">${esc(data.quotaNote)}</span>
      </div>`
    : "";
  return el(
    "shic-card",
    `
    <div class="shic-glow"></div>
    <div class="shic-top">
      <div class="shic-pill"><span class="shic-batt"><i></i></span><b>TokenBar</b></div>
      <div class="shic-period">${esc(data.periodLabel)}</div>
    </div>
    <div class="shic-hero">
      <div class="shic-htag">${T("share.cumulativeUsage")}</div>
      <div class="shic-big"><b>${num}<em>${unit}</em></b><span class="cost">${money(
      data.totalCostUsd,
    )}<u>${T("share.est")}</u></span></div>
    </div>
    <div class="shic-bars">${rows}</div>
    ${note}`,
  );
}

// ── wa 日本古代 (byAgent; 量 seal & CUMULATIVE LEDGER stay untranslated) ───────
function waCard(data: ShareData, T: TFn): HTMLElement {
  const max = data.byAgent[0]?.tokens ?? 0;
  const rows = data.byAgent
    .slice(0, TOP_N)
    .map(
      (s) =>
        `<div class="shwa-srow"><div class="shwa-slbl">${esc(
          s.name,
        )}</div><div class="shwa-strack"><div class="shwa-sfill" style="width:${barPct(
          s.tokens,
          max,
        ).toFixed(1)}%"></div></div><div class="shwa-sval">${fmtTokens(s.tokens)}</div></div>`,
    )
    .join("");
  return el(
    "shwa-card",
    `
    <div class="shwa-rule"></div>
    <div class="shwa-side">
      <div class="shwa-brand"><span class="shwa-batt"><i></i></span><b>TokenBar</b></div>
      <div class="shwa-vert">CUMULATIVE LEDGER</div>
    </div>
    <div class="shwa-inner">
      <div class="shwa-head">
        <div class="shwa-kicker">${T("share.totalTokens")}</div>
        <div class="shwa-period">${esc(data.periodLabel)}</div>
      </div>
      <div class="shwa-main">
        <div class="shwa-num">${grouped(data.totalTokens)}<em>tok</em></div>
        <div class="shwa-cost">${money(data.totalCostUsd)}<u>${T("share.est")}</u></div>
        <div class="shwa-split">${rows}</div>
      </div>
    </div>
    <div class="shwa-foot">TOKENBAR · SHARE REPORT</div>
    <div class="shwa-seal"><span>量</span></div>`,
  );
}
