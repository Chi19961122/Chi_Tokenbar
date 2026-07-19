import { afterEach, describe, expect, it, vi } from "vitest";
import type { Analytics } from "./types";

const mocks = vi.hoisted(() => ({
  invoke: vi.fn(),
  getFontEmbedCSS: vi.fn(),
  toPng: vi.fn(),
  toBlob: vi.fn(),
}));

vi.mock("./datasource", () => ({ isTauri: () => true }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("html-to-image", () => ({
  getFontEmbedCSS: mocks.getFontEmbedCSS,
  toPng: mocks.toPng,
  toBlob: mocks.toBlob,
}));

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

function mountPanel(): HTMLElement {
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
  return container;
}

afterEach(() => {
  document.body.replaceChildren();
  mocks.invoke.mockReset();
  mocks.getFontEmbedCSS.mockReset();
  mocks.toPng.mockReset();
  mocks.toBlob.mockReset();
  vi.unstubAllGlobals();
});

describe("share card click preview", () => {
  it("opens immediately, renders in parallel, then publishes after open completes", async () => {
    Object.defineProperty(document, "fonts", {
      configurable: true,
      value: { ready: Promise.resolve() },
    });
    let finishOpen!: () => void;
    let finishPng!: (value: string) => void;
    // finishOpen closes the open_share_preview promise with a session id
    mocks.invoke.mockImplementation((command: string) => {
      if (command === "open_share_preview") {
        return new Promise<number>((resolve) => {
          finishOpen = () => resolve(42);
        });
      }
      return Promise.resolve(undefined);
    });
    mocks.getFontEmbedCSS.mockResolvedValue("@font-face { font-family: Geist; }");
    mocks.toPng.mockReturnValue(new Promise<string>((resolve) => (finishPng = resolve)));
    mocks.toBlob.mockResolvedValue(new Blob(["png"], { type: "image/png" }));

    const container = mountPanel();

    const mat = container.querySelector<HTMLElement>(".sharep-preview")!;
    mat.click();
    expect(mat.classList.contains("busy")).toBe(true);

    await vi.waitFor(() => {
      expect(mocks.invoke).toHaveBeenCalledWith("open_share_preview");
      expect(mocks.toPng).toHaveBeenCalledTimes(1);
    });
    finishPng("data:image/png;base64,cHJldmlldw==");
    await Promise.resolve();
    expect(mocks.invoke).not.toHaveBeenCalledWith("update_share_preview", expect.anything());

    finishOpen();

    await vi.waitFor(() => {
      expect(mocks.invoke).toHaveBeenCalledWith("update_share_preview", {
        dataUrl: "data:image/png;base64,cHJldmlldw==",
        session: 42,
      });
    });
    expect(mocks.toPng).toHaveBeenCalledWith(expect.any(HTMLElement), {
      width: 1200,
      height: 675,
      pixelRatio: 1,
      cacheBust: true,
      fontEmbedCSS: "@font-face { font-family: Geist; }",
    });
    const fontProbe = mocks.getFontEmbedCSS.mock.calls[0][0] as HTMLElement;
    for (const family of ["Geist", "Geist Mono", "Playfair Display", "Noto Sans TC"]) {
      expect(fontProbe.style.fontFamily).toContain(family);
    }
    expect(mocks.toBlob).not.toHaveBeenCalled();
    expect(mat.classList.contains("busy")).toBe(false);

    const clipboardWrite = vi.fn(async () => undefined);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { write: clipboardWrite },
    });
    vi.stubGlobal(
      "ClipboardItem",
      class ClipboardItem {
        constructor(_items: Record<string, Blob>) {}
      },
    );
    container.querySelector<HTMLElement>("#sharep-copy")!.click();
    await vi.waitFor(() => expect(clipboardWrite).toHaveBeenCalledTimes(1));
    expect(mocks.getFontEmbedCSS).toHaveBeenCalledTimes(1);
    expect(mocks.toBlob).toHaveBeenCalledWith(expect.any(HTMLElement), {
      width: 1200,
      height: 675,
      pixelRatio: 1,
      cacheBust: true,
      fontEmbedCSS: "@font-face { font-family: Geist; }",
    });
  });

  it("closes the generating window when rasterization fails", async () => {
    Object.defineProperty(document, "fonts", {
      configurable: true,
      value: { ready: Promise.resolve() },
    });
    mocks.invoke.mockResolvedValue(undefined);
    mocks.getFontEmbedCSS.mockResolvedValue("@font-face { font-family: Geist; }");
    mocks.toPng.mockRejectedValue(new Error("raster failed"));

    const container = mountPanel();
    container.querySelector<HTMLElement>(".sharep-preview")!.click();

    await vi.waitFor(() => {
      expect(mocks.invoke).toHaveBeenCalledWith("close_share_preview");
    });
    expect(container.querySelector(".sharep-status")?.textContent).toBe("Preview failed");
  });
});
