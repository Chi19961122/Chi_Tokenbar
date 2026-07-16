// Layer ③ analytics (UX Spec v3 §11): stat tiles, 2D charts, breakdown.
// Charts are hand-rolled SVG — 2D only (§14 excludes 3D).

import type { Analytics, DayPoint } from "./types";
import { fmtTokens, fmtUsd } from "./format";
import { keyColor, seriesColor } from "./colors";
import { t } from "./i18n";

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
    case "share":
      body = shareBars(opts.group === "model" ? a.byModel : a.byAgent);
      break;
    case "stats":
      body = statsView(a);
      break;
    default: // overview (daily stacked chart)
      body = stackedDaily(a, opts);
  }
  // "from {date}" when a month's local logs don't reach the nominal window start.
  const start = monthStartNote(a);
  const note = start ? `<div class="chart-note">${t("analytics.since", { date: start.slice(5) })}</div>` : "";
  container.innerHTML = tiles(a) + note + `<div class="chart-wrap">${body}</div>`;
}
