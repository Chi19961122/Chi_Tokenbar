// 階段 C Usage-tab quota summary tests. buildQuotaSummary is pure and drives
// the one-line digest; the render tests drive the real renderPanel output and
// locate things the way main.ts's click handler does (the [data-quota-toggle]
// button), not by asserting a selector equals itself.

import { describe, expect, it } from "vitest";
import { buildQuotaSummary, renderPanel } from "./panel";
import type { Limit, Snapshot } from "./types";

function limit(over: Partial<Limit>): Limit {
  return {
    id: "cc.5h",
    provider: "anthropic",
    label: "Claude·5h",
    util: 0,
    resets_at: 0,
    window_secs: 5 * 3600,
    status: "normal",
    absolute: null,
    pace: null,
    runway_secs: null,
    ...over,
  };
}

const LIMITS: Limit[] = [
  limit({ id: "cc.5h", provider: "anthropic", util: 38 }), // 62% left
  limit({ id: "cc.week", provider: "anthropic", util: 82 }), // 18% left
  limit({ id: "codex.week", provider: "codex", util: 100, status: "locked" }), // 0% left
];

const snap = (limits: Limit[]): Snapshot => ({
  limits,
  worst_id: null,
  updated_at: 0,
  next_fetch_in: 30,
});

const baseOpts = { resetDisplay: "relative" as const, now: 0, locale: "en" as const };

describe("buildQuotaSummary", () => {
  it("digests each provider's windows into fixed-English short labels + % left", () => {
    const groups = buildQuotaSummary(LIMITS);
    expect(groups.map((g) => g.name)).toEqual(["Claude", "Codex"]);

    const claude = groups[0];
    expect(claude.segs).toEqual([
      { short: "5h", pct: "62%" },
      { short: "wk", pct: "18%" },
    ]);
    expect(groups[1].segs).toEqual([{ short: "wk", pct: "0%" }]);
  });

  it("shows '—' for an unavailable reading rather than a fake 0%", () => {
    const groups = buildQuotaSummary([
      limit({ id: "cc.5h", provider: "anthropic", util: 0, status: "source_failed" }),
    ]);
    expect(groups[0].segs[0].pct).toBe("—");
  });

  it("omits a provider with no limits (no empty group)", () => {
    const groups = buildQuotaSummary([limit({ provider: "codex", id: "codex.5h" })]);
    expect(groups.map((g) => g.name)).toEqual(["Codex"]);
  });
});

describe("renderPanel summary variant", () => {
  it("collapses to a single toggle button, no full list", () => {
    const root = document.createElement("div");
    renderPanel(root, snap(LIMITS), { ...baseOpts, variant: "summary", summaryExpanded: false });

    expect(root.querySelector("[data-quota-toggle]")).not.toBeNull();
    expect(root.querySelector(".gauge-row")).toBeNull(); // full list is hidden
    expect(root.textContent).toContain("Claude");
    expect(root.textContent).toContain("62%");
  });

  it("expands to show the full gauge list beneath the toggle", () => {
    const root = document.createElement("div");
    renderPanel(root, snap(LIMITS), { ...baseOpts, variant: "summary", summaryExpanded: true });

    expect(root.querySelector("[data-quota-toggle]")).not.toBeNull();
    expect(root.querySelectorAll(".gauge-row").length).toBe(LIMITS.length);
  });

  it("full variant renders the list directly, with no summary toggle", () => {
    const root = document.createElement("div");
    renderPanel(root, snap(LIMITS), { ...baseOpts, variant: "full" });

    expect(root.querySelector("[data-quota-toggle]")).toBeNull();
    expect(root.querySelectorAll(".gauge-row").length).toBe(LIMITS.length);
    expect(root.querySelector(".status-pill")?.textContent).toContain("0% left");
    expect(root.querySelector(".section-number")?.textContent).toBe("01");
    expect(root.querySelector(".section-editorial")?.textContent).toBe("What's left in the tank");
    expect(root.querySelectorAll(".gauge-card").length).toBe(2);
    expect(root.querySelector(".gauge-card.prov-claude .picon")).not.toBeNull();
    expect(root.querySelector(".gauge-card.prov-claude .picon")?.getAttribute("width")).toBe("14");
    expect(root.querySelector(".gauge-card.prov-codex .gauge-card-status")?.textContent).toContain("locked");
    expect(root.querySelector(".gauge-row .gauge-value")?.textContent).toBe("62");
    expect(root.querySelector(".gauge-row .gauge-unit")?.textContent).toBe("%");
    expect(root.querySelector(".gauge-row .gauge-fill")?.getAttribute("style")).toContain("width:62%");
  });

  it("keeps unavailable data unknown and marks its provider degraded", () => {
    const root = document.createElement("div");
    renderPanel(root, snap([
      limit({ status: "source_failed", util: 0, hint: "offline" }),
      limit({ id: "cc.week", util: 40 }),
    ]), { ...baseOpts, variant: "full" });

    expect(root.querySelector(".gauge-card-status")?.textContent).toContain("degraded");
    expect(root.querySelector(".gauge-state-degraded .gauge-value")?.textContent).toBe("—");
    expect(root.querySelector(".gauge-state-degraded .gauge-unit")).toBeNull();
    expect(root.querySelector(".gauge-state-degraded .gauge-fill")?.getAttribute("style")).toContain("width:0%");
  });
});
