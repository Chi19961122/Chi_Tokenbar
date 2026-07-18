import { describe, expect, it } from "vitest";
import { ALL_SOURCES, hasSource, islandMode, sourcesKey } from "./sources";

describe("sources helpers (T-916)", () => {
  it("hasSource tolerates an undefined list", () => {
    expect(hasSource(undefined, "grok")).toBe(false);
    expect(hasSource(["grok"], "grok")).toBe(true);
    expect(hasSource(["claude"], "grok")).toBe(false);
  });

  it("islandMode derives from the two quota providers only", () => {
    expect(islandMode(["claude", "codex", "grok"])).toBe("both");
    expect(islandMode(["claude", "opencode"])).toBe("claude");
    expect(islandMode(["codex"])).toBe("codex");
    // Usage-only sources produce no island quota → empty state.
    expect(islandMode(["opencode", "gemini", "grok"])).toBe("none");
    expect(islandMode([])).toBe("none");
  });

  it("sourcesKey is order-independent and defaults to all sources", () => {
    expect(sourcesKey(["codex", "claude"])).toBe(sourcesKey(["claude", "codex"]));
    expect(sourcesKey(undefined)).toBe([...ALL_SOURCES].sort().join(","));
    expect(sourcesKey([])).toBe("");
  });
});
