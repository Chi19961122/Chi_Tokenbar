// 階段 C Usage-tab quota summary tests. buildQuotaSummary is pure and drives
// the one-line digest; the render tests drive the real renderPanel output and
// locate things the way main.ts's click handler does (the [data-quota-toggle]
// button), not by asserting a selector equals itself.

import { describe, expect, it } from "vitest";
import { buildQuotaSummary, historicalPaceNote, renderPanel } from "./panel";
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

describe("historicalPaceNote (T-feat-007)", () => {
  it("renders nothing on the linear path (current behavior preserved)", () => {
    // No pace, linear pace, or missing runway all produce an empty string.
    expect(historicalPaceNote(limit({ pace: null, runway_secs: 1800 }))).toBe("");
    expect(
      historicalPaceNote(
        limit({ pace: { deficit: 0, in_deficit: false, pace_basis: "linear" }, runway_secs: 1800 }),
      ),
    ).toBe("");
    expect(
      historicalPaceNote(
        limit({ pace: { deficit: 0, in_deficit: false, pace_basis: "historical" }, runway_secs: null }),
      ),
    ).toBe("");
  });

  it("shows the runway + hist tag once the basis is historical", () => {
    const html = historicalPaceNote(
      limit({
        pace: { deficit: 0, in_deficit: false, pace_basis: "historical", run_out_probability: 0.2 },
        runway_secs: 3 * 3600 + 12 * 60,
      }),
    );
    expect(html).toContain("hist");
    expect(html).toContain("3h 12m");
    expect(html).not.toContain("hist-amber"); // prob < 0.5 → no amber
  });

  it("turns amber when run_out_probability ≥ 0.5", () => {
    const html = historicalPaceNote(
      limit({
        pace: { deficit: 0, in_deficit: false, pace_basis: "historical", run_out_probability: 0.5 },
        runway_secs: 1800,
      }),
    );
    expect(html).toContain("hist-amber");
  });
});

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

  // T-917: Grok's context-fill limit shows a "ctx" short in the digest, after
  // the two quota providers, and renders "—" when there is no reading.
  it("digests Grok's context limit as a trailing 'ctx' group", () => {
    const groups = buildQuotaSummary([
      ...LIMITS,
      limit({ id: "grok.ctx", provider: "grok", label: "Grok·Context", util: 55 }),
    ]);
    expect(groups.map((g) => g.name)).toEqual(["Claude", "Codex", "Grok"]);
    const grok = groups[2];
    expect(grok.segs).toEqual([{ short: "ctx", pct: "45%" }]);
  });

  it("shows Grok '—' when the context reading is insufficient, never a fake 0%", () => {
    const groups = buildQuotaSummary([
      limit({ id: "grok.ctx", provider: "grok", util: 0, status: "insufficient_data" }),
    ]);
    expect(groups[0].name).toBe("Grok");
    expect(groups[0].segs[0]).toEqual({ short: "ctx", pct: "—" });
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
    expect(root.querySelector(".status-row")).toBeNull(); // ϟ status row removed
    // The hero digits are the one place a row says "% left" — no small-type echo.
    expect(root.querySelector(".gauge-row .gauge-detail")?.textContent ?? "").not.toContain("% left");
    expect(root.querySelector(".section-head")).toBeNull(); // tabs are the page title
    expect(root.querySelectorAll(".gauge-card").length).toBe(2);
    expect(root.querySelector(".gauge-card.prov-claude .picon")).not.toBeNull();
    expect(root.querySelector(".gauge-card.prov-claude .picon")?.getAttribute("width")).toBe("14");
    expect(root.querySelector(".gauge-card.prov-codex .gauge-card-status")?.textContent).toContain("locked");
    expect(root.querySelector(".gauge-row .gauge-value")?.textContent).toBe("62");
    expect(root.querySelector(".gauge-row .gauge-unit")?.textContent).toBe("%");
    expect(root.querySelector(".gauge-row .gauge-fill")?.getAttribute("style")).toContain("width:62%");
  });

  it("renders a Grok card with a per-session note instead of a reset time", () => {
    const root = document.createElement("div");
    renderPanel(root, snap([
      limit({ id: "cc.5h", provider: "anthropic", util: 30 }),
      limit({ id: "grok.ctx", provider: "grok", label: "Grok·Context", util: 55, resets_at: 0, window_secs: 0 }),
    ]), { ...baseOpts, variant: "full" });

    // Three cards? No — only Claude + Grok are present, each its own gauge card.
    expect(root.querySelector(".gauge-card.prov-grok")).not.toBeNull();
    // The Grok row shows the honest per-session note, never a "Resets…" line.
    const grokRow = root.querySelector(".gauge-card.prov-grok .gauge-row");
    const note = grokRow?.querySelector(".gauge-reset")?.textContent ?? "";
    expect(note).toBe("This conversation's memory; a new chat starts at 0%");
    expect(note).not.toContain("Resets");
    // Context fill 55% → 45% left in the hero digits.
    expect(grokRow?.querySelector(".gauge-value")?.textContent).toBe("45");
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
