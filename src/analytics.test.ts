// 階段 C analytics decision-logic tests: the share-of-range label and the
// month start-date annotation condition. These pin behaviour (exact strings /
// when the note appears), not the shape of the implementation — flipping the
// condition or the denominator must turn one of these red.

import { describe, expect, it } from "vitest";
import {
  axisTicks,
  dailyXTicks,
  heatCells,
  monthStartNote,
  renderAnalytics,
  sharePct,
  shareLabel,
} from "./analytics";
import type { Analytics, DayPoint } from "./types";
import { mockAnalytics } from "./mock";
import { setLocale } from "./i18n";

/** N consecutive daily buckets from `start`, each with `tokens` under one agent
 *  (0 → an empty day). Dates advance in UTC to match the backend buckets. */
function mkDaily(start: string, n: number, tokens = 1_000_000): DayPoint[] {
  const base = new Date(start + "T00:00:00Z").getTime();
  return Array.from({ length: n }, (_, i) => {
    const byAgent: Record<string, number> = {};
    if (tokens > 0) byAgent["Claude Code"] = tokens;
    return {
      date: new Date(base + i * 86_400_000).toISOString().slice(0, 10),
      byModel: {},
      byAgent,
      costUsd: 0,
    };
  });
}

describe("share-of-range labels", () => {
  it("computes a whole-number percent of the range total", () => {
    expect(sharePct(1_400_000, 8_000_000)).toBe(18); // 17.5 → 18
    expect(sharePct(2_000_000, 8_000_000)).toBe(25);
  });

  it("guards a zero denominator instead of producing NaN", () => {
    expect(sharePct(0, 0)).toBe(0);
    expect(shareLabel(0, 0)).toBe("0 · 0%");
  });

  it("formats tokens and percent together as '1.4M · 17%'", () => {
    expect(shareLabel(1_400_000, 8_000_000)).toBe("1.4M · 18%");
    expect(shareLabel(500_000, 1_000_000)).toBe("500.0K · 50%");
  });
});

/** A minimal month Analytics whose daily window starts at `windowStart` and
 *  reports `rangeStartDay` as its actual reach. */
function monthly(windowStart: string, rangeStartDay: string): Analytics {
  return {
    ...mockAnalytics("month"),
    range: "month",
    rangeStartDay,
    daily: [
      { date: windowStart, byModel: {}, byAgent: {}, costUsd: 0 },
      { date: rangeStartDay, byModel: { x: 1 }, byAgent: { A: 1 }, costUsd: 1 },
    ],
  };
}

describe("month start-date annotation", () => {
  it("returns the actual start when local logs don't reach the window start", () => {
    expect(monthStartNote(monthly("2026-06-17", "2026-07-10"))).toBe("2026-07-10");
  });

  it("stays silent when the history covers the whole window", () => {
    // rangeStartDay === daily[0].date → nothing to annotate.
    const a = monthly("2026-06-17", "2026-06-17");
    a.rangeStartDay = a.daily[0].date;
    expect(monthStartNote(a)).toBeNull();
  });

  it("never annotates today or week ranges", () => {
    const wk = { ...mockAnalytics("week"), range: "week" as const };
    expect(monthStartNote(wk)).toBeNull();
  });
});

describe("month chart with a short history", () => {
  it("renders the 'from {date}' note and drops leading empty days", () => {
    const a = mockAnalytics("month");
    a.range = "month";
    // Blank out all but the last two days, and report the true reach.
    for (let i = 0; i < a.daily.length - 2; i++) {
      a.daily[i] = { date: a.daily[i].date, byModel: {}, byAgent: {}, costUsd: 0 };
    }
    a.rangeStartDay = a.daily[a.daily.length - 2].date;

    const root = document.createElement("div");
    renderAnalytics(root, a, { metric: "tokens", group: "agent", granularity: "daily" });

    expect(root.querySelector(".chart-note")?.textContent).toContain(a.rangeStartDay.slice(5));
    // Leading empty days are still dropped, while the x-axis keeps its endpoints.
    expect(root.querySelectorAll(".daily-bar")).toHaveLength(2);
    const axisText = [...root.querySelectorAll(".chart .axis")].map((n) => n.textContent);
    expect(axisText).toContain("30d ago");
    expect(axisText).toContain("today");
    // n = 2 (< 4) → no interior date ticks crowding the two endpoints.
    expect(axisText.some((s) => s?.includes("/"))).toBe(false);
  });
});

describe("chart axes (T-911)", () => {
  it("axisTicks returns 0/half/max, and just [0] for an empty chart", () => {
    expect(axisTicks(8_000_000)).toEqual([0, 4_000_000, 8_000_000]);
    expect(axisTicks(0)).toEqual([0]);
    expect(axisTicks(-5)).toEqual([0]);
  });

  it("dailyXTicks spaces a month ~evenly and stays clear of the endpoints", () => {
    const dates = Array.from({ length: 30 }, (_, i) =>
      new Date(Date.UTC(2026, 5, 1) + i * 86_400_000).toISOString().slice(0, 10),
    );
    const ticks = dailyXTicks(dates, "month");
    // step = round(30/5) = 6 → interior ticks at 6/12/18/24 (never 0 or 29).
    expect(ticks.map((t) => t.i)).toEqual([6, 12, 18, 24]);
    expect(ticks.every((t) => t.i > 0 && t.i < dates.length - 1)).toBe(true);
    expect(ticks[0].label).toBe("06/07"); // 2026-06-07 → M/D, slashed, locale-free
  });

  it("dailyXTicks labels alternate days for a week", () => {
    const dates = Array.from({ length: 7 }, (_, i) =>
      new Date(Date.UTC(2026, 6, 13) + i * 86_400_000).toISOString().slice(0, 10),
    );
    expect(dailyXTicks(dates, "week").map((t) => t.i)).toEqual([2, 4]);
  });

  it("dailyXTicks emits nothing when n < 4 (endpoints would collide)", () => {
    const dates = ["2026-07-13", "2026-07-14", "2026-07-15"];
    expect(dailyXTicks(dates, "month")).toEqual([]);
    expect(dailyXTicks(dates, "week")).toEqual([]);
  });

  it("renders a y-axis gutter (gridlines + tokens labels) on the daily chart", () => {
    const a = mockAnalytics("month");
    const root = document.createElement("div");
    renderAnalytics(root, a, { metric: "tokens", group: "agent", granularity: "daily" });
    // 3 gridlines (0/half/max) and 3 matching gutter labels.
    expect(root.querySelectorAll(".daily-chart .grid")).toHaveLength(3);
    expect(root.querySelectorAll(".daily-chart .axis-y")).toHaveLength(3);
    // The gutter's bottom tick is "0"; nothing dollar-formatted in tokens mode.
    const yText = [...root.querySelectorAll(".daily-chart .axis-y")].map((n) => n.textContent);
    expect(yText).toContain("0");
    expect(yText.some((s) => s?.includes("$"))).toBe(false);
  });

  it("renders a y-axis on the hourly chart, in fmtUsd under price mode", () => {
    const a = { ...mockAnalytics("week"), hourlyCost: Array(24).fill(0) };
    a.hourlyCost[3] = 20;
    const root = document.createElement("div");
    renderAnalytics(root, a, { metric: "price", group: "agent", granularity: "hourly" });
    const yText = [...root.querySelectorAll(".chart .axis-y")].map((n) => n.textContent);
    expect(root.querySelectorAll(".chart .grid").length).toBeGreaterThanOrEqual(2);
    // Max tick reflects the $20 peak, formatted as USD.
    expect(yText).toContain("$20.00");
  });
});

describe("custom bar tooltip (T-911)", () => {
  it("stamps data-tip on daily bars with the date · value · share label", () => {
    const a = mockAnalytics("month");
    const root = document.createElement("div");
    renderAnalytics(root, a, { metric: "tokens", group: "agent", granularity: "daily" });
    const bars = [...root.querySelectorAll<SVGRectElement>(".daily-bar")];
    expect(bars.every((b) => b.hasAttribute("data-tip"))).toBe(true);
    // Shape: "MM/DD · <tokens> · <pct>%" and it matches the <title> fallback.
    const last = bars[bars.length - 1];
    expect(last.getAttribute("data-tip")).toMatch(/^\d{2}\/\d{2} · .+ · \d+%$/);
    expect(last.getAttribute("data-tip")).toBe(last.querySelector("title")?.textContent);
  });

  it("stamps data-tip on hourly bars ('H:00 · value') and mounts one tip div", () => {
    const a = { ...mockAnalytics("week"), hourly: Array(24).fill(0) };
    a.hourly[5] = 2_000_000;
    const root = document.createElement("div");
    renderAnalytics(root, a, { metric: "tokens", group: "agent", granularity: "hourly" });
    const bars = [...root.querySelectorAll<SVGRectElement>(".chart rect[data-tip]")];
    expect(bars).toHaveLength(24);
    expect(bars[5].getAttribute("data-tip")).toBe("5:00 · 2.0M");
    // Exactly one custom tooltip element sits inside the chart-wrap.
    expect(root.querySelectorAll(".chart-wrap .chart-tip")).toHaveLength(1);
  });
});

describe("heatCells (activity heatmap)", () => {
  it("aligns weeks and leaves leading blanks when the first day isn't Monday", () => {
    // 2026-07-15 is a Wednesday → Mon=0 row 2.
    const { cells, weeks } = heatCells(mkDaily("2026-07-15", 10));
    expect(cells).toHaveLength(10);
    expect(cells[0].weekdayRow).toBe(2); // Wednesday
    expect(cells[0].weekCol).toBe(0);
    // The first row-0/col-0 slots (Mon, Tue) are never emitted → leading blanks.
    expect(cells.some((c) => c.weekCol === 0 && c.weekdayRow < 2)).toBe(false);
    // 5 days after a Wednesday is a Monday, wrapping into the next column.
    expect(cells[5].weekdayRow).toBe(0);
    expect(cells[5].weekCol).toBe(1);
    expect(weeks).toBe(2);
  });

  it("normalizes intensity to the busiest day", () => {
    const daily = mkDaily("2026-07-13", 3, 0); // Monday start, all empty
    daily[0].byAgent = { "Claude Code": 500_000 };
    daily[1].byAgent = { "Claude Code": 1_000_000 };
    // daily[2] stays empty
    const { cells } = heatCells(daily);
    expect(cells[0].intensity).toBeCloseTo(0.5);
    expect(cells[1].intensity).toBe(1);
    expect(cells[2].intensity).toBe(0);
  });

  it("keeps every cell faint (0) when the whole range is empty", () => {
    const { cells } = heatCells(mkDaily("2026-07-13", 7, 0));
    expect(cells).toHaveLength(7);
    expect(cells.every((c) => c.intensity === 0)).toBe(true);
  });

  it("handles a single day", () => {
    const { cells, weeks } = heatCells(mkDaily("2026-07-15", 1));
    expect(cells).toHaveLength(1);
    expect(cells[0].weekCol).toBe(0);
    expect(cells[0].weekdayRow).toBe(2);
    expect(weeks).toBe(1);
    expect(cells[0].intensity).toBe(1);
  });

  it("returns an empty grid for no data", () => {
    expect(heatCells([])).toEqual({ cells: [], weeks: 0 });
  });
});

describe("T-ui-301 two-lens render wiring", () => {
  it("renders both lenses in one scrolling pane, no sub-tab switcher", () => {
    const a = mockAnalytics("week");
    const root = document.createElement("div");
    renderAnalytics(root, a, { metric: "tokens", group: "agent", granularity: "daily" });

    const lenses = [...root.querySelectorAll(".feature")];
    expect(lenses).toHaveLength(2);
    expect(lenses[0].querySelector(".cap")?.textContent).toContain("Trends");
    expect(lenses[1].querySelector(".cap")?.textContent).toContain("Breakdown");
    // No leftover sub-tab buttons anywhere in the analytics output.
    expect(root.querySelector("[data-sub]")).toBeNull();
  });

  it("renders daily totals as neutral single bars with a pink today bar", () => {
    const a = mockAnalytics("month");
    const root = document.createElement("div");
    renderAnalytics(root, a, { metric: "tokens", group: "agent", granularity: "daily" });

    const bars = [...root.querySelectorAll<SVGRectElement>(".daily-bar")];
    expect(bars).toHaveLength(a.daily.length);
    // Fill is class-driven (theme-following), not an inline hex: the last bar is
    // the pink "today" bar; the rest are heavy ("is-strong") or dim (plain).
    expect(bars[bars.length - 1]?.classList.contains("is-today")).toBe(true);
    expect(bars.slice(0, -1).some((bar) => bar.classList.contains("is-today"))).toBe(false);
    expect(bars.slice(0, -1).every((bar) => !bar.hasAttribute("fill"))).toBe(true);
  });

  it("shows the month heatmap only in the daily granularity for a month range", () => {
    const month = { ...mockAnalytics("month"), range: "month" as const };
    const week = { ...mockAnalytics("week"), range: "week" as const };
    const root = document.createElement("div");

    renderAnalytics(root, month, { metric: "tokens", group: "agent", granularity: "daily" });
    expect(root.querySelector(".hm")).not.toBeNull();
    expect(root.querySelectorAll(".hm-today")).toHaveLength(1);

    // A week range never draws the heatmap…
    renderAnalytics(root, week, { metric: "tokens", group: "agent", granularity: "daily" });
    expect(root.querySelector(".hm")).toBeNull();

    // …and neither does the hourly granularity, even for a month.
    renderAnalytics(root, month, { metric: "tokens", group: "agent", granularity: "hourly" });
    expect(root.querySelector(".hm")).toBeNull();
  });

  it("renders the grayscale activity donut and project rows on Breakdown", () => {
    const a = mockAnalytics("week");
    const root = document.createElement("div");
    renderAnalytics(root, a, { metric: "tokens", group: "agent", granularity: "daily" });
    const donut = root.querySelector(".donutsec svg");
    expect(donut).not.toBeNull();
    expect(donut?.tagName).toBe("svg");
    // One arc per kind + the base ring.
    expect(root.querySelectorAll(".donutsec circle")).toHaveLength(a.byKind.length + 1);
    // Breakdown carries ranked `.rows` (model/agent, projects) with token·% labels.
    expect(root.querySelector(".rows")).not.toBeNull();
    expect(root.querySelector(".vl")?.textContent).toMatch(/·/);
    // The single Breakdown magenta is the #1 row; nothing else gets `.top`.
    expect(root.querySelectorAll(".row.top")).toHaveLength(1);
  });

  it("keeps the donut and composition fully grayscale (no magenta/provider colors)", () => {
    const a = mockAnalytics("week");
    const root = document.createElement("div");
    renderAnalytics(root, a, { metric: "tokens", group: "agent", granularity: "daily" });
    const inlineColors = [
      ...root.querySelectorAll<HTMLElement>(".legend i, .complegend i, .compbar i"),
    ].map((dot) => dot.style.background || dot.style.stroke);
    expect(inlineColors.length).toBeGreaterThan(0);
    expect(
      inlineColors.every((c) => /^var\(--(?:ink-\d+|faint)\)$/.test(c)),
    ).toBe(true);
  });

  it("renders all expanded activity-kind labels in both locales", () => {
    const a = mockAnalytics("week");
    a.byKind = ["edit", "read", "search", "run", "web", "agent", "mcp", "other"].map(
      (kind, index) => ({ kind, tokens: 8 - index }),
    );
    const root = document.createElement("div");

    try {
      setLocale("en");
      renderAnalytics(root, a, { metric: "tokens", group: "agent", granularity: "daily" });
      expect(root.querySelector(".donutsec .legend")?.textContent).toContain("Search");
      expect(root.querySelector(".donutsec .legend")?.textContent).toContain("Web");
      expect(root.querySelector(".donutsec .legend")?.textContent).toContain("Agent");
      expect(root.querySelector(".donutsec .legend")?.textContent).toContain("MCP");
      expect(root.querySelectorAll(".donutsec .legend span")).toHaveLength(8);

      setLocale("zh-TW");
      renderAnalytics(root, a, { metric: "tokens", group: "agent", granularity: "daily" });
      expect(root.querySelector(".donutsec .legend")?.textContent).toContain("搜尋");
      expect(root.querySelector(".donutsec .legend")?.textContent).toContain("網路");
      expect(root.querySelector(".donutsec .legend")?.textContent).toContain("代理");
      expect(root.querySelector(".donutsec .legend")?.textContent).toContain("MCP");
    } finally {
      setLocale("en");
    }
  });

  it("omits empty advanced sections instead of drawing blank cards", () => {
    const a = { ...mockAnalytics("week"), byKind: [], byProject: [] };
    const root = document.createElement("div");
    renderAnalytics(root, a, { metric: "tokens", group: "agent", granularity: "daily" });
    expect(root.querySelector(".donutsec")).toBeNull();
    // No project section, but the primary model/agent ranking is still there.
    expect(root.querySelector(".rows")).not.toBeNull();
  });

  it("never renders accounts in the analytics output (moved to Settings)", () => {
    const a = mockAnalytics("week");
    const root = document.createElement("div");
    renderAnalytics(root, a, { metric: "tokens", group: "agent", granularity: "daily" });
    // Accounts carry an email; it must not leak into the analytics pane anymore.
    expect(root.querySelector(".acct")).toBeNull();
    expect(root.textContent).not.toContain(a.accounts[0].account);
  });
});

describe("records/rate footnote (Trends)", () => {
  it("omits the peak-day fact when records are empty", () => {
    const a = mockAnalytics("week");
    a.records = {
      maxDay: { date: "", tokens: 0 },
      maxHour: { date: "", hour: 0, tokens: 0 },
      streakDays: 0,
      prNow: false,
    };
    const root = document.createElement("div");
    renderAnalytics(root, a, { metric: "tokens", group: "agent", granularity: "daily" });
    const foot = root.querySelector(".footnote")?.textContent ?? "";
    // No peak/busiest facts with zeroed records; the rate/session facts remain.
    expect(foot).not.toContain("Peak day");
    expect(foot).not.toContain("Busiest hour");
    expect(foot).toContain("sessions this week");
  });

  it("carries peak day + busiest hour + sessions + rate when records exist", () => {
    const a = mockAnalytics("week");
    a.records = {
      maxDay: { date: "2026-07-16", tokens: 2_400_000 },
      maxHour: { date: "2026-07-16", hour: 9, tokens: 800_000 },
      streakDays: 6,
      prNow: true,
    };
    const root = document.createElement("div");
    renderAnalytics(root, a, { metric: "tokens", group: "agent", granularity: "daily" });
    const foot = root.querySelector(".footnote")?.textContent ?? "";
    expect(foot).toContain("07-16"); // peak day (M-D)
    expect(foot).toContain("09:00"); // busiest hour
    expect(foot).toContain("sessions this week");
    // The streak is deduped into the hero, not repeated in the footnote.
    expect(root.querySelector(".hero .sub")?.textContent).toContain("6d streak");
  });
});

describe("metric price mode", () => {
  it("hourly price mode draws bars from hourlyCost with $ tooltips", () => {
    const a = { ...mockAnalytics("week"), hourly: Array(24).fill(0), hourlyCost: Array(24).fill(0) };
    a.hourlyCost[3] = 12.5;
    const root = document.createElement("div");
    renderAnalytics(root, a, { metric: "price", group: "agent", granularity: "hourly" });
    const titles = [...root.querySelectorAll(".chart title")].map((n) => n.textContent);
    expect(titles).toHaveLength(24);
    expect(titles[3]).toBe("3:00 · $12.50");
    // Every tooltip is a dollar amount — nothing token-formatted leaks in.
    expect(titles.every((t) => t?.includes("$"))).toBe(true);
  });

  it("hourly tokens mode keeps token tooltips (no $)", () => {
    const a = { ...mockAnalytics("week"), hourly: Array(24).fill(0) };
    a.hourly[5] = 2_000_000;
    const root = document.createElement("div");
    renderAnalytics(root, a, { metric: "tokens", group: "agent", granularity: "hourly" });
    const titles = [...root.querySelectorAll(".chart title")].map((n) => n.textContent);
    expect(titles[5]).toBe("5:00 · 2.0M");
    expect(titles.every((t) => !t?.includes("$"))).toBe(true);
  });

  it("share price mode labels bars with fmtUsd and cost share %", () => {
    const a = {
      ...mockAnalytics("week"),
      byAgent: { "Claude Code": 100, "Codex CLI": 100 },
      byAgentCost: { "Claude Code": 30, "Codex CLI": 10 },
      byKind: [],
      byProject: [],
    };
    const root = document.createElement("div");
    renderAnalytics(root, a, { metric: "price", group: "agent", granularity: "daily" });
    const vals = [...root.querySelectorAll(".rows .vl")].map((n) => n.textContent);
    // Sorted desc by cost: Claude $30 (75% of $40), Codex $10 (25%).
    expect(vals).toEqual(["$30.00 · 75%", "$10.00 · 25%"]);
  });

  it("share tokens mode ignores the cost fields entirely", () => {
    const a = {
      ...mockAnalytics("week"),
      byAgent: { "Claude Code": 3_000_000, "Codex CLI": 1_000_000 },
      byAgentCost: { "Claude Code": 999, "Codex CLI": 1 },
      byKind: [],
      byProject: [],
    };
    const root = document.createElement("div");
    renderAnalytics(root, a, { metric: "tokens", group: "agent", granularity: "daily" });
    const vals = [...root.querySelectorAll(".rows .vl")].map((n) => n.textContent);
    expect(vals).toEqual(["3.0M · 75%", "1.0M · 25%"]);
  });
});

describe("breakdown group toggle", () => {
  it("the ranking follows the model/agent group toggle", () => {
    const a = mockAnalytics("week");
    const root = document.createElement("div");

    renderAnalytics(root, a, { metric: "tokens", group: "agent", granularity: "daily" });
    const byAgent = root.innerHTML;
    renderAnalytics(root, a, { metric: "tokens", group: "model", granularity: "daily" });
    const byModel = root.innerHTML;

    // The agent view names agents (Claude Code / Codex CLI); the model view
    // names models (opus / gpt) — same Breakdown lens, switched by the toggle.
    expect(byAgent).toContain("Claude Code");
    expect(byModel).toContain("opus-4.8");
    expect(byAgent).not.toBe(byModel);
  });
});
