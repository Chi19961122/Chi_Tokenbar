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
    const unlisten = vi.fn();
    const cleanup = await bootSharePreview({
      getPreview: async () => ({
        dataUrl: "data:image/png;base64,cHJldmlldw==",
        locale: "zh-TW",
      }),
      listenForUpdates: async () => unlisten,
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
    expect(unlisten).toHaveBeenCalledTimes(1);
  });

  it("keeps generating when empty and re-pulls after subscribing to close the lost-event race", async () => {
    let payload = { dataUrl: null as string | null, locale: "zh-TW" };
    const getPreview = vi.fn(async () => payload);
    const unlisten = vi.fn();
    const listenForUpdates = vi.fn(async () => {
      // The backend update lands after the first pull but before the listener is
      // active. No event callback runs; the post-subscribe pull must recover it.
      payload = { dataUrl: "data:image/png;base64,bGF0ZXN0", locale: "zh-TW" };
      return unlisten;
    });

    const cleanup = await bootSharePreview({
      getPreview,
      listenForUpdates,
      closeWindow: vi.fn(async () => undefined),
    });

    expect(getPreview).toHaveBeenCalledTimes(2);
    expect(listenForUpdates).toHaveBeenCalledTimes(1);
    expect(document.querySelector<HTMLImageElement>(".share-preview-image")?.src).toBe(
      "data:image/png;base64,bGF0ZXN0",
    );
    cleanup();
  });

  it("shows generating copy for an empty payload and pulls state on update events", async () => {
    const payloads = [
      { dataUrl: null, locale: "en" },
      { dataUrl: null, locale: "en" },
      { dataUrl: "data:image/png;base64,dXBkYXRlZA==", locale: "en" },
    ];
    const getPreview = vi.fn(async () => payloads.shift()!);
    let onUpdate!: () => void;
    const cleanup = await bootSharePreview({
      getPreview,
      listenForUpdates: async (listener) => {
        onUpdate = listener;
        return () => undefined;
      },
      closeWindow: vi.fn(async () => undefined),
    });

    expect(document.querySelector(".share-preview-image")).toBeNull();
    expect(document.querySelector(".share-preview-hint")?.textContent).toBe("Rendering\u2026");

    onUpdate();
    await vi.waitFor(() => {
      expect(document.querySelector<HTMLImageElement>(".share-preview-image")?.src).toBe(
        "data:image/png;base64,dXBkYXRlZA==",
      );
    });
    expect(getPreview).toHaveBeenCalledTimes(3);
    cleanup();
  });


  it("ignores an older empty pull that resolves after a newer event pull", async () => {
    let resolveOlder!: (payload: { dataUrl: null; locale: string }) => void;
    let resolveNewer!: (payload: { dataUrl: string; locale: string }) => void;
    const olderPull = new Promise<{ dataUrl: null; locale: string }>((resolve) => {
      resolveOlder = resolve;
    });
    const newerPull = new Promise<{ dataUrl: string; locale: string }>((resolve) => {
      resolveNewer = resolve;
    });
    const getPreview = vi
      .fn()
      .mockResolvedValueOnce({ dataUrl: null, locale: "en" })
      .mockReturnValueOnce(olderPull)
      .mockReturnValueOnce(newerPull);
    let onUpdate!: () => void;
    const booting = bootSharePreview({
      getPreview,
      listenForUpdates: async (listener) => {
        onUpdate = listener;
        return () => undefined;
      },
      closeWindow: vi.fn(async () => undefined),
    });

    await vi.waitFor(() => expect(getPreview).toHaveBeenCalledTimes(2));
    onUpdate();
    await vi.waitFor(() => expect(getPreview).toHaveBeenCalledTimes(3));

    resolveNewer({ dataUrl: "data:image/png;base64,bmV3ZXI=", locale: "en" });
    await vi.waitFor(() => {
      expect(document.querySelector<HTMLImageElement>(".share-preview-image")?.src).toBe(
        "data:image/png;base64,bmV3ZXI=",
      );
    });
    resolveOlder({ dataUrl: null, locale: "en" });
    const cleanup = await booting;

    expect(document.querySelector<HTMLImageElement>(".share-preview-image")?.src).toBe(
      "data:image/png;base64,bmV3ZXI=",
    );
    cleanup();
  });
  it("closes on Escape but ignores other keys", async () => {
    const closeWindow = vi.fn(async () => undefined);
    const cleanup = await bootSharePreview({
      getPreview: async () => ({ dataUrl: "data:image/png;base64,eA==", locale: "en" }),
      listenForUpdates: async () => () => undefined,
      closeWindow,
    });

    window.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter" }));
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    await Promise.resolve();
    expect(closeWindow).toHaveBeenCalledTimes(1);
    cleanup();
  });
});
