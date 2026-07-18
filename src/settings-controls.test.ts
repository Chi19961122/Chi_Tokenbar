import { describe, expect, it } from "vitest";
import {
  activateSegment,
  readSegMultiValue,
  readSegmentValue,
  segmentHtml,
  segMultiHtml,
  toggleSegMulti,
} from "./settings-controls";

describe("settings segmented controls", () => {
  it("renders one active button with safe settings metadata", () => {
    document.body.innerHTML = segmentHtml("s-theme", "dark", [
      ["system", "Follow system"],
      ["dark", "Dark & dim"],
    ]);

    const segment = document.querySelector<HTMLElement>('.seg-set[data-sid="s-theme"]')!;
    expect(segment.querySelectorAll("button.on")).toHaveLength(1);
    expect(segment.querySelector<HTMLButtonElement>("button.on")!.dataset.val).toBe("dark");
    expect(segment.querySelector("button.on")!.textContent).toBe("Dark & dim");
  });

  it("reads the active value and falls back when no option is active", () => {
    document.body.innerHTML = `
      <div id="settings">
        <div class="seg seg-set" data-sid="s-theme">
          <button type="button" data-val="system">System</button>
          <button type="button" data-val="dark" class="on">Dark</button>
        </div>
      </div>`;
    const settings = document.querySelector("#settings")!;

    expect(readSegmentValue(settings, "s-theme", "system")).toBe("dark");
    document.querySelector("button.on")!.classList.remove("on");
    expect(readSegmentValue(settings, "s-theme", "system")).toBe("system");
  });

  it("moves the active state only for buttons inside a settings segment", () => {
    document.body.innerHTML = `
      <div class="seg seg-set" data-sid="s-providers">
        <button type="button" data-val="both" class="on">Both</button>
        <button type="button" data-val="claude"><span>Claude</span></button>
      </div>
      <button id="outside" type="button">Outside</button>`;
    const claudeLabel = document.querySelector('[data-val="claude"] span')!;

    expect(activateSegment(claudeLabel)).toBe(true);
    expect(document.querySelector('[data-val="both"]')!.classList.contains("on")).toBe(false);
    expect(document.querySelector('[data-val="claude"]')!.classList.contains("on")).toBe(true);
    expect(activateSegment(document.querySelector("#outside"))).toBe(false);
  });
});

describe("settings multi-select chips (T-916)", () => {
  it("renders every selected value as an on chip", () => {
    document.body.innerHTML = segMultiHtml("s-sources", ["claude", "grok"], [
      ["claude", "Claude"],
      ["codex", "Codex"],
      ["grok", "Grok"],
    ]);
    const row = document.querySelector<HTMLElement>('.seg-multi[data-sid="s-sources"]')!;
    expect(row.querySelectorAll("button.on")).toHaveLength(2);
    expect(readSegMultiValue(document, "s-sources")).toEqual(["claude", "grok"]);
  });

  it("reads on values in DOM order, and an empty selection is empty", () => {
    document.body.innerHTML = `
      <div id="settings">
        <div class="seg seg-multi" data-sid="s-sources">
          <button type="button" data-val="claude" class="on">Claude</button>
          <button type="button" data-val="codex">Codex</button>
          <button type="button" data-val="grok" class="on">Grok</button>
        </div>
      </div>`;
    const settings = document.querySelector("#settings")!;
    expect(readSegMultiValue(settings, "s-sources")).toEqual(["claude", "grok"]);
    settings.querySelectorAll("button.on").forEach((b) => b.classList.remove("on"));
    expect(readSegMultiValue(settings, "s-sources")).toEqual([]);
  });

  it("toggles chips independently (not radio) and ignores unrelated clicks", () => {
    document.body.innerHTML = `
      <div class="seg seg-multi" data-sid="s-sources">
        <button type="button" data-val="claude" class="on">Claude</button>
        <button type="button" data-val="codex"><span>Codex</span></button>
      </div>
      <button id="outside" type="button">Outside</button>`;
    const codexLabel = document.querySelector('[data-val="codex"] span')!;

    // Turning Codex on must leave Claude on (independent, unlike activateSegment).
    expect(toggleSegMulti(codexLabel)).toBe(true);
    expect(document.querySelector('[data-val="claude"]')!.classList.contains("on")).toBe(true);
    expect(document.querySelector('[data-val="codex"]')!.classList.contains("on")).toBe(true);
    // Clicking Claude again turns it off.
    expect(toggleSegMulti(document.querySelector('[data-val="claude"]'))).toBe(true);
    expect(document.querySelector('[data-val="claude"]')!.classList.contains("on")).toBe(false);
    // A multi-select click is not a single-select one and vice-versa.
    expect(activateSegment(codexLabel)).toBe(false);
    expect(toggleSegMulti(document.querySelector("#outside"))).toBe(false);
  });
});
