# T-903 settings controls redesign — implementation brief

Implement ticket T-903 in this repo (Tauri 2 + vanilla TS in src/). Read docs/ROUND-v070.md (T-903 + 全局決策) first. Do NOT commit — leave changes in the working tree. Do NOT touch src-tauri/, src/share.css card scopes, or the island styles/`--island-*` block in src/styles.css.

## Why
User feedback F-02d: the settings dropdowns (native `<select>`) need a redesign. Also fix a flagged defect: `styles.css` `.srow select option { background:#16181c }` hardcodes a dark popup, unreadable with the light theme's near-black `--text`. The settings page is already full-page (T-902) and dual-theme (T-901, `.dark` on `<html>`, tokens in styles.css `:root` / `:root.dark`).

## Work
1. **Fixed-option selects → segmented buttons.** Every `<select>` in `renderSettings()` (src/main.ts) whose options are a FIXED set of 2-3 becomes a `.seg`-style segmented control (same visual vocabulary as the analytics toggles — reuse/extend the existing `.seg` CSS, don't invent a new look): language (跟隨系統/中文/English), providers (3), expand_default (2), island_aux (3), reset_display (2), theme (3), claude token refresh (2), codex source (3). Markup suggestion: `<div class="seg seg-set" data-sid="s-providers">` with `<button type="button" data-val="claude" class="on">…</button>`; clicking moves `.on`.
2. **State plumbing.** `readSettingsForm()` currently reads `<select>` values by id. Adapt it to read seg state (helper `segVal(id): string` returning the `.on` button's `data-val`, same fallback defaults as today). Saving currently hangs off `$("settings").addEventListener("change", …)` — seg buttons emit clicks, not change events. Extract the handler body into one `commitSettings()` and invoke it from BOTH the existing change listener (checkboxes, number inputs, remaining selects) and a new click listener on #settings that (a) ignores clicks not on a seg button, (b) moves `.on` within that seg, then commits. Locale/theme special-casing must survive: locale change → re-render everything incl. the settings page (segs rebuild with new labels); theme change → applyTheme. Beware: `renderSettings()` is async and re-reads saved settings — a locale flip triggered from a seg must not lose the just-committed value.
3. **Dynamic pin dropdowns stay `<select>`** (Island · Claude / Island · Codex, dynamic model lists) but restyle: `appearance:none`, token-based colors, drawn chevron (CSS), height consistent with segs, AND fix the option-popup defect: remove the hardcoded `#16181c`; rely on T-901's `color-scheme` (light/dark) for native popup chrome plus token colors — must be readable in BOTH themes.
4. **Checkbox + number inputs**: tokenize remaining hardcoded chrome (e.g. knob `#dfe3e8`) so both themes pass WCAG AA; keep interactions.
5. **Layout**: rows stay `.srow` label-left control-right; segs must not overflow panel width with zh OR en labels (allow the seg to wrap to its own line under the label if needed — pick the cleaner outcome and note it).
6. i18n: reuse existing keys in src/i18n.ts; any NEW visible string needs both en and zhTW (the `satisfies` check enforces parity).

## Done criteria
- `npx tsc --noEmit -p tsconfig.json` clean.
- `npx vitest run` all green (add a unit test if you extract a pure helper worth testing).
- No Rust changes; do not run cargo.
- Do not start any dev server.
- Print a final summary: files touched, judgment calls, how each done-criterion was verified.
