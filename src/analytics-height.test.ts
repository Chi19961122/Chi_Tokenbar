import { describe, expect, it } from "vitest";
import { analyticsHeight } from "./analytics-height";

describe("analyticsHeight", () => {
  it("uses the available screen budget between the fixed bounds", () => {
    expect(analyticsHeight(900, 420)).toBe(440);
  });

  it("never shrinks below the original 300px height", () => {
    expect(analyticsHeight(720, 500)).toBe(300);
  });

  it("caps a large-screen budget at 640px", () => {
    expect(analyticsHeight(1440, 420)).toBe(640);
  });

  it("keeps 300px when the available screen height is tiny or unavailable", () => {
    expect(analyticsHeight(699, 0)).toBe(300);
    expect(analyticsHeight(Number.NaN, 0)).toBe(300);
  });
});
