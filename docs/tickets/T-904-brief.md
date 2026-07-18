# T-904 analytics height decoupling — implementation brief

Implement ticket T-904 in this repo (Tauri 2 + vanilla TS in src/). Read docs/ROUND-v070.md (T-904 + 全局決策) first. Do NOT commit — leave changes in the working tree. Do NOT touch src-tauri/, src/share.css, or the island styles/`--island-*` block in src/styles.css.

## Why
User feedback F-02c: on a large screen the analytics area is a fixed 300px box with an inner scrollbar while plenty of screen space sits unused below the window. The 300px lock exists for a real reason — subtab switches must NEVER resize the OS window (anti-jank, see the comments around `#analytics` in styles.css and fitWindow in src/main.ts) — so the fix is to size the box from the screen's available space once per mode entry, not per subtab.

## Design (decided)
- Replace the hardcoded `#analytics { height: 300px }` with `height: var(--analytics-h, 300px)`.
- New helper in src/main.ts, e.g. `sizeAnalytics()`: compute
  `budget = window.screen.availHeight - OTHER - MARGIN` where OTHER = the measured height of everything else in the panel (use `contentHeight()` minus the #analytics element's current offsetHeight, or measure siblings directly) and MARGIN ≈ 40px breathing room for the island offset + window chrome;
  `h = clamp(300, budget, 640)`; write it via `document.documentElement.style.setProperty("--analytics-h", h + "px")` (or on #panel).
- Call it at every mode-entry point BEFORE `fitWindow()` measures (the F-06 lesson — render/size first, measure after): setExpanded(true) when !compact, applyCompact() switching to the Usage tab, closeSettings() returning to the Usage tab. Do NOT call it on subtab clicks or in the 1s tick — the box must stay fixed while the user browses subtabs.
- Short subtabs will show internal whitespace instead of a scrollbar on big screens; that is the accepted trade-off (anti-jank wins). Content taller than the box still scrolls inside as today.
- `showAnalyticsSkeleton()` renders fixed-height placeholder blocks; make sure it still looks sane in a taller box (stretch the chart placeholder with flex or min-height if trivial; do not over-engineer).
- Guard: if `window.screen.availHeight` is unavailable or tiny (e.g. < 700), the clamp floor keeps today's 300px behavior. Nothing else may shrink below 300.

## Done criteria
- `npx tsc --noEmit -p tsconfig.json` clean.
- `npx vitest run` all green (if you extract the clamp/budget math as a pure function, add a small unit test for it).
- No Rust changes; do not run cargo. Do not start any dev server. Do not commit.
- Print a final summary: files touched, the exact call sites where sizeAnalytics() runs, judgment calls, and how each done-criterion was verified.
