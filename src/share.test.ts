// 階段 D 戰報 Share — data layer + card rendering tests (jsdom).
//
// share.ts is pure and locale-parameterized, so these drive it directly with a
// fixed fake Analytics and assert on the rendered DOM — no global i18n state.
// T-915 redesign: decision-logic tests (quota gauge / genMonthYear+docNo /
// sparkline scaling / splits) are kept meaningful; markup assertions target the
// ported `.shXX-card` structure.

import { describe, expect, it } from "vitest";
import type { Analytics, Limit } from "./types";
import { buildShareData, renderShareCard, type ShareStyle } from "./share";
import { fmtTokens } from "./format";

// A fixed fake so every number is assertable. byAgent sums to totalTokens; one
// zero-token agent is present to prove the tokens>0 filter. byModel is distinct
// so the fuel card (model-grouped) can be told apart from the agent cards.
function fakeAnalytics(over: Partial<Analytics> = {}): Analytics {
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
    ...over,
  };
}

// Claude 5h + Claude week (anthropic). Codex week added in a dedicated test.
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

  it("threads through the authorized numeric fields", () => {
    const d = buildShareData(fakeAnalytics(), { range: "week", locale: "en" });
    expect(d.streakDays).toBe(7);
    expect(d.maxDayTokens).toBe(2_400_000);
    expect(d.sessionCount).toBe(3);
    expect(d.peakHour).toBe(15);
    expect(d.hourly).toHaveLength(24);
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

  it("genMonthYear is fixed uppercase MON YYYY from the period's last day (both locales)", () => {
    const en = buildShareData(fakeAnalytics(), { range: "week", locale: "en" });
    const zh = buildShareData(fakeAnalytics(), { range: "week", locale: "zh-TW" });
    expect(en.genMonthYear).toBe("JUL 2026");
    expect(zh.genMonthYear).toBe("JUL 2026"); // never toLocale — fixed English month
  });

  it("docNo is TB-YYYY-MMDD from the period's last day", () => {
    const d = buildShareData(fakeAnalytics(), { range: "week", locale: "en" });
    expect(d.docNo).toBe("TB-2026-0716");
  });

  it("omits genMonthYear / docNo when there is no daily data", () => {
    const d = buildShareData(fakeAnalytics({ daily: [] }), { range: "week", locale: "en" });
    expect(d.genMonthYear).toBeUndefined();
    expect(d.docNo).toBeUndefined();
  });
});

describe("quotaGauge (island_card only, USED %)", () => {
  it("is undefined when includeQuotaNote is false", () => {
    const d = buildShareData(fakeAnalytics(), {
      range: "week",
      locale: "en",
      limits: LIMITS,
      includeQuotaNote: false,
    });
    expect(d.quotaGauge).toBeUndefined();
  });

  it("builds Claude 5h + Claude week rows carrying USED util (not % left)", () => {
    const d = buildShareData(fakeAnalytics(), {
      range: "week",
      locale: "en",
      limits: LIMITS,
      includeQuotaNote: true,
    });
    expect(d.quotaGauge).toEqual([
      { label: "Claude · 5h", util: 72 },
      { label: "Claude · week", util: 41 },
    ]);
  });

  it("orders Claude 5h, Claude week, Codex week and caps at 3", () => {
    const codexWeek: Limit = { ...LIMITS[1], id: "cx.week", provider: "codex", util: 55 };
    const d = buildShareData(fakeAnalytics(), {
      range: "week",
      locale: "en",
      limits: [codexWeek, ...LIMITS],
      includeQuotaNote: true,
    });
    expect(d.quotaGauge).toEqual([
      { label: "Claude · 5h", util: 72 },
      { label: "Claude · week", util: 41 },
      { label: "Codex · week", util: 55 },
    ]);
  });

  it("localizes the week descriptor (週) but keeps the brand fixed English", () => {
    const d = buildShareData(fakeAnalytics(), {
      range: "week",
      locale: "zh-TW",
      limits: LIMITS,
      includeQuotaNote: true,
    });
    expect(d.quotaGauge?.[1].label).toBe("Claude · 週");
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
      // total tokens: grouped ("8,204,113") for the statement ledger, abbreviated
      // ("8.2M") for the hero numerals — assert either representation is present.
      const grouped = (8_204_113).toLocaleString("en-US");
      expect(
        txt.includes(grouped) || txt.includes(fmtTokens(8_204_113)),
        `${style} should show the total tokens`,
      ).toBe(true);
      // est cost (statement/minimal/fuel/wa show "$47.20"; diagnostics "47.20").
      expect(txt).toContain("47.20");
      // a split row name — fuel is model-grouped (uppercased); island_card shows
      // the quota gauge (no agent splits) so it carries a brand label; the rest
      // carry the top agent.
      if (style === "fuel") expect(txt).toContain("SONNET-5");
      else if (style === "island_card") expect(txt).toContain("Claude");
      else expect(txt).toContain("main");
    });
  }

  it("fuel honours fuelGroup=agent (agent names, uppercased) and numbers grades", () => {
    const card = renderShareCard("fuel", data, "en", { fuelGroup: "agent" });
    expect(card.textContent ?? "").toContain("MAIN");
    // grade numbers 01..04 are literal, untranslated
    expect(card.querySelector(".fl-row .gr")?.textContent).toBe("01");
  });

  it("island_card renders the quota gauge rows only when a gauge is present", () => {
    const withGauge = renderShareCard("island_card", data, "en");
    const rows = withGauge.querySelectorAll(".ic-qrow");
    expect(rows.length).toBe(2);
    // USED % (util), not % left: Claude 5h util 72 → "72%".
    expect(withGauge.querySelector(".ic-qval")?.textContent).toContain("72%");
    // fill width + fixed used-% color
    const fill = withGauge.querySelector<HTMLElement>(".ic-fill");
    expect(fill?.style.width).toBe("72%");
    expect(fill?.style.background).toContain("rgb(24, 24, 27)"); // #18181B

    const noGauge = buildShareData(fakeAnalytics(), { range: "week", locale: "en" });
    const card = renderShareCard("island_card", noGauge, "en");
    expect(card.querySelectorAll(".ic-qrow").length).toBe(0);
  });

  it("diagnostics scales the 24h sparkline and flags the peak bar", () => {
    const hourly = new Array(24).fill(0);
    hourly[9] = 50;
    hourly[14] = 100; // peak
    hourly[18] = 25;
    const d = buildShareData(fakeAnalytics({ hourly }), { range: "week", locale: "en" });
    const card = renderShareCard("diagnostics", d, "en");
    const bars = card.querySelectorAll<HTMLElement>(".dx-spark .bars i");
    expect(bars.length).toBe(24);
    expect(bars[14].classList.contains("pk")).toBe(true);
    // jsdom normalizes "100.0%" → "100%" when re-serializing inline styles.
    expect(bars[14].style.height).toBe("100%");
    expect(bars[9].style.height).toBe("50%"); // 50/100
    expect(bars[18].style.height).toBe("25%");
    // only one peak bar
    expect(card.querySelectorAll(".dx-spark .bars i.pk").length).toBe(1);
  });

  it("diagnostics renders a flat sparkline when hourly is all-zero", () => {
    const card = renderShareCard("diagnostics", data, "en"); // fake hourly is all-zero
    const bars = card.querySelectorAll<HTMLElement>(".dx-spark .bars i");
    expect(bars.length).toBe(24);
    expect(card.querySelectorAll(".dx-spark .bars i.pk").length).toBe(0);
    expect(bars[0].style.height).toBe("0%");
  });

  it("renders the peak hour as zero-padded HH:00", () => {
    const card = renderShareCard("minimal", data, "en");
    expect(card.textContent ?? "").toContain("15:00");
  });

  it("renders locale-specific labels (en vs zh-TW)", () => {
    const zhData = buildShareData(fakeAnalytics(), { range: "week", locale: "zh-TW" });
    const en = renderShareCard("statement", data, "en");
    const zh = renderShareCard("statement", zhData, "zh-TW");
    expect(en.textContent ?? "").toContain("This week");
    expect(zh.textContent ?? "").toContain("本週");
    expect(zh.textContent ?? "").toContain("用量結算單");
  });

  it("statement hero subline carries agents/sessions/streak/peak, dropping empties", () => {
    const stmt = renderShareCard("statement", data, "en");
    const sub = stmt.querySelector(".st-tsub")?.textContent ?? "";
    expect(sub).toContain("across 5 agents");
    expect(sub).toContain("3 sessions");
    expect(sub).toContain("streak 7d");
    expect(sub).toContain("peak 2.4M/day");

    const bare = { ...data, streakDays: 0, maxDayTokens: 0, sessionCount: 0 };
    const stmt2 = renderShareCard("statement", bare, "en");
    const sub2 = stmt2.querySelector(".st-tsub")?.textContent ?? "";
    expect(sub2).toContain("across 5 agents");
    expect(sub2).not.toContain("streak");
    expect(sub2).not.toContain("peak");
    expect(sub2).not.toContain("sessions");
  });

  it("every card carries the unified signature (battery + TokenBar)", () => {
    for (const style of ALL_STYLES) {
      const card = renderShareCard(style, data, "en", { fuelGroup: "model" });
      expect(card.textContent ?? "", `${style} brand`).toContain("TokenBar");
      expect(card.querySelector(".batt, .fl-pump, .pb"), `${style} mark`).not.toBeNull();
    }
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
