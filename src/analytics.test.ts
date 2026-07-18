// 階段 C analytics decision-logic tests: the share-of-range label and the
// month start-date annotation condition. These pin behaviour (exact strings /
// when the note appears), not the shape of the implementation — flipping the
// condition or the denominator must turn one of these red.

import { describe, expect, it } from "vitest";
import {
  heatCells,
  monthStartNote,
  renderAnalytics,
  sharePct,
  shareLabel,
} from "./analytics";
import type { Analytics, DayPoint } from "./types";
import { mockAnalytics } from "./mock";

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
    renderAnalytics(root, a, { subtab: "overview", metric: "tokens", group: "agent" });

    expect(root.querySelector(".chart-note")?.textContent).toContain(a.rangeStartDay.slice(5));
    // Leading empty days are still dropped, while the editorial axis stays minimal.
    expect(root.querySelectorAll(".daily-bar")).toHaveLength(2);
    expect([...root.querySelectorAll(".chart .axis")].map((n) => n.textContent)).toEqual([
      "30d ago",
      "today",
    ]);
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

describe("階段 C+ render wiring", () => {
  it("renders daily totals as neutral single bars with a pink today bar", () => {
    const a = mockAnalytics("month");
    const root = document.createElement("div");
    renderAnalytics(root, a, { subtab: "overview", metric: "tokens", group: "agent" });

    const bars = [...root.querySelectorAll<SVGRectElement>(".daily-bar")];
    expect(bars).toHaveLength(a.daily.length);
    // Fill is now class-driven (theme-following), not an inline hex: the last bar
    // is the pink "today" bar; the rest are heavy ("is-strong") or dim (plain).
    expect(bars[bars.length - 1]?.classList.contains("is-today")).toBe(true);
    expect(bars.slice(0, -1).some((bar) => bar.classList.contains("is-today"))).toBe(false);
    expect(bars.slice(0, -1).every((bar) => !bar.hasAttribute("fill"))).toBe(true);
    expect(root.querySelector(".legend")).toBeNull();
  });

  it("shows the heatmap on overview only for the month range", () => {
    const month = { ...mockAnalytics("month"), range: "month" as const };
    const week = { ...mockAnalytics("week"), range: "week" as const };
    const root = document.createElement("div");

    renderAnalytics(root, month, { subtab: "overview", metric: "tokens", group: "agent" });
    expect(root.querySelector(".hm")).not.toBeNull();
    expect(root.querySelectorAll(".hm-today")).toHaveLength(1);

    renderAnalytics(root, week, { subtab: "overview", metric: "tokens", group: "agent" });
    expect(root.querySelector(".hm")).toBeNull();
  });

  it("renders the activity donut and project bars on Breakdown", () => {
    const a = mockAnalytics("week");
    const root = document.createElement("div");
    renderAnalytics(root, a, { subtab: "share", metric: "tokens", group: "agent" });
    expect(root.querySelector(".donut")).not.toBeNull();
    expect(root.querySelector(".donut")?.tagName).toBe("svg");
    expect(root.querySelectorAll(".donut circle")).toHaveLength(a.byKind.length + 1);
    // Project bars carry a token·% label (shareLabel), like the other bars.
    expect(root.querySelector(".sub-sec")).not.toBeNull();
  });

  it("omits empty advanced sections instead of drawing blank cards", () => {
    const a = { ...mockAnalytics("week"), byKind: [], byProject: [] };
    const root = document.createElement("div");
    renderAnalytics(root, a, { subtab: "share", metric: "tokens", group: "agent" });
    expect(root.querySelector(".donut")).toBeNull();
    expect(root.querySelector(".sub-sec")).toBeNull();
    // The primary model/agent breakdown is still there.
    expect(root.querySelector(".bars")).not.toBeNull();
  });
});

describe("personal records", () => {
  it("omits the whole records section when records are empty", () => {
    const a = mockAnalytics("week");
    a.records = {
      maxDay: { date: "", tokens: 0 },
      maxHour: { date: "", hour: 0, tokens: 0 },
      streakDays: 0,
      prNow: false,
    };
    const root = document.createElement("div");
    renderAnalytics(root, a, { subtab: "stats", metric: "tokens", group: "agent" });
    expect(root.querySelector(".records")).toBeNull();
  });

  it("renders three stat tiles with Est. Cost reversed", () => {
    const root = document.createElement("div");
    renderAnalytics(root, mockAnalytics("week"), { subtab: "overview", metric: "tokens", group: "agent" });
    expect(root.querySelectorAll(":scope > .tiles > .tile")).toHaveLength(3);
    expect(root.querySelector(":scope > .tiles > .tile")?.classList.contains("tile-accent")).toBe(true);
  });

  it("renders record values and PR badge", () => {
    const a = mockAnalytics("week");
    a.records = {
      maxDay: { date: "2026-07-16", tokens: 2_400_000 },
      maxHour: { date: "2026-07-16", hour: 9, tokens: 800_000 },
      streakDays: 6,
      prNow: true,
    };
    const root = document.createElement("div");
    renderAnalytics(root, a, { subtab: "stats", metric: "tokens", group: "agent" });
    expect(root.querySelector(".records")?.textContent).toContain("2.4M");
    expect(root.querySelector(".records")?.textContent).toContain("800.0K");
    expect(root.querySelector(".records")?.textContent).toContain("07-16 09:00");
    expect(root.querySelector(".pr-now")?.textContent).toBe("PR NOW");
  });
});

describe("metric price mode", () => {
  it("hourly price mode draws bars from hourlyCost with $ tooltips", () => {
    const a = { ...mockAnalytics("week"), hourly: Array(24).fill(0), hourlyCost: Array(24).fill(0) };
    a.hourlyCost[3] = 12.5;
    const root = document.createElement("div");
    renderAnalytics(root, a, { subtab: "hourly", metric: "price", group: "agent" });
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
    renderAnalytics(root, a, { subtab: "hourly", metric: "tokens", group: "agent" });
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
    renderAnalytics(root, a, { subtab: "share", metric: "price", group: "agent" });
    const vals = [...root.querySelectorAll(".bar-val")].map((n) => n.textContent);
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
    renderAnalytics(root, a, { subtab: "share", metric: "tokens", group: "agent" });
    const vals = [...root.querySelectorAll(".bar-val")].map((n) => n.textContent);
    expect(vals).toEqual(["3.0M · 75%", "1.0M · 25%"]);
  });
});

describe("subtab convergence", () => {
  it("share breakdown follows the model/agent group toggle", () => {
    const a = mockAnalytics("week");
    const root = document.createElement("div");

    renderAnalytics(root, a, { subtab: "share", metric: "tokens", group: "agent" });
    const byAgent = root.innerHTML;
    renderAnalytics(root, a, { subtab: "share", metric: "tokens", group: "model" });
    const byModel = root.innerHTML;

    // The agent view names agents (Claude Code / Codex CLI); the model view
    // names models (opus / gpt) — the same subtab, switched by the toggle.
    expect(byAgent).toContain("Claude Code");
    expect(byModel).toContain("opus-4.8");
    expect(byAgent).not.toBe(byModel);
  });
});
