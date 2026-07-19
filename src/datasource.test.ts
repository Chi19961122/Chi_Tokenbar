import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn();
const mockAnalytics = vi.fn((range: string) => ({ range, mock: true }));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));

vi.mock("./mock", async () => {
  const actual = await vi.importActual<typeof import("./mock")>("./mock");
  return {
    ...actual,
    mockAnalytics: (range: "today" | "week" | "month") => mockAnalytics(range),
  };
});

describe("getAnalytics error contract", () => {
  beforeEach(() => {
    invoke.mockReset();
    mockAnalytics.mockClear();
    // Force Tauri mode
    (window as any).__TAURI_INTERNALS__ = {};
  });

  it("browser mode (no Tauri) may return mock analytics", async () => {
    delete (window as any).__TAURI_INTERNALS__;
    delete (window as any).__TAURI__;
    // Re-import after clearing Tauri markers is awkward; call isTauri path via
    // dynamic import after env change.
    vi.resetModules();
    const { getAnalytics, isTauri } = await import("./datasource");
    expect(isTauri()).toBe(false);
    const a = await getAnalytics("week");
    expect(a).not.toBeNull();
    expect(a!.range).toBe("week");
    expect(mockAnalytics).toHaveBeenCalledWith("week");
  });

  it("Tauri superseded returns null and never mocks", async () => {
    vi.resetModules();
    (window as any).__TAURI_INTERNALS__ = {};
    invoke.mockRejectedValue("analytics_error:superseded:analytics request superseded");
    const { getAnalytics } = await import("./datasource");
    const a = await getAnalytics("week");
    expect(a).toBeNull();
    expect(mockAnalytics).not.toHaveBeenCalled();
  });

  it("Tauri cancelled returns null and never mocks", async () => {
    vi.resetModules();
    (window as any).__TAURI_INTERNALS__ = {};
    invoke.mockRejectedValue("analytics_error:cancelled:analytics request cancelled");
    const { getAnalytics } = await import("./datasource");
    const a = await getAnalytics("month");
    expect(a).toBeNull();
    expect(mockAnalytics).not.toHaveBeenCalled();
  });

  it("Tauri scan_failed throws and never mocks", async () => {
    vi.resetModules();
    (window as any).__TAURI_INTERNALS__ = {};
    invoke.mockRejectedValue("analytics_error:scan_failed:disk full");
    const { getAnalytics } = await import("./datasource");
    await expect(getAnalytics("today")).rejects.toThrow(/scan_failed|disk full/i);
    expect(mockAnalytics).not.toHaveBeenCalled();
  });

  it("Tauri unknown rejection throws without mock", async () => {
    vi.resetModules();
    (window as any).__TAURI_INTERNALS__ = {};
    invoke.mockRejectedValue("ipc exploded");
    const { getAnalytics } = await import("./datasource");
    await expect(getAnalytics("today")).rejects.toBeTruthy();
    expect(mockAnalytics).not.toHaveBeenCalled();
  });
});
