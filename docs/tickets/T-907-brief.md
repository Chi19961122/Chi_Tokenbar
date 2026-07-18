# T-907 share preview zoom latency — implementation brief

Optimize the T-906 share-preview zoom flow in this repo (Tauri 2 + vanilla TS). Do NOT commit. Do NOT kill or touch the running dev instance (port 1420 / tokenbar.exe); never run `tauri dev`; cargo test may wait on the target lock — fine.

## The problem (user feedback)
「戰報預覽放大的載入速度有點慢」— the current flow is SERIAL: click → html-to-image `toPng` renders the export PNG (the slow part — it re-embeds the bundled webfonts as data URLs on every call: Geist, Geist Mono, Playfair, Noto TC) → only then `open_share_preview` destroys+recreates the WebView2 window (several hundred ms more) → the window boots and pulls the image. The user stares at a dimmed mat for the whole chain.

## The fix (decided — follow it)
1. **Parallelize + instant feedback.** On click: immediately invoke a window-open command (no payload yet) so the preview window appears right away showing its scrim + a subtle generating state (reuse the existing hint pill styling, i18n both locales, e.g. 產生中…/Rendering…); concurrently start the PNG generation; when it resolves, hand the data URL to the backend and notify the window to swap the image in.
2. **Race-proof the handoff — pull + subscribe, never event-only**: the preview window on boot (a) pulls `get_share_preview` (may be empty → keep the generating state), (b) subscribes to an update event; the backend stores the data URL in State FIRST, then emits the event to the `share-preview` window; on event the frontend pulls again (or uses the event payload — but the stored-state pull must remain the source of truth so a lost event or an event-before-subscribe can never strand the window: also re-check state right after subscribing). Clear stale state on each new open so the window never flashes the PREVIOUS card while the new one renders.
3. **Cache the font-embed CSS**: use html-to-image's `getFontEmbedCSS()` once per session (memoize on first use) and pass it as `fontEmbedCSS` to every `toPng`/`toBlob` call — this speeds up the zoom AND the existing PNG/copy export buttons. Invalidate nothing (fonts never change at runtime).
4. Keep the destroy+recreate-per-open window strategy (memory-lean) — the parallel open hides its cost. If reuse-when-already-open is trivial (window exists → just update state + emit + focus), you may do it as a bonus; note it.
5. Capabilities: the preview window will now need event-listen permission (`core:event:default` or the minimal listen permission) in src-tauri/capabilities/share-preview.json — keep it minimal, don't widen main.

## Done criteria
- `npx tsc --noEmit -p tsconfig.json` clean; `npx vitest run` all green (update/extend the share-preview tests for the new pull+subscribe logic — the race-proofing deserves a unit test if the module structure allows); `export PATH="$HOME/.cargo/bin:$PATH" && cargo test --manifest-path src-tauri/Cargo.toml` all green.
- Report: files touched, the exact open→generate→swap sequence with its race analysis, capability changes, what cannot be verified without a live Tauri run. Do not commit.
