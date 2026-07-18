// 階段 D 戰報 Share — data layer + card rendering tests (jsdom).
//
// share.ts is pure and locale-parameterized, so these drive it directly with a
// fixed fake Analytics and assert on the rendered DOM — no global i18n state.

import { describe, expect, it } from "vitest";
import type { Analytics, Limit } from "./types";
import { buildShareData, renderShareCard, type ShareStyle } from "./share";
import { fmtTokens } from "./format";

// A fixed fake so every number is assertable. byAgent sums to totalTokens; one
// zero-token agent is present to prove the tokens>0 filter. byModel is distinct
// so the fuel card (model-grouped) can be told apart from the agent cards.
function fakeAnalytics(): Analytics {
  const daily = ["10", "11", "12", "13", "14", "15", "16"].map((d) => ({
    date: `2026-07-${d}`,
    byModel: {},
    byAgent: {},
    costUsd: 0,
  }));
  return {
    range: "week",
    rangeStartDay: "2026-07-10",
    totalTokens: 8_204_113,
    totalCostUsd: 47.2,
    bestDay: { date: "2026-07-14", costUsd: 12 },
    activeDays: 7,
    records: {
      maxDay: { date: "2026-07-14", tokens: 2_400_000 },
      maxHour: { date: "2026-07-14", hour: 15, tokens: 800_000 },
      streakDays: 7,
      prNow: false,
    },
    daily,
    hourly: new Array(24).fill(0),
    hourlyCost: new Array(24).fill(0),
    byModel: { "sonnet-5": 6_204_113, "opus-4.8": 2_000_000 },
    byAgent: {
      main: 3_600_412,
      executor: 2_380_193,
      scout: 1_394_700,
      verifier: 492_247,
      codex: 336_561,
      idle: 0,
    },
    byModelCost: { "sonnet-5": 30, "opus-4.8": 17.2 },
    byAgentCost: {
      main: 20,
      executor: 13,
      scout: 8,
      verifier: 3,
      codex: 3.2,
      idle: 0,
    },
    breakdown: { input: 1, cached: 1, output: 1, reasoning: 1 },
    byKind: [],
    byProject: [{ name: "secret-project", tokens: 5_000_000 }],
    sessionsThisWeek: 3,
    tokPerMin: 1000,
    accounts: [],
  };
}

const LIMITS: Limit[] = [
  {
    id: "cc.5h",
    provider: "anthropic",
    label: "Claude·5h",
    util: 72,
    resets_at: 0,
    window_secs: 5 * 3600,
    status: "normal",
    absolute: null,
    pace: null,
    runway_secs: null,
  },
  {
    id: "cc.week",
    provider: "anthropic",
    label: "Claude·Weekly",
    util: 41,
    resets_at: 0,
    window_secs: 7 * 86400,
    status: "normal",
    absolute: null,
    pace: null,
    runway_secs: null,
  },
];

const ALL_STYLES: ShareStyle[] = [
  "statement",
  "diagnostics",
  "minimal",
  "fuel",
  "island_card",
  "wa",
];

describe("buildShareData contract", () => {
  it("excludes tokens===0 entries and sorts desc", () => {
    const d = buildShareData(fakeAnalytics(), { range: "week", locale: "en" });
    expect(d.byAgent.length).toBe(5); // 'idle' (0 tokens) dropped
    expect(d.byAgent.map((s) => s.name)).toEqual([
      "main",
      "executor",
      "scout",
      "verifier",
      "codex",
    ]);
    expect(d.agentCount).toBe(5);
  });

  it("never exposes byProject / project data (§0)", () => {
    const d = buildShareData(fakeAnalytics(), { range: "week", locale: "en" });
    expect(Object.keys(d)).not.toContain("byProject");
    expect(JSON.stringify(d)).not.toContain("secret-project");
  });

  it("exposes only the two authorized numeric record fields", () => {
    const d = buildShareData(fakeAnalytics(), { range: "week", locale: "en" });
    expect(d.streakDays).toBe(7);
    expect(d.maxDayTokens).toBe(2_400_000);
    expect(JSON.stringify(d)).not.toContain("secret-project");
  });

  it("pct is share of the period total", () => {
    const d = buildShareData(fakeAnalytics(), { range: "week", locale: "en" });
    expect(d.byAgent[0].pct).toBe(44); // 3,600,412 / 8,204,113 ≈ 43.9 → 44
    expect(d.byModel[0].pct).toBe(76); // 6,204,113 / 8,204,113 ≈ 75.6 → 76
  });

  it("periodLabel is locale-aware with fixed month table", () => {
    const en = buildShareData(fakeAnalytics(), { range: "week", locale: "en" });
    const zh = buildShareData(fakeAnalytics(), { range: "week", locale: "zh-TW" });
    expect(en.periodLabel).toBe("This week · Jul 10 - 16");
    expect(zh.periodLabel).toBe("本週 · 7月10日 - 16日");
  });
});

describe("quotaNote toggle", () => {
  it("is undefined when includeQuotaNote is false", () => {
    const d = buildShareData(fakeAnalytics(), {
      range: "week",
      locale: "en",
      limits: LIMITS,
      includeQuotaNote: false,
    });
    expect(d.quotaNote).toBeUndefined();
  });

  it("is defined and carries 'left' (en) when enabled with limits", () => {
    const d = buildShareData(fakeAnalytics(), {
      range: "week",
      locale: "en",
      limits: LIMITS,
      includeQuotaNote: true,
    });
    expect(d.quotaNote).toBeDefined();
    expect(d.quotaNote).toContain("left");
    expect(d.quotaNote).toContain("28%"); // pctLeft(72)
    expect(d.quotaNote).toContain("59%"); // pctLeft(41)
  });

  it("carries '剩' in zh-TW", () => {
    const d = buildShareData(fakeAnalytics(), {
      range: "week",
      locale: "zh-TW",
      limits: LIMITS,
      includeQuotaNote: true,
    });
    expect(d.quotaNote).toContain("剩");
  });
});

describe("renderShareCard — all six styles", () => {
  const data = buildShareData(fakeAnalytics(), {
    range: "week",
    locale: "en",
    limits: LIMITS,
    includeQuotaNote: true,
  });

  for (const style of ALL_STYLES) {
    it(`${style}: shows total tokens, est cost and a split row`, () => {
      const card = renderShareCard(style, data, "en", { fuelGroup: "model" });
      const txt = card.textContent ?? "";
      // total tokens: grouped ("8,204,113") for the digit cards, abbreviated
      // ("8.2M") for minimal/island_card — assert either representation.
      const grouped = (8_204_113).toLocaleString("en-US");
      expect(
        txt.includes(grouped) || txt.includes(fmtTokens(8_204_113)),
        `${style} should show the total tokens`,
      ).toBe(true);
      // est cost (statement/minimal/fuel/wa show "$47.20"; diagnostics "47.20").
      expect(txt).toContain("47.20");
      // a split row name — fuel is model-grouped (uppercased), the rest agent.
      if (style === "fuel") expect(txt).toContain("SONNET-5");
      else expect(txt).toContain("main");
    });
  }

  it("fuel honours fuelGroup=agent (agent names, uppercased)", () => {
    const card = renderShareCard("fuel", data, "en", { fuelGroup: "agent" });
    expect(card.textContent ?? "").toContain("MAIN");
  });

  it("island_card renders the quota note only when present", () => {
    const withNote = renderShareCard("island_card", data, "en");
    expect(withNote.querySelector(".shic-note")).not.toBeNull();
    const noNote = buildShareData(fakeAnalytics(), { range: "week", locale: "en" });
    const card = renderShareCard("island_card", noNote, "en");
    expect(card.querySelector(".shic-note")).toBeNull();
  });

  it("renders locale-specific labels (en vs zh-TW)", () => {
    const zhData = buildShareData(fakeAnalytics(), { range: "week", locale: "zh-TW" });
    const en = renderShareCard("statement", data, "en");
    const zh = renderShareCard("statement", zhData, "zh-TW");
    expect(en.textContent ?? "").toContain("This week");
    expect(zh.textContent ?? "").toContain("本週");
    expect(zh.textContent ?? "").toContain("用量結算單");
  });

  it("shows records only in the two stats-oriented templates", () => {
    const records = "7d streak · peak 2.4M";
    for (const style of ALL_STYLES) {
      const card = renderShareCard(style, data, "en", { fuelGroup: "model" });
      if (style === "statement" || style === "diagnostics") {
        expect(card.textContent ?? "", `${style} should show records`).toContain(records);
      } else {
        expect(card.textContent ?? "", `${style} should not show records`).not.toContain(records);
      }
    }
  });

  it("omits the records caption when both record values are empty", () => {
    const empty = { ...data, streakDays: 0, maxDayTokens: 0 };
    expect(renderShareCard("statement", empty, "en").querySelector(".sh-records")).toBeNull();
    expect(renderShareCard("diagnostics", empty, "en").querySelector(".sh-records")).toBeNull();
  });

  it("never renders project names in any template", () => {
    for (const style of ALL_STYLES) {
      const card = renderShareCard(style, data, "en", { fuelGroup: "model" });
      expect(card.textContent ?? "").not.toContain("secret-project");
    }
  });

  it("adds the sh-916 class only for the story size (T-905)", () => {
    for (const style of ALL_STYLES) {
      const auto = renderShareCard(style, data, "en", { fuelGroup: "model", size: "auto" });
      expect(auto.classList.contains("sh-916"), `${style} auto`).toBe(false);
      const story = renderShareCard(style, data, "en", { fuelGroup: "model", size: "story" });
      expect(story.classList.contains("sh-916"), `${style} story`).toBe(true);
      // The story variant keeps its template identity + the same content.
      expect(story.className).toContain(auto.className.split(" ")[0]);
      expect(story.textContent ?? "").not.toContain("secret-project");
    }
  });

  it("defaults to auto (no sh-916) when size is omitted", () => {
    const card = renderShareCard("statement", data, "en");
    expect(card.classList.contains("sh-916")).toBe(false);
  });
});
