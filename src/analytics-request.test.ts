import { describe, expect, it, vi } from "vitest";
import { createAnalyticsRequestGate } from "./analytics-request";

describe("analytics request generation", () => {
  it("only the newest generation may commit (today → week → month race)", () => {
    const gate = createAnalyticsRequestGate();
    const key = "range"; // logical cache key family for the race

    const gToday = gate.begin(key);
    const gWeek = gate.begin(key);
    const gMonth = gate.begin(key);

    expect(gate.decide(key, gToday)).toBe("stale");
    expect(gate.decide(key, gWeek)).toBe("stale");
    expect(gate.decide(key, gMonth)).toBe("commit");

    const cache = new Map<string, string>();
    const paint = vi.fn();
    const finish = (gen: number, range: string) => {
      if (gate.decide(key, gen) !== "commit") return;
      cache.set(key, range);
      paint(range);
    };

    // Out-of-order completion: week, today, then month.
    finish(gWeek, "week");
    finish(gToday, "today");
    finish(gMonth, "month");

    expect(cache.get(key)).toBe("month");
    expect(paint).toHaveBeenCalledTimes(1);
    expect(paint).toHaveBeenCalledWith("month");
  });

  it("isCurrent tracks the latest begin", () => {
    const gate = createAnalyticsRequestGate();
    const a = gate.begin("k");
    expect(gate.isCurrent("k", a)).toBe(true);
    const b = gate.begin("k");
    expect(gate.isCurrent("k", a)).toBe(false);
    expect(gate.isCurrent("k", b)).toBe(true);
  });
});
