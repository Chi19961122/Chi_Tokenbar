// 階段 D 戰報 Share — DOM + IO layer for the "report" subtab.
//
// share.ts stays pure (data + card rendering); everything with a side effect
// lives here: the picker/range/toggle controls, the scaled live preview, and the
// PNG export / clipboard copy (Tauri writes to Downloads via save_share_png; the
// browser mock falls back to an <a download>). The panel never persists directly
// — it calls back into main.ts, which owns ui state + setSettings.

import type { Analytics, AnalyticsRange, Limit } from "./types";
import type { Locale } from "./i18n";
import { tl } from "./i18n";
import { isTauri } from "./datasource";
import { buildShareData, renderShareCard, type ShareStyle } from "./share";

const STYLES: [ShareStyle, string][] = [
  ["statement", "statement"],
  ["diagnostics", "diagnostics"],
  ["minimal", "minimal"],
  ["fuel", "fuel"],
  ["island_card", "island"],
  ["wa", "wa"],
];

export interface SharePanelOpts {
  analytics: Analytics;
  limits: Limit[];
  locale: Locale;
  style: ShareStyle;
  range: AnalyticsRange;
  fuelGroup: "model" | "agent";
  /** Effective quota-note flag (main.ts resolves the style default + override). */
  quotaNote: boolean;
  setStyle: (s: ShareStyle) => void;
  setRange: (r: AnalyticsRange) => void;
  setFuelGroup: (g: "model" | "agent") => void;
  setQuotaNote: (on: boolean) => void;
}

/** Two-digit-padded local YYYYMMDD for the export filename (fixed format, never
 *  toLocale*). */
function todayStamp(): string {
  const d = new Date();
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}${p(d.getMonth() + 1)}${p(d.getDate())}`;
}

export function renderSharePanel(container: HTMLElement, o: SharePanelOpts): void {
  const T = (key: Parameters<typeof tl>[1], vars?: Record<string, string | number>) =>
    tl(o.locale, key, vars);

  const data = buildShareData(o.analytics, {
    range: o.range,
    locale: o.locale,
    limits: o.limits,
    includeQuotaNote: o.quotaNote,
  });

  const styleBtns = STYLES.map(
    ([id, label]) =>
      `<button data-style="${id}" class="${o.style === id ? "on" : ""}">${label}</button>`,
  ).join("");

  const rangeBtns = (["today", "week", "month"] as AnalyticsRange[])
    .map(
      (r) =>
        `<button data-srange="${r}" class="${o.range === r ? "on" : ""}">${T(
          r === "today" ? "toggle.today" : r === "week" ? "toggle.week" : "toggle.month",
        )}</button>`,
    )
    .join("");

  const fuelSeg =
    o.style === "fuel"
      ? `<div class="seg" data-seg="fuelgroup">
           <button data-fuel="model" class="${o.fuelGroup === "model" ? "on" : ""}">${T("share.model")}</button>
           <button data-fuel="agent" class="${o.fuelGroup === "agent" ? "on" : ""}">${T("share.agent")}</button>
         </div>`
      : "";

  container.innerHTML = `
    <div class="sharep">
      <div class="sharep-preview"><div class="sharep-scale" id="sharep-scale"></div></div>
      <div class="sharep-styles">${styleBtns}</div>
      <div class="sharep-row">
        <div class="seg" data-seg="srange">${rangeBtns}</div>
        ${fuelSeg}
      </div>
      <label class="sharep-toggle">
        <span>${T("share.quotaNote")}</span>
        <input type="checkbox" id="sharep-quota" ${o.quotaNote ? "checked" : ""}/>
      </label>
      <div class="sharep-actions">
        <button id="sharep-save" class="sharep-btn primary">${T("share.exportPng")}</button>
        <button id="sharep-copy" class="sharep-btn">${T("share.copyImage")}</button>
      </div>
      <div class="sharep-status" id="sharep-status"></div>
    </div>`;

  // Live preview: render the real 1200×675 card, scale it to fit the box width.
  const scaleHost = container.querySelector<HTMLElement>("#sharep-scale")!;
  const preview = renderShareCard(o.style, data, o.locale, { fuelGroup: o.fuelGroup });
  scaleHost.appendChild(preview);
  const fit = () => {
    const box = scaleHost.parentElement as HTMLElement;
    const w = box.clientWidth || 340;
    const scale = w / 1200;
    scaleHost.style.transform = `scale(${scale})`;
    scaleHost.style.height = `${675 * scale}px`;
  };
  fit();
  // Width is 0 until layout settles on first paint inside a freshly-shown panel.
  requestAnimationFrame(fit);

  const status = container.querySelector<HTMLElement>("#sharep-status")!;
  const setStatus = (msg: string, err = false) => {
    status.textContent = msg;
    status.classList.toggle("err", err);
  };

  // ── control wiring ──────────────────────────────────────────────────
  container.querySelector(".sharep-styles")!.addEventListener("click", (e) => {
    const b = (e.target as HTMLElement).closest("[data-style]");
    if (b) o.setStyle(b.getAttribute("data-style") as ShareStyle);
  });
  container.querySelector("[data-seg='srange']")!.addEventListener("click", (e) => {
    const b = (e.target as HTMLElement).closest("[data-srange]");
    if (b) o.setRange(b.getAttribute("data-srange") as AnalyticsRange);
  });
  const fuelSegEl = container.querySelector("[data-seg='fuelgroup']");
  if (fuelSegEl)
    fuelSegEl.addEventListener("click", (e) => {
      const b = (e.target as HTMLElement).closest("[data-fuel]");
      if (b) o.setFuelGroup(b.getAttribute("data-fuel") as "model" | "agent");
    });
  container.querySelector<HTMLInputElement>("#sharep-quota")!.addEventListener("change", (e) => {
    o.setQuotaNote((e.target as HTMLInputElement).checked);
  });

  // ── export / copy ───────────────────────────────────────────────────

  /** Mount the full-size card offscreen, run html-to-image, return {dataUrl,
   *  blob}, then clean up. Fonts must be ready or the raster misses glyphs. */
  const rasterize = async (): Promise<{ dataUrl: string; blob: Blob | null }> => {
    await document.fonts.ready;
    const holder = document.createElement("div");
    holder.style.position = "fixed";
    holder.style.left = "-99999px";
    holder.style.top = "0";
    document.body.appendChild(holder);
    const card = renderShareCard(o.style, data, o.locale, { fuelGroup: o.fuelGroup });
    holder.appendChild(card);
    try {
      const { toPng, toBlob } = await import("html-to-image");
      const dataUrl = await toPng(card, {
        width: 1200,
        height: 675,
        pixelRatio: 1,
        cacheBust: true,
      });
      const blob = await toBlob(card, { width: 1200, height: 675, pixelRatio: 1, cacheBust: true });
      return { dataUrl, blob };
    } finally {
      holder.remove();
    }
  };

  container.querySelector("#sharep-save")!.addEventListener("click", async () => {
    const filename = `tokenbar-${o.range}-${todayStamp()}.png`;
    try {
      const { dataUrl } = await rasterize();
      if (isTauri()) {
        const bytes = Array.from(new Uint8Array(await (await fetch(dataUrl)).arrayBuffer()));
        const { invoke } = await import("@tauri-apps/api/core");
        const path = await invoke<string>("save_share_png", { bytes, filename });
        setStatus(T("share.saved", { path }));
      } else {
        const a = document.createElement("a");
        a.href = dataUrl;
        a.download = filename;
        a.click();
        setStatus(T("share.savedShort"));
      }
    } catch {
      setStatus(T("share.copyFailed"), true);
    }
  });

  container.querySelector("#sharep-copy")!.addEventListener("click", async () => {
    try {
      const { blob } = await rasterize();
      if (!blob) throw new Error("no blob");
      await navigator.clipboard.write([new ClipboardItem({ "image/png": blob })]);
      setStatus(T("share.savedShort"));
    } catch {
      setStatus(T("share.copyFailed"), true);
    }
  });
}
