// 階段 C analytics decision-logic tests: the share-of-range label and the
// month start-date annotation condition. These pin behaviour (exact strings /
// when the note appears), not the shape of the implementation — flipping the
// condition or the denominator must turn one of these red.

import { describe, expect, it } from "vitest";
import { monthStartNote, renderAnalytics, sharePct, shareLabel } from "./analytics";
import type { Analytics } from "./types";
import { mockAnalytics } from "./mock";

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
    // Leading empty days dropped → the axis starts at the first active day.
    const firstAxis = root.querySelector(".chart .axis")?.textContent;
    expect(firstAxis).toBe(a.rangeStartDay.slice(5));
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
