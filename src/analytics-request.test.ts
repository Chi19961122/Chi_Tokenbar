import { describe, expect, it, vi } from "vitest";
import { createAnalyticsRequestGate } from "./analytics-request";

/** Production slice keys: range|sources (no updated_at). */
function sliceKey(range: string, sources = "claude,codex,grok"): string {
  return `${range}|${sources}`;
}

describe("analytics request generation (production keys)", () => {
  it("today → week → month out-of-order: only month commits", () => {
    const gate = createAnalyticsRequestGate();
    const today = sliceKey("today");
    const week = sliceKey("week");
    const month = sliceKey("month");

    // Independent lanes: each range has its own generation counter.
    const gToday = gate.begin(today);
    const gWeek = gate.begin(week);
    const gMonth = gate.begin(month);

    // Stale revalidation of today must not win after a newer today begin.
    const gToday2 = gate.begin(today);
    expect(gate.decide(today, gToday)).toBe("stale");
    expect(gate.decide(today, gToday2)).toBe("commit");

    const cache = new Map<string, string>();
    const paint = vi.fn((range: string) => {
      cache.set(range, range);
    });

    const finish = (lane: string, gen: number, range: string) => {
      if (gate.decide(lane, gen) !== "commit") return;
      paint(range);
    };

    // Completions out of order across *different* production keys.
    finish(week, gWeek, "week");
    finish(today, gToday, "today"); // stale vs gToday2
    finish(month, gMonth, "month");
    finish(today, gToday2, "today");

    expect(cache.get("week")).toBe("week");
    expect(cache.get("month")).toBe("month");
    expect(cache.get("today")).toBe("today");
    // today painted once (only gToday2), not the stale gToday
    expect(paint.mock.calls.filter((c) => c[0] === "today")).toHaveLength(1);
  });

  it("same slice: only newest generation paints (race on one range)", () => {
    const gate = createAnalyticsRequestGate();
    const lane = sliceKey("week");
    const g1 = gate.begin(lane);
    const g2 = gate.begin(lane);
    const paint = vi.fn();
    if (gate.decide(lane, g1) === "commit") paint("old");
    if (gate.decide(lane, g2) === "commit") paint("new");
    expect(paint).toHaveBeenCalledTimes(1);
    expect(paint).toHaveBeenCalledWith("new");
  });

  it("isCurrent tracks the latest begin per lane", () => {
    const gate = createAnalyticsRequestGate();
    const a = gate.begin(sliceKey("today"));
    expect(gate.isCurrent(sliceKey("today"), a)).toBe(true);
    const b = gate.begin(sliceKey("today"));
    expect(gate.isCurrent(sliceKey("today"), a)).toBe(false);
    expect(gate.isCurrent(sliceKey("today"), b)).toBe(true);
  });
});
