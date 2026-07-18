// Layer ③ analytics (UX Spec v3 §11): stat tiles, charts, breakdown.

import type { Analytics, DayPoint, KindCount, ProjectCount } from "./types";
import { fmtTokens, fmtUsd } from "./format";
import { keyColor, seriesColor } from "./colors";
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

// 階段 C subtab convergence: "daily" folded into overview (the stacked daily
// chart *is* the overview's main chart), and "models"/"agents" collapsed into a
// single "share" breakdown switched by the model/agent group toggle — one less
// navigation layer either way.
export type SubTab = "overview" | "hourly" | "share" | "stats" | "report";
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
  subtab: SubTab;
  metric: Metric;
  group: Group;
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

/** Weekday of a YYYY-MM-DD bucket, Mon=0 … Sun=6. Parsed as UTC so it matches
 *  the backend's UTC day bucketing and never drifts with the local timezone. */
function weekdayMon(date: string): number {
  const d = new Date(date + "T00:00:00Z");
  return (d.getUTCDay() + 6) % 7; // JS Sun=0..Sat=6 → Mon=0..Sun=6
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

/** Fixed editorial sequence; activity kinds no longer carry semantic colors.
 *  CSS-variable values (theme-following) — see colors.ts SERIES header. */
function kindColor(index: number): string {
  const colors = ["var(--ink-900)", "var(--ink-500)", "var(--ink-300)", "var(--accent)"];
  return colors[index % colors.length];
}
function kindLabel(kind: string): string {
  switch (kind) {
    case "edit":
      return t("analytics.kindEdit");
    case "read":
      return t("analytics.kindRead");
    case "run":
      return t("analytics.kindRun");
    case "other":
      return t("analytics.kindOther");
    default:
      return kind;
  }
}

/** Activity-type donut (conic-gradient ring + center total + legend with %).
 *  Empty when nothing is classifiable — the caller then omits the section. */
function donut(byKind: KindCount[]): string {
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
      const col = kindColor(index);
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
  return `<div class="donut-sec">
    <svg class="donut" viewBox="0 0 56 56" role="img" aria-label="${fmtTokens(total)} ${t("analytics.tokens")}">
      <circle cx="28" cy="28" r="${radius}" fill="none" style="stroke:var(--donut-ring)" stroke-width="7"/>
      ${arcs.join("")}
    </svg>
    <div class="donut-legend">${legend}</div>
  </div>`;
}

/** Per-project horizontal bars, "{tokens} · {pct}%" labels (reuses shareLabel).
 *  Empty when there is no project data. */
function projectBars(byProject: ProjectCount[]): string {
  if (byProject.length === 0) return "";
  const total = byProject.reduce((s, p) => s + p.tokens, 0);
  const max = Math.max(1, ...byProject.map((p) => p.tokens));
  const rows = byProject
    .map((p, i) => {
      const name = p.name === "__other__" ? t("analytics.projectsOther") : p.name;
      return `<div class="bar-row">
        <span class="bar-label" title="${esc(name)}">${esc(name)}</span>
        <div class="bar-track"><div class="bar-fill${i === 0 ? " is-top" : ""}" style="width:${
          (p.tokens / max) * 100
        }%"></div></div>
        <span class="bar-val">${shareLabel(p.tokens, total)}</span>
      </div>`;
    })
    .join("");
  return `<div class="bars bars-stack">${rows}</div>`;
}

/** A titled sub-section wrapper for the extra Breakdown / overview dimensions. */
function section(title: string, inner: string): string {
  return `<div class="sub-sec"><div class="sub-sec-h">${title}</div>${inner}</div>`;
}

function tiles(a: Analytics): string {
  return `
    <div class="tiles">
      <div class="tile tile-accent"><span>${t("analytics.estCost")}</span><b>${fmtUsd(a.totalCostUsd)}</b></div>
      <div class="tile"><span>${t("analytics.peak")}</span><b>${a.records.maxDay.date.slice(5)}</b></div>
      <div class="tile"><span>${t("analytics.streak")}</span><b>${a.records.streakDays}d</b></div>
    </div>`;
}

/** Value shown on the y-axis for one day, honouring the metric. */
function dayTotal(d: DayPoint, opts: AnalyticsOpts): number {
  return opts.metric === "price"
    ? d.costUsd
    : Object.values(opts.group === "model" ? d.byModel : d.byAgent).reduce((s, v) => s + v, 0);
}

function stackedDaily(a: Analytics, opts: AnalyticsOpts): string {
  const W = 320, plotH = 56, H = 74, gap = 2;
  // Drop leading empty days so a month backed by a few days of logs doesn't
  // render a wall of blank bars; the x-axis then starts at the first active day
  // (which matches the backend's range_start_day annotation).
  const allTotals = a.daily.map((d) => dayTotal(d, opts));
  let fi = allTotals.findIndex((v) => v > 0);
  if (fi < 0) fi = 0;
  const daily = a.daily.slice(fi);
  const totals = allTotals.slice(fi);
  const n = daily.length;
  const bw = (W - gap * Math.max(0, n - 1)) / Math.max(1, n);

  const max = Math.max(1, ...totals);
  const scale = plotH / max;
  // Denominator for the "share of range total" hover labels (§ readability).
  const rangeTotal = totals.reduce((s, v) => s + v, 0);
  const fmtDayVal = (v: number) => (opts.metric === "price" ? fmtUsd(v) : shareLabel(v, rangeTotal));

  const bars = daily
    .map((d, i) => {
      const x = i * (bw + gap);
      const h = totals[i] * scale;
      // Fill is set by class in styles.css (theme-following): today = accent,
      // a "strong" day = heavy ink, else a dim/weak ink.
      const isToday = i === n - 1;
      const cls = isToday ? " is-today" : totals[i] / max > 0.6 ? " is-strong" : "";
      const title = `<title>${d.date.slice(5)} · ${fmtDayVal(totals[i])}</title>`;
      return `<rect class="daily-bar${cls}" x="${x}" y="${plotH - h}" width="${bw}" height="${Math.max(0, h)}" rx="1">${title}</rect>`;
    })
    .join("");

  const xlabels = `<text x="0" y="${H - 1}" class="axis">30d ago</text>
    <text x="${W}" y="${H - 1}" class="axis axis-today" text-anchor="end">today</text>`;

  return `<svg viewBox="0 0 ${W} ${H}" class="chart daily-chart">${bars}${xlabels}</svg>`;
}

function hourly(a: Analytics, opts: AnalyticsOpts): string {
  const W = 320, H = 130, padB = 16, padT = 6;
  // Price mode reads the per-hour cost series and normalizes on its own max, so
  // the shape reflects spend rather than raw tokens.
  const price = opts.metric === "price";
  const data = price ? a.hourlyCost : a.hourly;
  const fmtVal = (v: number) => (price ? fmtUsd(v) : fmtTokens(v));
  const max = Math.max(price ? 1e-9 : 1, ...data);
  const bw = (W / 24) * 0.6;
  const scale = (H - padB - padT) / max;
  const bars = data
    .map((v, i) => {
      const cx = (i + 0.5) * (W / 24);
      const h = v * scale;
      const title = `<title>${i}:00 · ${fmtVal(v)}</title>`;
      return `<rect x="${cx - bw / 2}" y="${H - padB - h}" width="${bw}" height="${h}" rx="1" style="fill:${seriesColor(3)}">${title}</rect>`;
    })
    .join("");
  // Mid-axis labels every 6h, centered under their bar — the two endpoints
  // alone left 22 unlabeled slots to count by hand.
  const mids = [6, 12, 18]
    .map((h) => `<text x="${(h + 0.5) * (W / 24)}" y="${H - 4}" class="axis" text-anchor="middle">${h}h</text>`)
    .join("");
  return `<svg viewBox="0 0 ${W} ${H}" class="chart">${bars}
    <text x="2" y="${H - 4}" class="axis">0h</text>
    ${mids}
    <text x="${W - 2}" y="${H - 4}" class="axis" text-anchor="end">23h</text></svg>`;
}

function shareBars(rec: Record<string, number>, price = false): string {
  const entries = Object.entries(rec).sort((a, b) => b[1] - a[1]);
  const max = Math.max(price ? 1e-9 : 1, ...entries.map((e) => e[1]));
  // Share-of-total denominator = the sum of every bar shown = this grouping's
  // range total, so each label reads "value · % of range" (§ readability). In
  // price mode the value is cost and the % is share of the cost total.
  const total = entries.reduce((s, [, v]) => s + v, 0);
  const label = (v: number) =>
    price ? `${fmtUsd(v)} · ${sharePct(v, total)}%` : shareLabel(v, total);
  return `<div class="bars bars-stack">${entries
    .map(
      ([k, v], i) => `
      <div class="bar-row">
        <span class="bar-label" title="${esc(k)}">${esc(k)}</span>
        <div class="bar-track"><div class="bar-fill" style="width:${(v / max) * 100}%;background:${keyColor(k, i)}"></div></div>
        <span class="bar-val">${label(v)}</span>
      </div>`,
    )
    .join("")}</div>`;
}

function statsView(a: Analytics): string {
  const b = a.breakdown;
  const total = Math.max(1, b.input + b.cached + b.output + b.reasoning);
  const seg = (label: string, v: number, i: number) =>
    `<div class="bar-row"><span class="bar-label">${label}</span>
       <div class="bar-track"><div class="bar-fill" style="width:${(v / total) * 100}%;background:${seriesColor(i)}"></div></div>
       <span class="bar-val">${fmtTokens(v)}</span></div>`;
  const accounts = a.accounts
    .map((ac) => `<div class="acct"><b>${ac.client}</b> · ${ac.account} · ${ac.plan}</div>`)
    .join("");
  const records = a.records.maxDay.tokens > 0
    ? `<div class="records">${a.records.prNow ? `<span class="pr-now">${t("analytics.prNow")}</span>` : ""}
       <div class="tiles">
         <div class="tile"><b>${fmtTokens(a.records.maxDay.tokens)}</b><span>${t("analytics.maxDay")} · ${a.records.maxDay.date.slice(5)}</span></div>
         <div class="tile"><b>${fmtTokens(a.records.maxHour.tokens)}</b><span>${t("analytics.maxHour")} · ${a.records.maxHour.date.slice(5)} ${String(a.records.maxHour.hour).padStart(2, "0")}:00</span></div>
         <div class="tile"><b>${a.records.streakDays}</b><span>${t("analytics.streak")}</span></div>
       </div></div>`
    : "";
  return `
    ${records}
    <div class="bars">
      ${seg(t("analytics.input"), b.input, 0)}
      ${seg(t("analytics.cached"), b.cached, 1)}
      ${seg(t("analytics.output"), b.output, 2)}
      ${seg(t("analytics.reasoning"), b.reasoning, 3)}
    </div>
    <div class="kv">
      <div><b>${a.sessionsThisWeek}</b><span>${t("analytics.sessionsThisWeek")}</span></div>
      <div><b>${fmtTokens(a.tokPerMin)}</b><span>${t("analytics.tokPerMin")}</span></div>
    </div>
    <div class="accounts">${accounts}</div>`;
}

export function renderAnalytics(container: HTMLElement, a: Analytics, opts: AnalyticsOpts): void {
  let body = "";
  switch (opts.subtab) {
    case "hourly":
      body = hourly(a, opts);
      break;
    case "share": {
      // The model/agent grouping, then two independent dimensions below it
      // (階段 C+): activity type (donut) and per-project totals (bars). Each is
      // omitted when it has no data, so an empty section never shows. The metric
      // toggle switches the primary grouping between token and cost totals.
      const price = opts.metric === "price";
      const rec = price
        ? opts.group === "model" ? a.byModelCost : a.byAgentCost
        : opts.group === "model" ? a.byModel : a.byAgent;
      const activity = donut(a.byKind);
      const projects = projectBars(a.byProject);
      body =
        shareBars(rec, price) +
        (activity ? section(t("analytics.activityTitle"), activity) : "") +
        (projects ? section(t("analytics.projectsTitle"), projects) : "");
      break;
    }
    case "stats":
      body = statsView(a);
      break;
    default: {
      // overview: daily stacked chart, plus the GitHub-style heatmap for the
      // month range only (§ layout decision).
      body = stackedDaily(a, opts);
      if (a.range === "month") {
        const hm = heatmap(a);
        if (hm) body += section(t("analytics.heatmapTitle"), hm);
      }
    }
  }
  // "from {date}" when a month's local logs don't reach the nominal window start.
  const start = monthStartNote(a);
  const note = start ? `<div class="chart-note">${t("analytics.since", { date: start.slice(5) })}</div>` : "";
  container.innerHTML = tiles(a) + note + `<div class="chart-wrap">${body}</div>`;
}
