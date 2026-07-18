import { afterEach, describe, expect, it, vi } from "vitest";
import type { Analytics } from "./types";

const mocks = vi.hoisted(() => ({
  invoke: vi.fn(),
  toPng: vi.fn(),
  toBlob: vi.fn(),
}));

vi.mock("./datasource", () => ({ isTauri: () => true }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("html-to-image", () => ({ toPng: mocks.toPng, toBlob: mocks.toBlob }));

import { renderSharePanel } from "./share-panel";

function analytics(): Analytics {
  return {
    range: "week",
    rangeStartDay: "2026-07-12",
    totalTokens: 1000,
    totalCostUsd: 1,
    bestDay: { date: "2026-07-18", costUsd: 1 },
    activeDays: 1,
    records: {
      maxDay: { date: "2026-07-18", tokens: 1000 },
      maxHour: { date: "2026-07-18", hour: 12, tokens: 1000 },
      streakDays: 1,
      prNow: false,
    },
    daily: [],
    hourly: new Array(24).fill(0),
    hourlyCost: new Array(24).fill(0),
    byModel: { model: 1000 },
    byAgent: { agent: 1000 },
    byModelCost: { model: 1 },
    byAgentCost: { agent: 1 },
    breakdown: { input: 1000, cached: 0, output: 0, reasoning: 0 },
    byKind: [],
    byProject: [],
    sessionsThisWeek: 1,
    tokPerMin: 10,
    accounts: [],
  };
}

afterEach(() => {
  document.body.replaceChildren();
  mocks.invoke.mockReset();
  mocks.toPng.mockReset();
  mocks.toBlob.mockReset();
});

describe("share card click preview", () => {
  it("uses the export raster pipeline and opens the Tauri preview window", async () => {
    Object.defineProperty(document, "fonts", {
      configurable: true,
      value: { ready: Promise.resolve() },
    });
    let finishPng!: (value: string) => void;
    mocks.toPng.mockReturnValue(new Promise<string>((resolve) => (finishPng = resolve)));
    mocks.toBlob.mockResolvedValue(new Blob(["png"], { type: "image/png" }));
    mocks.invoke.mockResolvedValue(undefined);

    const container = document.createElement("section");
    document.body.appendChild(container);
    renderSharePanel(container, {
      analytics: analytics(),
      limits: [],
      locale: "en",
      style: "statement",
      range: "week",
      size: "auto",
      fuelGroup: "model",
      quotaNote: false,
      setStyle: vi.fn(),
      setRange: vi.fn(),
      setSize: vi.fn(),
      setFuelGroup: vi.fn(),
      setQuotaNote: vi.fn(),
    });

    const mat = container.querySelector<HTMLElement>(".sharep-preview")!;
    mat.click();
    expect(mat.classList.contains("busy")).toBe(true);
    finishPng("data:image/png;base64,cHJldmlldw==");

    await vi.waitFor(() => {
      expect(mocks.invoke).toHaveBeenCalledWith("open_share_preview", {
        dataUrl: "data:image/png;base64,cHJldmlldw==",
      });
    });
    expect(mocks.toPng).toHaveBeenCalledWith(expect.any(HTMLElement), {
      width: 1200,
      height: 675,
      pixelRatio: 1,
      cacheBust: true,
    });
    expect(mat.classList.contains("busy")).toBe(false);
  });
});
