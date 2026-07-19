// Layer ③ analytics (UX Spec v3 §11): stat tiles, charts, breakdown.

import type { Analytics, AnalyticsRange, DayPoint, KindCount, ProjectCount } from "./types";
import { fmtTokens, fmtUsd } from "./format";
import { seriesColor } from "./colors";
import { t } from "./i18n";

/** Escape before interpolating a project name (derived from a local folder
 *  path) into innerHTML. */
function esc(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

// T-ui-301 二鏡頭 IA: the analytics pane is now one scrolling column with two
// stacked lenses — Trends (何時) over Breakdown (去哪) — rendered together every
// time. There is no more sub-tab switcher; the old overview/hourly split became
// the Trends granularity toggle, and the old share/stats folded into Breakdown.
export type Granularity = "daily" | "hourly";
export type Metric = "tokens" | "price";
export type Group = "model" | "agent";

/** Whole-number "share of range total" percent, guarding a zero denominator. */
export function sharePct(value: number, total: number): number {
  return total > 0 ? Math.round((value / total) * 100) : 0;
}

/** The dual-label a bar carries (階段 C readability): absolute tokens plus its
 *  share of the selected range's total, e.g. "1.4M · 17%". */
export function shareLabel(value: number, total: number): string {
  return `${fmtTokens(value)} · ${sharePct(value, total)}%`;
}

/** Y-axis tick values for a bar chart whose plot maxes at `max`: 0 / half / max
 *  (or just [0] for an empty chart). Pure so the tick set is unit-testable; the
 *  caller formats each value with fmtTokens or fmtUsd per the active metric. */
export function axisTicks(max: number): number[] {
  if (!(max > 0)) return [0];
  return [0, max / 2, max];
}

/** M/D label for a YYYY-MM-DD bucket, locale-free like the "07-16" convention
 *  elsewhere but slashed ("07/14") so it reads as a date on the x-axis. */
function mdLabel(date: string): string {
  return date.slice(5).replace("-", "/");
}

/**
 * Interior x-axis date ticks for the daily overview chart — the labels *between*
 * the fixed "30d ago"/"today" endpoints, so a month/week no longer leaves ~28
 * unlabeled bars to count by hand (same fix as the hourly 6h/12h/18h ticks).
 * Month spaces ~4 evenly; week labels alternate days. Guards a small `n` (< 4)
 * by emitting nothing, and never places an interior tick close enough to an
 * endpoint to collide. Returns bar indices + labels; pure, so tick placement is
 * unit-testable.
 */
export function dailyXTicks(dates: string[], range: AnalyticsRange): { i: number; label: string }[] {
  const n = dates.length;
  if (n < 4) return [];
  const step = range === "month" ? Math.max(1, Math.round(n / 5)) : 2;
  const out: { i: number; label: string }[] = [];
  // Stop early enough that the last interior tick can't crowd the "today" end.
  const last = n - 1 - Math.ceil(step / 2);
  for (let i = step; i <= last; i += step) out.push({ i, label: mdLabel(dates[i]) });
  return out;
}

/**
 * The start-date annotation for a short "month" history, or null when none is
 * warranted. Pure so the condition (month only, and only when local logs don't
 * reach the nominal window start) is unit-testable.
 */
export function monthStartNote(a: Analytics): string | null {
  if (a.range !== "month" || a.daily.length === 0) return null;
  if (!a.rangeStartDay || a.rangeStartDay === a.daily[0].date) return null;
  return a.rangeStartDay;
}

export interface AnalyticsOpts {
  metric: Metric;
  group: Group;
  granularity: Granularity;
}

// ── activity heatmap (階段 C+, overview · month only) ─────────────────────

export interface HeatCell {
  date: string; // YYYY-MM-DD
  weekdayRow: number; // Mon=0 … Sun=6
  weekCol: number; // 0-based week column
  intensity: number; // 0..1, day tokens ÷ busiest day (0 when the range is empty)
}
export interface HeatGrid {
  cells: HeatCell[];
  weeks: number; // number of week columns
}

/** Weekday of a YYYY-MM-DD bucket, Mon=0 … Sun=6. Parsed in local time so it
 *  matches the backend's local-timezone day bucketing (F-15). */
function weekdayMon(date: string): number {
  const d = new Date(date + "T00:00:00"); // local (no trailing Z)
  return (d.getDay() + 6) % 7; // JS Sun=0..Sat=6 → Mon=0..Sun=6
}

/**
 * GitHub-style calendar cells from the daily buckets (§ activity heatmap).
 * Row = weekday (Mon top), column = week index. The first day sits at its own
 * weekday, so a month that doesn't start on Monday leaves the leading slots of
 * column 0 empty (they simply aren't emitted). Intensity is the day's tokens as
 * a fraction of the busiest day — 0 for an empty day, and every cell 0 when the
 * whole range is empty. Pure, so the alignment is unit-testable.
 */
export function heatCells(daily: DayPoint[]): HeatGrid {
  if (daily.length === 0) return { cells: [], weeks: 0 };
  const totals = daily.map((d) => Object.values(d.byAgent).reduce((s, v) => s + v, 0));
  const max = Math.max(0, ...totals);
  const lead = weekdayMon(daily[0].date);
  let weeks = 0;
  const cells = daily.map((d, i) => {
    const slot = lead + i;
    const weekCol = Math.floor(slot / 7);
    if (weekCol + 1 > weeks) weeks = weekCol + 1;
    return {
      date: d.date,
      weekdayRow: slot % 7,
      weekCol,
      intensity: max > 0 ? totals[i] / max : 0,
    };
  });
  return { cells, weeks };
}

/** Fixed short month labels (kept English like the island's short labels, so a
 *  zh-TW machine can never leak a localized month into the chart axis). */
const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
/** Weekday axis: only Mon/Wed/Fri/Sun get a label (GitHub convention). */
const WEEKDAY_AXIS = ["Mon", "", "Wed", "", "Fri", "", "Sun"];

function heatmap(a: Analytics): string {
  const { cells, weeks } = heatCells(a.daily);
  if (cells.length === 0) return "";
  const today = cells[cells.length - 1].date;
  const totalByDate = new Map(
    a.daily.map((d) => [d.date, Object.values(d.byAgent).reduce((s, v) => s + v, 0)]),
  );

  const cellDivs = cells
    .map((c) => {
      const level = c.intensity === 0 ? 0 : Math.min(4, Math.ceil(c.intensity * 4));
      const tot = totalByDate.get(c.date) ?? 0;
      return `<div class="hm-cell hm-l${level}${c.date === today ? " hm-today" : ""}" style="grid-row:${c.weekdayRow + 1};grid-column:${
        c.weekCol + 1
      }" title="${c.date} · ${fmtTokens(tot)}"></div>`;
    })
    .join("");

  // Month labels along the column tops: at each column where the month of its
  // earliest day changes.
  const colMonth = new Map<number, string>();
  for (const c of cells) if (!colMonth.has(c.weekCol)) colMonth.set(c.weekCol, c.date.slice(0, 7));
  let prev = "";
  let months = "";
  for (let col = 0; col < weeks; col++) {
    const ym = colMonth.get(col);
    if (ym && ym !== prev) {
      months += `<span class="hm-mo" style="grid-column:${col + 1}">${
        MONTHS[Number(ym.slice(5, 7)) - 1] ?? ""
      }</span>`;
      prev = ym;
    }
  }

  const wds = WEEKDAY_AXIS.map((w, r) =>
    w ? `<span class="hm-wd" style="grid-row:${r + 1}">${w}</span>` : "",
  ).join("");

  const legend =
    `<div class="hm-legend"><span>${t("analytics.less")}</span>` +
    [0, 1, 2, 3, 4].map((l) => `<i class="hm-cell hm-l${l}"></i>`).join("") +
    `<span>${t("analytics.more")}</span></div>`;

  return `<div class="hm" style="--hm-weeks:${weeks}">
    <div class="hm-months">${months}</div>
    <div class="hm-wds">${wds}</div>
    <div class="hm-grid">${cellDivs}</div>
    ${legend}
  </div>`;
}

// ── activity-type donut + project bars (階段 C+, Breakdown) ────────────────

// T-ui-301 grayscale ramp (SPEC §2): donut/composition/projects stay fully
// grayscale — the single magenta per lens is reserved (Trends=today bar,
// Breakdown=#1 row underline). The mockup names these --g5..--g1/--dim; those
// are local aliases, so we map them onto the real :root ink scale (darkest →
// lightest), theme-following in both light and dark.
const GRAY_RAMP = [
  "var(--ink-900)",
  "var(--ink-700)",
  "var(--ink-500)",
  "var(--ink-400)",
  "var(--faint)",
  "var(--ink-300)",
];
/** One rung of the grayscale ramp, clamped so overflow kinds reuse the lightest
 *  ink rather than wrapping back to a dark (which would misread as emphasis). */
function grayColor(index: number): string {
  return GRAY_RAMP[Math.min(index, GRAY_RAMP.length - 1)];
}
function kindLabel(kind: string): string {
  switch (kind) {
    case "edit":
      return t("analytics.kindEdit");
    case "read":
      return t("analytics.kindRead");
    case "search":
      return t("analytics.kindSearch");
    case "run":
      return t("analytics.kindRun");
    case "web":
      return t("analytics.kindWeb");
    case "agent":
      return t("analytics.kindAgent");
    case "mcp":
      return t("analytics.kindMcp");
    case "other":
      return t("analytics.kindOther");
    default:
      return kind;
  }
}

/** Activity-type donut (C1 `.donutsec`): grayscale ring + right-hand legend with
 *  %. Empty when nothing is classifiable — the caller then omits the section. */
function donutGray(byKind: KindCount[]): string {
  if (byKind.length === 0) return "";
  const total = byKind.reduce((s, k) => s + k.tokens, 0);
  if (total <= 0) return "";
  const radius = 20;
  const circumference = 2 * Math.PI * radius;
  const gap = 2;
  let offset = 0;
  const arcs: string[] = [];
  const legend = byKind
    .map((k, index) => {
      const col = grayColor(index);
      const share = k.tokens / total;
      const dash = Math.max(0, share * circumference - gap);
      arcs.push(`<circle cx="28" cy="28" r="${radius}" fill="none" style="stroke:${col}" stroke-width="7"
        stroke-dasharray="${dash} ${circumference - dash}" stroke-dashoffset="${-offset}"
        transform="rotate(-90 28 28)"/>`);
      offset += share * circumference;
      return `<span><i style="background:${col}"></i>${kindLabel(k.kind)} <b>${sharePct(
        k.tokens,
        total,
      )}%</b></span>`;
    })
    .join("");
  return `<div class="donutsec">
    <svg width="92" height="92" viewBox="0 0 56 56" role="img" aria-label="${fmtTokens(total)} ${t("analytics.tokens")}">
      <circle cx="28" cy="28" r="${radius}" fill="none" style="stroke:var(--donut-ring)" stroke-width="7"/>
      ${arcs.join("")}
    </svg>
    <div class="legend">${legend}</div>
  </div>`;
}

/** Ranked horizontal rows (C1 `.rows`/`.row`/`.track`). Grayscale fills come from
 *  T-302's `.track i`; the lone Breakdown magenta is the `.row.top` underline, so
 *  only the #1 row gets `.top` and only when `topAccent`. `price` reads the cost
 *  series and labels with fmtUsd. Empty record → "". */
function rankRows(rec: Record<string, number>, price: boolean, topAccent: boolean): string {
  const entries = Object.entries(rec).sort((a, b) => b[1] - a[1]);
  if (entries.length === 0) return "";
  const max = Math.max(price ? 1e-9 : 1, ...entries.map((e) => e[1]));
  // Share-of-total denominator = the sum of every row = this grouping's range
  // total, so each label reads "value · % of range" (§ readability).
  const total = entries.reduce((s, [, v]) => s + v, 0);
  const label = (v: number) =>
    price ? `${fmtUsd(v)} · ${sharePct(v, total)}%` : shareLabel(v, total);
  return `<div class="rows">${entries
    .map(
      ([k, v], i) => `
      <div class="row${topAccent && i === 0 ? " top" : ""}">
        <div class="meta"><span class="nm" title="${esc(k)}">${esc(k)}</span><span class="vl">${label(v)}</span></div>
        <div class="track"><i style="width:${(v / max) * 100}%"></i></div>
      </div>`,
    )
    .join("")}</div>`;
}

/** Per-project ranked rows (grayscale, never the accent — `.row.top` is reserved
 *  for the model/agent #1). Empty when there is no project data. */
function projectRows(byProject: ProjectCount[]): string {
  if (byProject.length === 0) return "";
  const total = byProject.reduce((s, p) => s + p.tokens, 0);
  const max = Math.max(1, ...byProject.map((p) => p.tokens));
  return `<div class="rows">${byProject
    .map((p) => {
      const name = p.name === "__other__" ? t("analytics.projectsOther") : p.name;
      return `<div class="row"><div class="meta"><span class="nm" title="${esc(name)}">${esc(
        name,
      )}</span><span class="vl">${shareLabel(p.tokens, total)}</span></div><div class="track"><i style="width:${
        (p.tokens / max) * 100
      }%"></i></div></div>`;
    })
    .join("")}</div>`;
}

/** Token-composition segmented bar (C1 `.comp`): four grayscale segments +
 *  wrapping legend with %. Empty (no token breakdown yet) → "". */
function compositionBar(a: Analytics): string {
  const b = a.breakdown;
  const total = b.input + b.cached + b.output + b.reasoning;
  if (total <= 0) return "";
  const segs: [string, number, string][] = [
    [t("analytics.input"), b.input, "var(--ink-900)"],
    [t("analytics.cached"), b.cached, "var(--ink-700)"],
    [t("analytics.output"), b.output, "var(--ink-500)"],
    [t("analytics.reasoning"), b.reasoning, "var(--ink-400)"],
  ];
  const bar = segs
    .map(([, v, c]) => `<i style="width:${(v / total) * 100}%;background:${c}"></i>`)
    .join("");
  const legend = segs
    .map(([lbl, v, c]) => `<span><i style="background:${c}"></i>${lbl} <b>${sharePct(v, total)}%</b></span>`)
    .join("");
  return `<div class="comp">
    <span class="lbl">${t("analytics.compositionTitle")}</span>
    <div class="compbar">${bar}</div>
    <div class="complegend">${legend}</div>
  </div>`;
}

/** Value shown on the y-axis for one day, honouring the metric. */
function dayTotal(d: DayPoint, opts: AnalyticsOpts): number {
  return opts.metric === "price"
    ? d.costUsd
    : Object.values(opts.group === "model" ? d.byModel : d.byAgent).reduce((s, v) => s + v, 0);
}

function stackedDaily(a: Analytics, opts: AnalyticsOpts): string {
  // padL reserves a left gutter for the y-axis (values + gridlines); the plot
  // area is inset by it so bars never overlap the axis. viewBox is unchanged.
  const W = 320, padL = 30, plotH = 92, H = 112, gap = 2;
  const plotW = W - padL;
  // Drop leading empty days so a month backed by a few days of logs doesn't
  // render a wall of blank bars; the x-axis then starts at the first active day
  // (which matches the backend's range_start_day annotation).
  const allTotals = a.daily.map((d) => dayTotal(d, opts));
  let fi = allTotals.findIndex((v) => v > 0);
  if (fi < 0) fi = 0;
  const daily = a.daily.slice(fi);
  const totals = allTotals.slice(fi);
  const n = daily.length;
  const bw = (plotW - gap * Math.max(0, n - 1)) / Math.max(1, n);

  const max = Math.max(1, ...totals);
  const scale = plotH / max;
  const price = opts.metric === "price";
  // Denominator for the "share of range total" hover labels (§ readability).
  const rangeTotal = totals.reduce((s, v) => s + v, 0);
  const fmtY = (v: number) => (price ? fmtUsd(v) : fmtTokens(v));
  const fmtDayVal = (v: number) => (price ? fmtUsd(v) : shareLabel(v, rangeTotal));

  // Y axis: faint gridlines across the plot + 0/half/max labels in the gutter.
  const yaxis = axisTicks(max)
    .map((v) => {
      const y = plotH - v * scale;
      // Clamp the label baseline so the top (max) tick isn't clipped at y≈0.
      const ty = Math.max(y + 3, 8);
      return `<line class="grid" x1="${padL}" y1="${y}" x2="${W}" y2="${y}"/><text x="${
        padL - 4
      }" y="${ty}" class="axis axis-y" text-anchor="end">${fmtY(v)}</text>`;
    })
    .join("");

  const bars = daily
    .map((d, i) => {
      const x = padL + i * (bw + gap);
      const h = totals[i] * scale;
      // Fill is set by class in styles.css (theme-following): today = accent,
      // a "strong" day = heavy ink, else a dim/weak ink.
      const isToday = i === n - 1;
      const cls = isToday ? " is-today" : totals[i] / max > 0.6 ? " is-strong" : "";
      // label doubles as the <title> fallback and the custom-tooltip payload.
      const label = `${mdLabel(d.date)} · ${fmtDayVal(totals[i])}`;
      return `<rect class="daily-bar${cls}" x="${x}" y="${plotH - h}" width="${bw}" height="${Math.max(
        0,
        h,
      )}" rx="1" data-tip="${esc(label)}"><title>${label}</title></rect>`;
    })
    .join("");

  // Interior date ticks between the fixed endpoints (guards small n).
  const xmids = dailyXTicks(daily.map((d) => d.date), a.range)
    .map(({ i, label }) => {
      const x = padL + i * (bw + gap) + bw / 2;
      return `<text x="${x}" y="${H - 1}" class="axis" text-anchor="middle">${label}</text>`;
    })
    .join("");

  // Left endpoint is range-aware: "30d ago" was a month-ism that read wrong on
  // week/today — those show the first plotted day's M/D instead.
  const leftLabel = a.range === "month" ? "30d ago" : (daily[0]?.date.slice(5).replace("-", "/") ?? "");
  const xlabels = `<text x="${padL}" y="${H - 1}" class="axis">${leftLabel}</text>${xmids}<text x="${W}" y="${
    H - 1
  }" class="axis axis-today" text-anchor="end">today</text>`;

  return `<svg viewBox="0 0 ${W} ${H}" class="chart daily-chart">${yaxis}${bars}${xlabels}</svg>`;
}

function hourly(a: Analytics, opts: AnalyticsOpts): string {
  // padL reserves a left gutter for the y-axis; the 24-slot plot is inset by it.
  const W = 320, H = 182, padB = 18, padT = 8, padL = 30;
  const plotW = W - padL;
  // Price mode reads the per-hour cost series and normalizes on its own max, so
  // the shape reflects spend rather than raw tokens.
  const price = opts.metric === "price";
  const data = price ? a.hourlyCost : a.hourly;
  const fmtVal = (v: number) => (price ? fmtUsd(v) : fmtTokens(v));
  const max = Math.max(price ? 1e-9 : 1, ...data);
  const bw = (plotW / 24) * 0.6;
  const scale = (H - padB - padT) / max;
  const plotBottom = H - padB;

  // Y axis: faint gridlines + 0/half/max labels in the gutter.
  const yaxis = axisTicks(max)
    .map((v) => {
      const y = plotBottom - v * scale;
      return `<line class="grid" x1="${padL}" y1="${y}" x2="${W}" y2="${y}"/><text x="${
        padL - 4
      }" y="${y + 3}" class="axis axis-y" text-anchor="end">${fmtVal(v)}</text>`;
    })
    .join("");

  const bars = data
    .map((v, i) => {
      const cx = padL + (i + 0.5) * (plotW / 24);
      const h = v * scale;
      const label = `${i}:00 · ${fmtVal(v)}`;
      return `<rect x="${cx - bw / 2}" y="${plotBottom - h}" width="${bw}" height="${h}" rx="1" style="fill:${seriesColor(
        3,
      )}" data-tip="${esc(label)}"><title>${label}</title></rect>`;
    })
    .join("");
  // Mid-axis labels every 6h, centered under their bar — the two endpoints
  // alone left 22 unlabeled slots to count by hand.
  const mids = [6, 12, 18]
    .map(
      (h) =>
        `<text x="${padL + (h + 0.5) * (plotW / 24)}" y="${H - 4}" class="axis" text-anchor="middle">${h}h</text>`,
    )
    .join("");
  return `<svg viewBox="0 0 ${W} ${H}" class="chart">${yaxis}${bars}
    <text x="${padL + 2}" y="${H - 4}" class="axis">0h</text>
    ${mids}
    <text x="${W - 2}" y="${H - 4}" class="axis" text-anchor="end">23h</text></svg>`;
}

/** The segmented metric control (Tokens|Cost) embedded in each lens header. */
function metricSeg(opts: AnalyticsOpts): string {
  return `<div class="seg" data-seg="metric">
    <button data-metric="tokens" class="${opts.metric === "tokens" ? "on" : ""}">${t("toggle.tokens")}</button>
    <button data-metric="price" class="${opts.metric === "price" ? "on" : ""}">${t("toggle.price")}</button>
  </div>`;
}

/** Split the range-total into a big figure + a unit tail so the hero can render
 *  "5.6" large with "M tokens" small (C1 `.fig` + `.u`). Cost metric shows the
 *  dollar total whole, with no unit tail. */
function heroFig(a: Analytics, price: boolean): { fig: string; unit: string } {
  if (price) return { fig: fmtUsd(a.totalCostUsd), unit: "" };
  const s = fmtTokens(a.totalTokens); // "5.6M" / "608.0K" / "1234"
  const m = /^([\d.]+)([KMB])?$/.exec(s);
  if (!m) return { fig: s, unit: t("analytics.tokens") };
  const suffix = m[2] ? `${m[2]} ` : "";
  return { fig: m[1], unit: `${suffix}${t("analytics.tokens")}` };
}

/** Trends footnote (C1): the leftover records/rate facts, deduped against the
 *  hero (which already carries the streak) — Peak day, Busiest hour, sessions
 *  this week, tok/min. Records with no data drop out rather than showing 0. */
function trendsFootnote(a: Analytics): string {
  const parts: string[] = [];
  if (a.records.maxDay.tokens > 0)
    parts.push(t("analytics.footPeak", { date: `<b>${a.records.maxDay.date.slice(5)}</b>` }));
  if (a.records.maxHour.tokens > 0) {
    const hh = String(a.records.maxHour.hour).padStart(2, "0");
    parts.push(t("analytics.footBusiest", { hour: `<b>${hh}:00</b>` }));
  }
  parts.push(`<b>${a.sessionsThisWeek}</b> ${t("analytics.sessionsThisWeek")}`);
  parts.push(`<b>${fmtTokens(a.tokPerMin)}</b> ${t("analytics.tokPerMin")}`);
  return parts.join(" · ");
}

/** 鏡頭 1 · Trends (何時): hero total, granularity-switched main chart (daily
 *  stacked + month heatmap / 24h hourly), and the records/rate footnote. */
function trendsLens(a: Analytics, opts: AnalyticsOpts): string {
  const price = opts.metric === "price";
  const chart = opts.granularity === "hourly" ? hourly(a, opts) : stackedDaily(a, opts);
  const heat = opts.granularity === "daily" && a.range === "month" ? heatmap(a) : "";
  // "from {date}" when a month's local logs don't reach the nominal window start.
  const start = monthStartNote(a);
  const note = start ? `<div class="chart-note">${t("analytics.since", { date: start.slice(5) })}</div>` : "";
  const { fig, unit } = heroFig(a, price);
  return `<div class="feature">
    <span class="cap">Lens 1 · ${t("subtab.trends")}</span>
    <p class="kick">${t("analytics.trendsKick")}</p>
    <div class="toggles">
      <div class="seg" data-seg="granularity">
        <button data-granularity="daily" class="${opts.granularity === "daily" ? "on" : ""}">${t("toggle.daily")}</button>
        <button data-granularity="hourly" class="${opts.granularity === "hourly" ? "on" : ""}">${t("toggle.hourly")}</button>
      </div>
      ${metricSeg(opts)}
    </div>
    <div class="hero">
      <div class="eyebrow">${t("analytics.trendsEyebrow")}</div>
      <div class="fig">${fig}${unit ? `<span class="u">${unit}</span>` : ""}</div>
      <div class="sub num">${t("analytics.trendsSub", { cost: fmtUsd(a.totalCostUsd), days: a.records.streakDays })}</div>
    </div>
    <div class="support">
      <span class="lbl">${t("analytics.trendsChartLabel")}</span>
      ${chart}
      ${note}
    </div>
    ${heat ? `<div class="support"><span class="lbl">${t("analytics.heatmapTitle")}</span>${heat}</div>` : ""}
    <div class="footnote num">${trendsFootnote(a)}</div>
  </div>`;
}

/** 鏡頭 2 · Breakdown (去哪): leading model/agent hero, group-switched ranking
 *  (#1 = the lens's single magenta), grayscale activity donut, project rows, and
 *  the token-composition bar. Each secondary section drops out when it has no
 *  data, so an empty section never draws a blank card. */
function breakdownLens(a: Analytics, opts: AnalyticsOpts): string {
  const price = opts.metric === "price";
  const rec = price
    ? opts.group === "model" ? a.byModelCost : a.byAgentCost
    : opts.group === "model" ? a.byModel : a.byAgent;
  const entries = Object.entries(rec).sort((x, y) => y[1] - x[1]);
  const total = entries.reduce((s, [, v]) => s + v, 0);
  const leader = entries[0];
  const eyebrow = opts.group === "model" ? t("analytics.leadingModel") : t("analytics.leadingAgent");
  const hero = leader
    ? `<div class="hero">
        <div class="eyebrow">${eyebrow}</div>
        <div class="fig fig-name">${esc(leader[0])}</div>
        <div class="sub num">${t("analytics.breakdownSub", {
          value: price ? fmtUsd(leader[1]) : fmtTokens(leader[1]),
          pct: sharePct(leader[1], total),
        })}</div>
      </div>`
    : "";
  const activity = donutGray(a.byKind);
  const projects = projectRows(a.byProject);
  return `<div class="feature">
    <span class="cap">Lens 2 · ${t("subtab.breakdown")}</span>
    <p class="kick">${t("analytics.breakdownKick")}</p>
    <div class="toggles">
      <div class="seg" data-seg="group">
        <button data-group="model" class="${opts.group === "model" ? "on" : ""}">${t("toggle.model")}</button>
        <button data-group="agent" class="${opts.group === "agent" ? "on" : ""}">${t("toggle.agent")}</button>
      </div>
      ${metricSeg(opts)}
    </div>
    ${hero}
    ${rankRows(rec, price, true)}
    ${activity ? `<div class="support"><span class="lbl">${t("analytics.activityTitle")}</span>${activity}</div>` : ""}
    ${projects ? `<div class="support"><span class="lbl">${t("analytics.projectsTitle")}</span>${projects}</div>` : ""}
    ${compositionBar(a)}
  </div>`;
}

/** Render both lenses into one scrolling column. `.chart-wrap` is the shared
 *  positioning context for the single re-created `.chart-tip`; wireChartTip is
 *  delegated on `container` once and survives this innerHTML swap. */
export function renderAnalytics(container: HTMLElement, a: Analytics, opts: AnalyticsOpts): void {
  container.innerHTML =
    `<div class="chart-wrap">` +
    trendsLens(a, opts) +
    breakdownLens(a, opts) +
    `<div class="chart-tip" hidden></div>` +
    `</div>`;
  wireChartTip(container);
}

/**
 * Custom bar tooltip (native <title> is slow to appear): one absolutely-
 * positioned div per chart-wrap, shown on pointerover of a bar and populated
 * from the rect's `data-tip`. Delegated on `container` and wired only once, so
 * it survives the innerHTML re-render every renderAnalytics does. The <title>
 * stays as a fallback.
 */
function wireChartTip(container: HTMLElement): void {
  if (container.dataset.tipWired) return;
  container.dataset.tipWired = "1";

  const tipFor = (target: EventTarget | null): HTMLElement | null => {
    const rect = (target as Element | null)?.closest?.("rect[data-tip]") ?? null;
    if (!rect) return null;
    const wrap = rect.closest(".chart-wrap");
    return (wrap?.querySelector(".chart-tip") as HTMLElement) ?? null;
  };
  const place = (tip: HTMLElement, e: PointerEvent) => {
    const wrap = tip.parentElement as HTMLElement;
    const r = wrap.getBoundingClientRect();
    tip.style.left = `${e.clientX - r.left}px`;
    tip.style.top = `${e.clientY - r.top}px`;
  };

  container.addEventListener("pointerover", (e) => {
    const rect = (e.target as Element | null)?.closest?.("rect[data-tip]");
    const tip = tipFor(e.target);
    if (!rect || !tip) return;
    tip.textContent = rect.getAttribute("data-tip") ?? "";
    place(tip, e as PointerEvent);
    tip.hidden = false;
  });
  container.addEventListener("pointermove", (e) => {
    const tip = tipFor(e.target);
    if (tip && !tip.hidden) place(tip, e as PointerEvent);
  });
  container.addEventListener("pointerout", (e) => {
    const tip = tipFor(e.target);
    if (tip) tip.hidden = true;
  });
}
