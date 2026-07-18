import { afterEach, describe, expect, it, vi } from "vitest";
import { setLocale } from "./i18n";
import { bootSharePreview, isSharePreviewHash } from "./share-preview";

afterEach(() => {
  document.body.className = "";
  document.body.replaceChildren();
  setLocale("en");
});

describe("share preview route", () => {
  it("matches only the dedicated hash", () => {
    expect(isSharePreviewHash("#share-preview")).toBe(true);
    expect(isSharePreviewHash("#share-preview-extra")).toBe(false);
    expect(isSharePreviewHash("")).toBe(false);
  });

  it("renders the exported PNG and localized close hint", async () => {
    const closeWindow = vi.fn(async () => undefined);
    const cleanup = await bootSharePreview({
      getPreview: async () => ({
        dataUrl: "data:image/png;base64,cHJldmlldw==",
        locale: "zh-TW",
      }),
      closeWindow,
    });

    expect(document.body.classList.contains("share-preview-body")).toBe(true);
    const image = document.querySelector<HTMLImageElement>(".share-preview-image")!;
    expect(image.src).toBe("data:image/png;base64,cHJldmlldw==");
    expect(image.alt).toBe("開啟大圖預覽");
    expect(document.querySelector(".share-preview-hint")?.textContent).toBe(
      "Esc / 點擊關閉",
    );

    document.body.click();
    await Promise.resolve();
    expect(closeWindow).toHaveBeenCalledTimes(1);
    cleanup();
  });

  it("closes on Escape but ignores other keys", async () => {
    const closeWindow = vi.fn(async () => undefined);
    const cleanup = await bootSharePreview({
      getPreview: async () => ({ dataUrl: "data:image/png;base64,eA==", locale: "en" }),
      closeWindow,
    });

    window.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter" }));
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    await Promise.resolve();
    expect(closeWindow).toHaveBeenCalledTimes(1);
    cleanup();
  });
});
