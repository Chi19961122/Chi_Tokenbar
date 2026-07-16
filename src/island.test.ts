// Island event-routing tests (項目 A: the hide-to-tray button).
//
// These drive the *rendered* island through the real function `main.ts`
// dispatches on, rather than asserting that a predicate returns a value.
// The island has to serve three gestures on one 340×52 pill — click to expand,
// drag to move, click the button to hide — and the failure modes are all
// collisions between them. So every test starts from renderIsland's real
// output and asks the real question: "the user pressed *this* element; what
// happens?"
//
// Elements are located the way a user finds them (the button, the percentage
// text), never by the `data-` attribute islandIntent itself matches on — that
// would just assert the selector equals itself and would survive any mutation
// of the routing rules.

import { describe, expect, it } from "vitest";
import { islandIntent, islandText, pickIslandLimit, renderIsland, windowShort } from "./island";
import type { Limit } from "./types";
import { SCENARIOS } from "./mock";

const NOT_DRAGGED = false;
const DRAGGED = true;

function island(scenario: keyof typeof SCENARIOS | "none" = "safe"): HTMLElement {
  const root = document.createElement("div");
  renderIsland(root, scenario === "none" ? null : SCENARIOS[scenario], {
    mode: "both",
    pinClaude: "auto",
    pinCodex: "auto",
    resetDisplay: "relative",
    aux: "tok_per_min",
    tokPerMin: 1234,
    costToday: 1.5,
    now: Math.floor(Date.now() / 1000),
    locale: "en",
  });
  return root;
}

/** The hide affordance as a user would find it: the island's only button. */
function hideButton(root: HTMLElement): HTMLElement {
  const buttons = root.querySelectorAll("button");
  expect(buttons.length, "島嶼應該只有一個按鈕(隱藏鈕)").toBe(1);
  return buttons[0] as HTMLElement;
}

describe("島嶼的隱藏鈕", () => {
  it("在收合狀態下就存在 —— 擋到畫面的是島嶼本身,不該逼使用者先展開", () => {
    expect(island().querySelectorAll("button")).toHaveLength(1);
  });

  it("沒有資料時也在 —— 空島嶼一樣擋著畫面", () => {
    expect(island("none").querySelectorAll("button")).toHaveLength(1);
  });

  it("按下去是隱藏,不是展開", () => {
    expect(islandIntent(hideButton(island()), NOT_DRAGGED)).toBe("hide");
  });

  it("按到鈕裡面的圖形也算按到鈕(不是只有按到邊框才算)", () => {
    const inner = hideButton(island()).firstElementChild;
    expect(inner, "隱藏鈕應該有圖形子節點").not.toBeNull();
    expect(islandIntent(inner, NOT_DRAGGED)).toBe("hide");
  });
});

describe("島嶼本體", () => {
  it("點島嶼(非按鈕處)仍然展開面板", () => {
    const pct = island().querySelector(".pct");
    expect(pct, "島嶼應該有 % 文字").not.toBeNull();
    expect(islandIntent(pct, NOT_DRAGGED)).toBe("expand");
  });

  it("點島嶼根節點本身也展開", () => {
    expect(islandIntent(island(), NOT_DRAGGED)).toBe("expand");
  });
});

describe("拖曳優先於一切 —— 移動視窗不該有副作用", () => {
  it("拖完島嶼放開不會展開面板", () => {
    expect(islandIntent(island().querySelector(".pct"), DRAGGED)).toBe("none");
  });

  // 島嶼很小,拖曳很容易在隱藏鈕上放開。若順序寫反(先判斷按鈕再判斷拖曳),
  // 使用者只是想把島嶼挪開,結果視窗整個消失,而唯一的救援途徑是系統匣選單。
  it("拖曳結束在隱藏鈕上,不可以把視窗隱藏掉", () => {
    expect(islandIntent(hideButton(island()), DRAGGED)).toBe("none");
  });
});

// ── 階段 B 顯示矩陣 ────────────────────────────────────────────────────

/** Build a limit; overrides win. Deterministic epoch (no `now()`). */
function lim(p: Partial<Limit> & { id: string; provider: Limit["provider"] }): Limit {
  return {
    label: p.id,
    util: 0,
    resets_at: 0,
    window_secs: 5 * 3600,
    status: "normal",
    absolute: null,
    pace: null,
    runway_secs: null,
    ...p,
  };
}

describe("pickIslandLimit — 釘選矩陣", () => {
  const limits: Limit[] = [
    lim({ id: "cc.5h", provider: "anthropic", util: 55 }),
    lim({ id: "cc.week", provider: "anthropic", util: 47 }),
    lim({ id: "cc.opus", provider: "anthropic", util: 18 }),
    lim({ id: "codex.5h", provider: "codex", util: 88, status: "near" }),
    lim({ id: "codex.week", provider: "codex", util: 61 }),
  ];

  it("auto 回傳最危險的限額(locked>near>util)", () => {
    // codex.5h is near, so it outranks the higher-util-but-normal week window.
    expect(pickIslandLimit(limits, "codex", "auto")?.id).toBe("codex.5h");
    // Claude has no near/locked → highest util wins.
    expect(pickIslandLimit(limits, "anthropic", "auto")?.id).toBe("cc.5h");
  });

  it("釘 5h / week 回傳該視窗", () => {
    expect(pickIslandLimit(limits, "anthropic", "5h")?.id).toBe("cc.5h");
    expect(pickIslandLimit(limits, "anthropic", "week")?.id).toBe("cc.week");
    expect(pickIslandLimit(limits, "codex", "5h")?.id).toBe("codex.5h");
  });

  it("釘 model:<id> 回傳該限額", () => {
    expect(pickIslandLimit(limits, "anthropic", "model:cc.opus")?.id).toBe("cc.opus");
  });

  it("釘了但無資料 → null(不靜默退回 auto)", () => {
    expect(pickIslandLimit(limits, "anthropic", "model:cc.nope")).toBeNull();
    const noFiveH = limits.filter((l) => l.id !== "codex.5h");
    expect(pickIslandLimit(noFiveH, "codex", "5h")).toBeNull();
    expect(pickIslandLimit([], "anthropic", "week")).toBeNull();
  });

  it("未知釘值 → null,不猜", () => {
    expect(pickIslandLimit(limits, "anthropic", "garbage")).toBeNull();
  });
});

describe("windowShort — 固定英文短標", () => {
  it("視窗與模型都給短英文", () => {
    expect(windowShort(lim({ id: "cc.5h", provider: "anthropic" }))).toBe("5h");
    expect(windowShort(lim({ id: "codex.week", provider: "codex" }))).toBe("wk");
    expect(windowShort(lim({ id: "cc.opus", provider: "anthropic" }))).toBe("Opus");
    expect(windowShort(lim({ id: "cc.w.fable", provider: "anthropic" }))).toBe("Fable");
  });
});

describe("islandText — normal/near/locked × relative/clock", () => {
  // Jan 15 2026 10:00 local (Thu). Deterministic, TZ-independent (local ctor).
  const now = Math.floor(new Date(2026, 0, 15, 10, 0, 0).getTime() / 1000);
  const at = (h: number, mi: number) =>
    Math.floor(new Date(2026, 0, 15, h, mi, 0).getTime() / 1000);

  it("normal 只顯示 {left}%,無短標", () => {
    const l = lim({ id: "cc.5h", provider: "anthropic", util: 30, status: "normal" });
    expect(islandText(l, "relative", now, "en")).toBe("70%");
  });

  it("near relative: {short} {left}% · {倒數}", () => {
    const l = lim({ id: "codex.5h", provider: "codex", util: 88, status: "near", resets_at: at(10, 22) });
    expect(islandText(l, "relative", now, "en")).toBe("5h 12% · 22m");
  });

  it("near clock: {short} {left}% · {時刻}(依 locale)", () => {
    const l = lim({ id: "codex.5h", provider: "codex", util: 88, status: "near", resets_at: at(14, 30) });
    expect(islandText(l, "clock", now, "en")).toBe("5h 12% · 2:30 PM");
    expect(islandText(l, "clock", now, "zh-TW")).toBe("5h 12% · 14:30");
  });

  it("locked: {short} 0% · {reset}(短標指出鎖住的是哪個視窗)", () => {
    const l = lim({ id: "codex.5h", provider: "codex", util: 100, status: "locked", resets_at: at(11, 20) });
    expect(islandText(l, "relative", now, "en")).toBe("5h 0% · 1h 20m");
    expect(islandText(l, "clock", now, "en")).toBe("5h 0% · 11:20 AM");
  });

  it("estimate/stale 帶 est. 標;source_failed 顯示 —", () => {
    expect(islandText(lim({ id: "cc.week", provider: "anthropic", util: 40, status: "stale" }), "relative", now, "en")).toBe("60% est.");
    expect(islandText(lim({ id: "cc.5h", provider: "anthropic", util: 0, status: "source_failed" }), "relative", now, "en")).toBe("—");
  });
});
