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
import { islandIntent, renderIsland } from "./island";
import { SCENARIOS } from "./mock";

const NOT_DRAGGED = false;
const DRAGGED = true;

function island(scenario: keyof typeof SCENARIOS | "none" = "safe"): HTMLElement {
  const root = document.createElement("div");
  renderIsland(root, scenario === "none" ? null : SCENARIOS[scenario], {
    mode: "both",
    tokPerMin: 1234,
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
