# T-914 「戰報」→「分享」改稱 + 移出 subtab + header icon 整頁模式 — implementation brief

Implement in this repo (Tauri 2 + vanilla TS in `src/`, Rust in `src-tauri/`). Do NOT commit. Do NOT kill or interfere with the running dev instance (port 1420, `tokenbar.exe`) — never run `tauri dev` yourself. For a browser sanity check only, a throwaway vite on a port ≥5200 is allowed (kill it after). `cargo test` may block on the target-dir lock — that's fine, wait.

## Why
Today the share/report feature lives as the 5th sub-tab **inside** the Analytics page ("戰報"). User decision: rename it to **「分享」** everywhere user-facing, pull it OUT of the analytics sub-tabs, and give it its own **full-page mode** opened by a **new icon in the header next to the gear** — architecturally identical to how Settings (T-902) already works. This ticket does the SHELL/ROUTING move only; the six card visuals are a separate ticket (T-915) — do NOT touch card markup/CSS internals (`src/share.ts` renderers, `src/share.css` `.shXX-*` blocks) here.

## Reference pattern to mirror (Settings full-page, T-902) — read these first
- `src/main.ts:703-713` `openSettingsPanel()` — `await renderSettings()` → `removeAttribute("hidden")` → `body.classList.add("settings-open")` → `fitWindow()`.
- `src/main.ts:719-730` `closeSettings()` — hide → remove class → re-render normal view (`renderCards/renderSubtabs/renderToggles/beginAnalytics/sizeAnalytics`) → `fitWindow()` (render-BEFORE-measure is load-bearing; keep it).
- `src/main.ts:841-844` gear click — toggles open/close by checking `hidden`.
- `src/main.ts:883-896` `onTab()` — tab clicks "escape" an open settings page.
- `src/styles.css:531-546` `body.settings-open { ... }` — hides `#cards/#subtabs/#toggles/#analytics/.rate`, and `#gear` gets `color: var(--accent)` when active.
- `index.html:24` `<button id="gear" title="Settings">⚙</button>`; `index.html:28` `<section id="settings" hidden>`.

## Current share/report wiring to unpick
- `src/analytics.ts:23` — `SubTab` union includes `"report"`.
- `src/main.ts:90-101` `renderSubtabs()` renders report as the 5th sub-tab button (i18n `subtab.report`).
- `src/main.ts:106-108` — when `ui.subtab === "report"`, `#toggles` is blanked.
- `src/main.ts:264-301` `paintReport()` → calls `renderSharePanel()` which currently mounts INTO `#analytics`.
- `src/main.ts:313, 358` — dispatch to `paintReport()` when `ui.subtab === "report"`.
- `src/main.ts:949-956` — sub-tab click handler sets `ui.subtab`.
- `src/share-panel.ts:67` `renderSharePanel()` — renders the whole panel (style/range/size/quota controls + preview + export buttons); currently targets `#analytics`.
- `src/i18n.ts:84` en `subtab.report: "Share"`; `:284` zh `subtab.report: "戰報"`.

## What to build

1. **New full-page container.** Add `<section id="share" hidden>` to `index.html` (sibling of `#settings`, same shell level). `renderSharePanel()` must mount into `#share` instead of `#analytics`. Keep the panel's internal control/preview/export markup and behavior exactly as-is (this ticket only changes WHERE it mounts and HOW it's opened).

2. **Header share icon.** Add `<button id="share-btn" title="…">` next to `#gear` in `index.html`. Use a small inline SVG share/card glyph consistent with the existing icon buttons' sizing/color (match `#gear` / refresh button styling — inherit their CSS class if there is one; do NOT invent a new visual language). Tooltip via i18n (`header.shareTitle`, both locales).

3. **open/close, mirroring settings:**
   - `openSharePanel()`: `renderSharePanel()` → `#share.removeAttribute("hidden")` → `body.classList.add("share-open")` → `fitWindow()`. **If settings is open, close it first** (mutual exclusion — never both open).
   - `closeShare()`: hide `#share` → remove `share-open` → re-render normal view (same sequence closeSettings uses) → `fitWindow()`.
   - `#share-btn` click toggles open/close by checking `#share`'s `hidden` (like gear does).
   - **Cross-exclusion:** opening Settings must close Share too; wire both directions so the two full-page modes can't stack.

4. **CSS.** Add `body.share-open { ... }` next to the settings block in `styles.css`: hide `#cards`, `#subtabs`, `#toggles`, `#analytics`, `.rate`, **and `#settings`**; give `#share-btn` `color: var(--accent)` when `body.share-open`. (Symmetrically ensure `body.settings-open #share { display:none }` if needed so a stale render can't bleed through.)

5. **Tab escape.** Extend `onTab()` so a tab click while Share is open closes Share and navigates (mirror the settings branch at `main.ts:883-896`).

6. **Remove report from sub-tabs.** Drop `"report"` from `SubTab` (`analytics.ts:23`), from `renderSubtabs()`, from the `paintReport`-dispatch in the render/fetch paths, and the `#toggles`-blanking special-case. `paintReport()` itself is no longer a sub-tab painter — its body (calling `renderSharePanel`) moves into `openSharePanel()`. Make sure removing the union member leaves no dangling `case "report"` / comparisons (tsc will catch; fix all).

7. **Persisted-state safety.** If `ui.subtab` was persisted as `"report"` from an older build, it must not crash or land on a dead tab — coerce any unknown/removed sub-tab to the default (overview) on load.

8. **i18n.** Rename the user-facing label to 分享: zh `header.shareTitle` = "分享", en = "Share". Remove `subtab.report` (or repurpose) now that it's not a sub-tab. Grep for every reference to `subtab.report` and any hard-coded "戰報" / "Share" tied to the old sub-tab and update. Keep all `share.*` card-content keys untouched (T-915 owns those).

## Gotchas
- Render-BEFORE-measure everywhere a window height is taken (the T-902/F-06 lesson) — never `fitWindow()` before the target content is in the DOM.
- The share panel opening should behave in both mock (browser) and Tauri; `fitWindow()` is a no-op/guarded off-Tauri already — follow how settings handles it.
- Do NOT change the export pipeline, `--share-*` tokens, card renderers, or `#share-preview` zoom window (T-906) — only the mount point + open/close routing.
- Keep the contextmenu (`src/contextmenu.ts`) settings entry working; if it makes sense to also offer a "分享" entry there, it's optional — don't break the existing one.

## Done criteria
- `npx tsc --noEmit -p tsconfig.json` clean; `npx vitest run` all green; `export PATH="$HOME/.cargo/bin:$PATH" && cargo test --manifest-path src-tauri/Cargo.toml` all green.
- Opening 分享 from the header icon shows the share panel full-page (other content hidden, icon in accent); closing (icon again, or clicking a tab) returns to the prior view with correct window height; Settings and 分享 never stack.
- Report: files touched, how open/close + mutual-exclusion is wired, i18n keys changed, the persisted-`report` coercion, and anything you could NOT verify without a live Tauri run (be explicit — orchestrator verifies live).
- Do not commit.
