import { describe, expect, it } from "vitest";
import { islandAuxNeedsAnalytics } from "./island-aux";

describe("islandAuxNeedsAnalytics (stage 1A)", () => {
  it("defaults missing aux to tok_per_min → needs scan", () => {
    expect(islandAuxNeedsAnalytics(undefined)).toBe(true);
    expect(islandAuxNeedsAnalytics(null)).toBe(true);
    expect(islandAuxNeedsAnalytics("")).toBe(false);
  });

  it("tok_per_min and cost_today need analytics", () => {
    expect(islandAuxNeedsAnalytics("tok_per_min")).toBe(true);
    expect(islandAuxNeedsAnalytics("cost_today")).toBe(true);
  });

  it("off skips the today scan (core 1A behaviour)", () => {
    expect(islandAuxNeedsAnalytics("off")).toBe(false);
  });

  it("unknown values do not scan", () => {
    expect(islandAuxNeedsAnalytics("nope")).toBe(false);
  });
});
