import { describe, expect, it } from "vitest";
import { activateSegment, readSegmentValue, segmentHtml } from "./settings-controls";

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
