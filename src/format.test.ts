// Reset-time formatting tests (階段 B). The clock format is hand-built with a
// fixed locale — never a bare toLocale* — so these assert the exact strings a
// zh-TW machine and an en machine each get, including the day marker and 12/24h.
// Dates are built with the local Date constructor, so getHours()/getDay() match
// the assertion regardless of the runner's timezone.

import { describe, expect, it } from "vitest";
import { fmtResetClock, fmtResetRel } from "./format";

// Thu 15 Jan 2026, 10:00 local. Jan 18 2026 is a Sunday.
const now = Math.floor(new Date(2026, 0, 15, 10, 0, 0).getTime() / 1000);
const at = (mo: number, d: number, h: number, mi: number) =>
  Math.floor(new Date(2026, mo, d, h, mi, 0).getTime() / 1000);

describe("fmtResetRel — 倒數", () => {
  it("距重置的時長", () => {
    expect(fmtResetRel(now + 22 * 60, now)).toBe("22m");
    expect(fmtResetRel(now + 80 * 60, now)).toBe("1h 20m");
    expect(fmtResetRel(now + 3 * 86400, now)).toBe("3d 0h");
  });
  it("已過期夾到 0", () => {
    expect(fmtResetRel(now - 60, now)).toBe("0s");
  });
});

describe("fmtResetClock — 時刻(固定 locale)", () => {
  it("當日:en 12h / zh 24h", () => {
    const reset = at(0, 15, 14, 30);
    expect(fmtResetClock(reset, now, "en")).toBe("2:30 PM");
    expect(fmtResetClock(reset, now, "zh-TW")).toBe("14:30");
  });

  it("當日上午與午夜邊界", () => {
    expect(fmtResetClock(at(0, 15, 9, 5), now, "en")).toBe("9:05 AM");
    expect(fmtResetClock(at(0, 15, 0, 0), now, "en")).toBe("12:00 AM");
    expect(fmtResetClock(at(0, 15, 12, 0), now, "en")).toBe("12:00 PM");
    expect(fmtResetClock(at(0, 15, 0, 0), now, "zh-TW")).toBe("00:00");
  });

  it("明日補「明」/「Tmrw」", () => {
    const reset = at(0, 16, 9, 0);
    expect(fmtResetClock(reset, now, "en")).toBe("Tmrw 9:00 AM");
    expect(fmtResetClock(reset, now, "zh-TW")).toBe("明 09:00");
  });

  it("跨週補星期(固定英文 / 中文週幾)", () => {
    const reset = at(0, 18, 9, 0); // Sunday
    expect(fmtResetClock(reset, now, "en")).toBe("Sun 9:00 AM");
    expect(fmtResetClock(reset, now, "zh-TW")).toBe("週日 09:00");
  });

  it("以行事曆日界定明日,不是 24 小時(23:00 → 01:00 算隔日)", () => {
    const late = Math.floor(new Date(2026, 0, 15, 23, 0, 0).getTime() / 1000);
    const early = at(0, 16, 1, 0);
    expect(fmtResetClock(early, late, "en")).toBe("Tmrw 1:00 AM");
  });
});
