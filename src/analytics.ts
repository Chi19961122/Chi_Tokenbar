// Layer ③ analytics (UX Spec v3 §11): stat tiles, 2D charts, breakdown.
// Charts are hand-rolled SVG — 2D only (§14 excludes 3D).

import type { Analytics, DayPoint } from "./types";
import { fmtTokens, fmtUsd } from "./format";
import { seriesColor } from "./colors";

export type SubTab = "overview" | "daily" | "hourly" | "models" | "agents" | "stats";
export type Metric = "tokens" | "price";
export type Group = "model" | "agent";

export interface AnalyticsOpts {
  subtab: SubTab;
  metric: Metric;
  group: Group;
}

function tiles(a: Analytics): string {
  return `
    <div class="tiles">
      <div class="tile"><b>${fmtTokens(a.totalTokens)}</b><span>tokens</span></div>
      <div class="tile"><b>${fmtUsd(a.totalCostUsd)}</b><span>估算 · 訂閱已含</span></div>
      <div class="tile"><b>${fmtUsd(a.bestDay.costUsd)}</b><span>best · ${a.bestDay.date.slice(5)}</span></div>
      <div class="tile"><b>${a.activeDays}</b><span>active days</span></div>
    </div>`;
}

function seriesKeys(daily: DayPoint[], group: Group): string[] {
  const set = new Set<string>();
  for (const d of daily) {
    for (const k of Object.keys(group === "model" ? d.byModel : d.byAgent)) set.add(k);
  }
  return [...set];
}

function stackedDaily(a: Analytics, opts: AnalyticsOpts): string {
  const W = 320, H = 150, padB = 18, padT = 6;
  const daily = a.daily;
  const n = daily.length;
  const bw = (W / n) * 0.62;
  const keys = seriesKeys(daily, opts.group);

  const totals = daily.map((d) =>
    opts.metric === "price"
      ? d.costUsd
      : Object.values(opts.group === "model" ? d.byModel : d.byAgent).reduce((s, v) => s + v, 0),
  );
  const max = Math.max(1, ...totals);
  const scale = (H - padB - padT) / max;

  const bars = daily
    .map((d, i) => {
      const cx = (i + 0.5) * (W / n);
      if (opts.metric === "price") {
        const h = d.costUsd * scale;
        return `<rect x="${cx - bw / 2}" y="${H - padB - h}" width="${bw}" height="${h}" rx="1.5" fill="${seriesColor(1)}"/>`;
      }
      const rec = opts.group === "model" ? d.byModel : d.byAgent;
      let y = H - padB;
      let segs = "";
      keys.forEach((k, ki) => {
        const v = rec[k] || 0;
        const h = v * scale;
        y -= h;
        segs += `<rect x="${cx - bw / 2}" y="${y}" width="${bw}" height="${h}" fill="${seriesColor(ki)}"/>`;
      });
      return segs;
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
          .map((k, ki) => `<span><i style="background:${seriesColor(ki)}"></i>${k}</span>`)
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
  return `<div class="bars">${entries
    .map(
      ([k, v], i) => `
      <div class="bar-row">
        <span class="bar-label">${k}</span>
        <div class="bar-track"><div class="bar-fill" style="width:${(v / max) * 100}%;background:${seriesColor(i)}"></div></div>
        <span class="bar-val">${fmtTokens(v)}</span>
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
      ${seg("input", b.input, 0)}
      ${seg("cached", b.cached, 1)}
      ${seg("output", b.output, 2)}
      ${seg("reasoning", b.reasoning, 3)}
    </div>
    <div class="kv">
      <div><b>${a.sessionsThisWeek}</b><span>sessions this week</span></div>
      <div><b>${fmtTokens(a.tokPerMin)}</b><span>tok/min</span></div>
    </div>
    <div class="accounts">${accounts}</div>`;
}

export function renderAnalytics(container: HTMLElement, a: Analytics, opts: AnalyticsOpts): void {
  let body = "";
  switch (opts.subtab) {
    case "hourly":
      body = hourly(a);
      break;
    case "models":
      body = shareBars(a.byModel);
      break;
    case "agents":
      body = shareBars(a.byAgent);
      break;
    case "stats":
      body = statsView(a);
      break;
    default: // overview / daily
      body = stackedDaily(a, opts);
  }
  container.innerHTML = tiles(a) + `<div class="chart-wrap">${body}</div>`;
}
