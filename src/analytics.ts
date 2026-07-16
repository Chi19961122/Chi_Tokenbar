// Layer ③ analytics (UX Spec v3 §11): stat tiles, 2D charts, breakdown.
// Charts are hand-rolled SVG — 2D only (§14 excludes 3D).

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
export type SubTab = "overview" | "hourly" | "share" | "stats";
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
  const totalByDate = new Map(
    a.daily.map((d) => [d.date, Object.values(d.byAgent).reduce((s, v) => s + v, 0)]),
  );

  const cellDivs = cells
    .map((c) => {
      const level = c.intensity === 0 ? 0 : Math.min(4, Math.ceil(c.intensity * 4));
      const tot = totalByDate.get(c.date) ?? 0;
      return `<div class="hm-cell hm-l${level}" style="grid-row:${c.weekdayRow + 1};grid-column:${
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

/** Fixed color per activity kind (mirrors the gem series family). */
function kindColor(kind: string): string {
  switch (kind) {
    case "edit":
      return "#2b6fb8";
    case "read":
      return "#2fa87e";
    case "run":
      return "#c2497a";
    default:
      return "#6f7883";
  }
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
  let acc = 0;
  const stops: string[] = [];
  const legend = byKind
    .map((k) => {
      const start = (acc / total) * 100;
      acc += k.tokens;
      const end = (acc / total) * 100;
      const col = kindColor(k.kind);
      stops.push(`${col} ${start.toFixed(2)}% ${end.toFixed(2)}%`);
      return `<span><i style="background:${col}"></i>${kindLabel(k.kind)} <b>${sharePct(
        k.tokens,
        total,
      )}%</b></span>`;
    })
    .join("");
  return `<div class="donut-sec">
    <div class="donut" style="background:conic-gradient(${stops.join(",")})">
      <div class="donut-hole"><b>${fmtTokens(total)}</b><span>${t("analytics.tokens")}</span></div>
    </div>
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
        <div class="bar-track"><div class="bar-fill" style="width:${
          (p.tokens / max) * 100
        }%;background:${seriesColor(i)}"></div></div>
        <span class="bar-val">${shareLabel(p.tokens, total)}</span>
      </div>`;
    })
    .join("");
  return `<div class="bars">${rows}</div>`;
}

/** A titled sub-section wrapper for the extra Breakdown / overview dimensions. */
function section(title: string, inner: string): string {
  return `<div class="sub-sec"><div class="sub-sec-h">${title}</div>${inner}</div>`;
}

function tiles(a: Analytics): string {
  return `
    <div class="tiles">
      <div class="tile"><b>${fmtTokens(a.totalTokens)}</b><span>${t("analytics.tokens")}</span></div>
      <div class="tile"><b>${fmtUsd(a.totalCostUsd)}</b><span>${t("analytics.estCost")}</span></div>
      <div class="tile"><b>${fmtUsd(a.bestDay.costUsd)}</b><span>${t("analytics.peak")} · ${a.bestDay.date.slice(5)}</span></div>
      <div class="tile"><b>${a.activeDays}</b><span>${t("analytics.activeDays")}</span></div>
    </div>`;
}

function seriesKeys(daily: DayPoint[], group: Group): string[] {
  const set = new Set<string>();
  for (const d of daily) {
    for (const k of Object.keys(group === "model" ? d.byModel : d.byAgent)) set.add(k);
  }
  return [...set];
}

/** Value shown on the y-axis for one day, honouring the metric. */
function dayTotal(d: DayPoint, opts: AnalyticsOpts): number {
  return opts.metric === "price"
    ? d.costUsd
    : Object.values(opts.group === "model" ? d.byModel : d.byAgent).reduce((s, v) => s + v, 0);
}

function stackedDaily(a: Analytics, opts: AnalyticsOpts): string {
  const W = 320, H = 150, padB = 18, padT = 6;
  // Drop leading empty days so a month backed by a few days of logs doesn't
  // render a wall of blank bars; the x-axis then starts at the first active day
  // (which matches the backend's range_start_day annotation).
  const allTotals = a.daily.map((d) => dayTotal(d, opts));
  let fi = allTotals.findIndex((v) => v > 0);
  if (fi < 0) fi = 0;
  const daily = a.daily.slice(fi);
  const totals = allTotals.slice(fi);
  const n = daily.length;
  const bw = (W / n) * 0.62;
  const keys = seriesKeys(daily, opts.group);

  const max = Math.max(1, ...totals);
  const scale = (H - padB - padT) / max;
  // Denominator for the "share of range total" hover labels (§ readability).
  const rangeTotal = totals.reduce((s, v) => s + v, 0);
  const fmtDayVal = (v: number) => (opts.metric === "price" ? fmtUsd(v) : shareLabel(v, rangeTotal));

  const bars = daily
    .map((d, i) => {
      const cx = (i + 0.5) * (W / n);
      const title = `<title>${d.date.slice(5)} · ${fmtDayVal(totals[i])}</title>`;
      if (opts.metric === "price") {
        const h = d.costUsd * scale;
        return `<rect x="${cx - bw / 2}" y="${H - padB - h}" width="${bw}" height="${h}" rx="1.5" fill="${seriesColor(1)}">${title}</rect>`;
      }
      const rec = opts.group === "model" ? d.byModel : d.byAgent;
      // Topmost non-empty segment gets a rounded top (C1 dome). rx on a <rect>
      // rounds all four corners; since this is the last segment drawn and the
      // ones below it are square-topped, the visible result reads as a dome.
      let topKi = -1;
      keys.forEach((k, ki) => {
        if ((rec[k] || 0) > 0) topKi = ki;
      });
      let y = H - padB;
      let segs = "";
      keys.forEach((k, ki) => {
        const v = rec[k] || 0;
        const h = v * scale;
        y -= h;
        const dome = ki === topKi ? ` rx="2"` : "";
        segs += `<rect x="${cx - bw / 2}" y="${y}" width="${bw}" height="${h}"${dome} fill="${keyColor(k, ki)}"/>`;
      });
      // A transparent full-height hitbox per day carries the total/% hover.
      const hit = `<rect x="${cx - bw / 2}" y="${padT}" width="${bw}" height="${H - padB - padT}" fill="transparent">${title}</rect>`;
      return segs + hit;
    })
    .join("");

  const xlabels =
    n > 1
      ? `<text x="2" y="${H - 5}" class="axis">${daily[0].date.slice(5)}</text>
         <text x="${W - 2}" y="${H - 5}" class="axis" text-anchor="end">${daily[n - 1].date.slice(5)}</text>`
      : `<text x="${W / 2}" y="${H - 5}" class="axis" text-anchor="middle">${daily[0].date.slice(5)}</text>`;

  const legend =
    opts.metric === "price"
      ? ""
      : `<div class="legend">${keys
          .map((k, ki) => `<span><i style="background:${keyColor(k, ki)}"></i>${k}</span>`)
          .join("")}</div>`;

  return `<svg viewBox="0 0 ${W} ${H}" class="chart">${bars}${xlabels}</svg>${legend}`;
}

function hourly(a: Analytics): string {
  const W = 320, H = 130, padB = 16, padT = 6;
  const max = Math.max(1, ...a.hourly);
  const bw = (W / 24) * 0.6;
  const scale = (H - padB - padT) / max;
  const bars = a.hourly
    .map((v, i) => {
      const cx = (i + 0.5) * (W / 24);
      const h = v * scale;
      return `<rect x="${cx - bw / 2}" y="${H - padB - h}" width="${bw}" height="${h}" rx="1" fill="${seriesColor(3)}"/>`;
    })
    .join("");
  return `<svg viewBox="0 0 ${W} ${H}" class="chart">${bars}
    <text x="2" y="${H - 4}" class="axis">0h</text>
    <text x="${W - 2}" y="${H - 4}" class="axis" text-anchor="end">23h</text></svg>`;
}

function shareBars(rec: Record<string, number>): string {
  const entries = Object.entries(rec).sort((a, b) => b[1] - a[1]);
  const max = Math.max(1, ...entries.map((e) => e[1]));
  // Share-of-total denominator = the sum of every bar shown = this grouping's
  // range total, so each label reads "tokens · % of range" (§ readability).
  const total = entries.reduce((s, [, v]) => s + v, 0);
  return `<div class="bars">${entries
    .map(
      ([k, v], i) => `
      <div class="bar-row">
        <span class="bar-label">${k}</span>
        <div class="bar-track"><div class="bar-fill" style="width:${(v / max) * 100}%;background:${keyColor(k, i)}"></div></div>
        <span class="bar-val">${shareLabel(v, total)}</span>
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
  return `
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
      body = hourly(a);
      break;
    case "share": {
      // The model/agent grouping, then two independent dimensions below it
      // (階段 C+): activity type (donut) and per-project totals (bars). Each is
      // omitted when it has no data, so an empty section never shows.
      const activity = donut(a.byKind);
      const projects = projectBars(a.byProject);
      body =
        shareBars(opts.group === "model" ? a.byModel : a.byAgent) +
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
