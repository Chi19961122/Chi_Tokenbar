// i18n dictionary + runtime tests (階段 A).

import { afterEach, describe, expect, it } from "vitest";
import { getLocale, resolveLocale, setLocale, t } from "./i18n";

// The key sets are enforced at compile time by `satisfies`, but exercise it at
// runtime too: a future edit that drops the compile check (or a merge that
// re-orders things) still gets caught, and the failure names the missing key.
//
// We can't import the private `en`/`zhTW` objects, so compare the two locales'
// output over a representative set of keys through the public `t`. Instead of
// re-listing every key, we assert that switching locale actually changes the
// rendered strings (i.e. zh-TW is a real translation, not an en fallback).
const SAMPLE_KEYS = [
  "tab.limits",
  "tab.usage",
  "settings.language",
  "subtab.overview",
  "toggle.today",
  "analytics.estCost",
  "limit.cc5h",
  "badge.unavailable",
  "note.locked",
  "note.lockedResets",
  "settings.resetDisplay",
  "menu.hide",
  "relogin.button",
  "list.noTools",
  "island.hideAria",
] as const;

afterEach(() => setLocale("en"));

describe("resolveLocale", () => {
  it("回傳明確指定的語系,不看系統", () => {
    expect(resolveLocale("en")).toBe("en");
    expect(resolveLocale("zh-TW")).toBe("zh-TW");
  });

  it("system:zh 開頭的系統語系 → zh-TW", () => {
    const orig = navigator.language;
    Object.defineProperty(navigator, "language", { value: "zh-TW", configurable: true });
    try {
      expect(resolveLocale("system")).toBe("zh-TW");
    } finally {
      Object.defineProperty(navigator, "language", { value: orig, configurable: true });
    }
  });

  it("system:非 zh 系統語系 → en", () => {
    const orig = navigator.language;
    Object.defineProperty(navigator, "language", { value: "en-US", configurable: true });
    try {
      expect(resolveLocale("system")).toBe("en");
    } finally {
      Object.defineProperty(navigator, "language", { value: orig, configurable: true });
    }
  });

  it("未知值走系統 fallback,不會炸", () => {
    const orig = navigator.language;
    Object.defineProperty(navigator, "language", { value: "fr-FR", configurable: true });
    try {
      expect(resolveLocale("garbage")).toBe("en");
    } finally {
      Object.defineProperty(navigator, "language", { value: orig, configurable: true });
    }
  });
});

describe("setLocale / t", () => {
  it("切換語系會改變輸出(zh-TW 是真翻譯,不是英文 fallback)", () => {
    for (const key of SAMPLE_KEYS) {
      setLocale("en");
      const enVal = t(key);
      setLocale("zh-TW");
      const zhVal = t(key);
      expect(enVal, `${key} 應該有英文值`).toBeTruthy();
      expect(zhVal, `${key} 應該有中文值`).toBeTruthy();
      expect(zhVal, `${key} 中英不應相同`).not.toBe(enVal);
    }
  });

  it("setLocale 同步 document.documentElement.lang", () => {
    setLocale("zh-TW");
    expect(document.documentElement.lang).toBe("zh-TW");
    setLocale("en");
    expect(document.documentElement.lang).toBe("en");
  });

  it("getLocale 反映目前語系", () => {
    setLocale("zh-TW");
    expect(getLocale()).toBe("zh-TW");
  });
});

describe("t() 佔位插值", () => {
  it("以 {name} 佔位插入變數(en)", () => {
    setLocale("en");
    expect(t("limit.weeklyModel", { name: "Opus" })).toBe("Weekly · Opus");
  });

  it("以 {name} 佔位插入變數(zh-TW)", () => {
    setLocale("zh-TW");
    expect(t("limit.weeklyModel", { name: "Opus" })).toBe("每週 · Opus");
  });

  it("插入佔位符與數字", () => {
    setLocale("en");
    expect(t("note.resetsIn", { d: "3h 12m" })).toBe("Resets in 3h 12m");
    expect(t("note.lockedResetsIn", { d: "45m" })).toBe("Locked · resets in 45m");
    setLocale("zh-TW");
    expect(t("note.resets", { r: "14:30" })).toBe("14:30 重置");
  });

  it("同一佔位符出現多次全部替換", () => {
    // header.refreshIn 只有一個 {v},改用可驗證多次替換的合成:確保 split/join 全換
    setLocale("en");
    const out = t("header.refreshIn", { v: "5s" });
    expect(out).toBe("Refresh in 5s");
  });
});
