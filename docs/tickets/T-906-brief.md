# T-906 share preview click-to-zoom window — implementation brief

Implement in this repo (Tauri 2 + vanilla TS in src/, Rust in src-tauri/). Do NOT commit. Do NOT kill or interfere with the running dev instance (port 1420, tokenbar.exe) — never run `tauri dev` yourself; `cargo test` may wait on the target-dir lock, that's fine. Do not start any dev server except a throwaway vite on a port ≥5200 if you need a browser sanity check (kill it after).

## Why
User decision (confirmed): clicking the share-card preview in the report subtab should open a NEAR-FULLSCREEN separate preview window showing the card big — the 380px panel window itself cannot host a meaningful zoom. Esc or a click anywhere closes it.

## Architecture (decided — follow it)
Click preview → frontend generates a PNG **data URL of the card via the exact existing export pipeline** (share-panel.ts already builds an offscreen holder + toPng with CARD_DIM; reuse that path so the zoom shows exactly what an export produces, at export resolution — 1200×675@1 for auto, 1080×1920 for story) → invoke a new Rust command `open_share_preview(data_url: String)` which (a) stores the data URL in managed State, (b) creates (or refreshes) a WebviewWindow labeled `share-preview` loading the app URL with hash `#share-preview`, sized ~90% of the current monitor's work area, centered, decorations(false), always_on_top(true), skip_taskbar(true), focused. If the window already exists: update the State and either emit an event the window listens to, or close-and-recreate — pick the simpler robust option and note it.
The preview window's frontend: in `boot()` (src/main.ts), FIRST check `location.hash === "#share-preview"` and branch into a minimal preview boot — no island, no snapshot subscription, no analytics: render a near-black scrim (rgba(0,0,0,.93)), the image centered with `max-width:92vw; max-height:92vh`, a soft drop shadow, and a small dim hint (both locales via i18n, e.g. "Esc / 點擊關閉"); pull the data URL via a `get_share_preview()` command; close the window on Esc keydown or any click (`getCurrentWindow().close()`).

## Gotchas to handle
- **Capabilities**: src-tauri/capabilities/*.json likely scopes permissions to the `main` window. The `share-preview` window must be allowed to call `get_share_preview` (and window.close). Add the label/permissions minimally — do not widen main's permissions.
- The preview window uses the SAME vite/devUrl in dev and bundled index.html in release — the hash branch must work in both (no separate HTML file).
- The click target in the report panel: the preview card only (not the style/range/size controls). Add `cursor: zoom-in` on the hoverable preview and a title tooltip (i18n both locales). Clicks that start a drag must not trigger it if that's already a concern in that area (check how the panel handles clicks; keep it simple otherwise).
- Generating the PNG takes a moment (~100-500ms): give lightweight feedback (e.g. reduce opacity or a busy cursor on the mat while generating; reuse existing vocabulary, no spinner invention).
- The stored data URL can be a few MB — fine in memory; overwrite on each open, no cleanup needed.
- share.css cards + island remain untouched. The scrim/hint styles are APP chrome — they may use global tokens but the scrim is intentionally near-black in both themes (note: the preview window may not have the .dark class applied — that's fine, hardcode the scrim, keep the hint readable on it).

## Done criteria
- `npx tsc --noEmit -p tsconfig.json` clean; `npx vitest run` all green; `export PATH="$HOME/.cargo/bin:$PATH" && cargo test --manifest-path src-tauri/Cargo.toml` all green (new commands compile; add a trivial state test if cheap).
- Report: files touched, how window-reuse is handled, capability changes made, judgment calls, what you could NOT verify without a live Tauri run (be explicit — the orchestrator will verify live).
- Do not commit.
