import { describe, it, expect } from "vitest";
import { resolveThemeDark } from "./theme";

describe("resolveThemeDark", () => {
  it("forces dark for theme=dark regardless of the system", () => {
    expect(resolveThemeDark("dark", false)).toBe(true);
    expect(resolveThemeDark("dark", true)).toBe(true);
  });

  it("forces light for theme=light regardless of the system", () => {
    expect(resolveThemeDark("light", true)).toBe(false);
    expect(resolveThemeDark("light", false)).toBe(false);
  });

  it("follows the system for theme=system", () => {
    expect(resolveThemeDark("system", true)).toBe(true);
    expect(resolveThemeDark("system", false)).toBe(false);
  });

  it("treats any unknown value as system (defensive)", () => {
    expect(resolveThemeDark("", true)).toBe(true);
    expect(resolveThemeDark("garbage", false)).toBe(false);
  });
});
