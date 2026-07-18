# T-921 前端品牌收尾 (header ◎ + title + i18n + PNG 前綴 + style 標籤 + mock) — implementation brief

Implement in `C:\Coding\TokenBar\TokenBar-Src`. Do NOT commit. Do NOT run `tauri dev`/`build` or touch port 1420. Throwaway vite ≥5204 only for a sanity check (kill after). Runs AFTER T-920 (identity, committed) and T-922 (share cards, committed). You OWN: `index.html`, `src/styles.css`, `src/i18n.ts`, `src/share-panel.ts`, `src/mock.ts`. Do NOT touch share.ts/share.css (T-922, done) or any Rust/build (T-920, done).

## Why
Final front-end Atoll brand touches: the header ◎ ring-mark, window `<title>`, remaining user-visible "TokenBar" strings, the export filename prefix, the style-picker labels (now the six Atoll names), and the dev-mock project name.

## Changes

### 1. Header ◎ ring-mark (index.html + styles.css)
- `index.html`: inside `<header class="phead">`, add a brand element as the FIRST child (before `.ptabs`):
  ```html
  <div class="pbrand" title="Atoll">
    <svg viewBox="0 0 24 24" fill="none" aria-hidden="true"><circle cx="12" cy="12" r="9" stroke="currentColor" stroke-width="1.8"/><circle cx="12" cy="12" r="3.3" fill="currentColor"/></svg>
  </div>
  ```
  (◎ ring-mark only — NO "Atoll" wordmark: the narrow ~400px header cannot fit the wordmark without wrapping the tabs, confirmed by live test. The wordmark lives in the window title / tray / share cards.)
- `src/styles.css`: style `.pbrand` — a small flex-none brand slot at the header's left. Target: 17px ring in `var(--accent)`, sits inline-centered with the tab row, ~8px right margin, does NOT increase the header height. It must read as active-accent in BOTH themes (uses `var(--accent)`, which already flips light `#EC4899` / dark `#F472B6`). Sanity-check the header still lays out on one line at the real panel width (open a throwaway vite, expand the panel, confirm `限額 / 分析` tabs do NOT wrap).

### 2. Window title (index.html)
- `index.html` L6: `<title>TokenBar</title>` → `<title>Atoll</title>`.

### 3. Remaining i18n "TokenBar" (src/i18n.ts)
- `relogin.cantLaunch` en (~L161) `"TokenBar can't launch claude..."` → `"Atoll can't launch claude..."`; zh (~L366) `"TokenBar 無法啟動 claude..."` → `"Atoll 無法啟動 claude..."`.
- NOTE: `share.generatedBy` was ALREADY changed to Atoll by T-922 — do NOT touch it. grep i18n.ts for any other remaining "TokenBar" and flip user-visible ones (there should be only the two relogin strings left).

### 4. Export filename prefix (src/share-panel.ts)
- L264: `const filename = \`tokenbar-${o.range}${sizeTag}-${todayStamp()}.png\`;` → prefix `atoll-`.

### 5. Style-picker labels (src/share-panel.ts) — from T-922 handoff
- The `STYLES` array's display labels are hardcoded here (second tuple element), currently `statement/diagnostics/minimal/fuel/island/wa`. Rename the six DISPLAY labels to the Atoll names, keeping the ShareStyle KEY (first tuple element) unchanged (keys persist in settings):
  `island_card`→**Atoll**, `statement`→**Ledger**, `diagnostics`→**Terminal**, `minimal`→**Minimal**, `fuel`→**Sounding**, `wa`→**Seal**. If these labels support i18n, add en+zh (環礁儀表/結算單/終端/極簡/測深/環印); if they're plain strings, keep them short English labels consistent with the panel's existing style.
- **model/agent toggle for the Sounding (`fuel`) slot**: T-922 kept `fuelGroup` driving Sounding's depth-layer legend (model vs agent). Decide: keep the toggle visible for the Sounding style (it still meaningfully switches the layer grouping) — recommended keep. If the panel's toggle logic keys off the old `fuel` semantics/label, make sure it still shows for the Sounding slot. Note your decision.

### 6. Dev-mock project name (src/mock.ts)
- L174: `{ name: "tokenbar", tokens: ... }` → `{ name: "atoll", ... }`. (Preview/dev-only fake data; cosmetic.)

## Hard rules / gotchas
- Don't break header layout: the ◎ must not push the tabs to two lines at the real panel width. Verify live in a throwaway vite.
- Dual-theme: brand mark uses `var(--accent)` — verify it reads correctly in both light and dark (the app is dual-theme, T-901).
- Don't touch share.ts/share.css (T-922) — the card ◎ marks + signatures are already done there.

## Done criteria
- `npx tsc --noEmit -p tsconfig.json` clean; `npx vitest run` all green; `export PATH="$HOME/.cargo/bin:$PATH" && cargo test --manifest-path src-tauri/Cargo.toml` green.
- Throwaway vite sanity: header shows ◎ at left, tabs on one line (both themes); style picker shows the six Atoll labels; no console errors.
- Report: files touched, the style-label + toggle decision, confirmation the header doesn't wrap, and anything needing live-Tauri. Do not commit.
