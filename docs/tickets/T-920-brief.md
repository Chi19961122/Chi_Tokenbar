# T-920 Atoll 身分/打包核心改名 — implementation brief

Implement in `C:\Coding\TokenBar\TokenBar-Src` (Tauri 2 + vanilla TS front, Rust back). Do NOT commit. Do NOT run `tauri dev`/`build` or touch the running dev instance (port 1420). `cargo test` may block on the target lock — wait. Background PATH: `export PATH="$HOME/.cargo/bin:$PATH"`.

## Why
Product is renamed **TokenBar → Atoll**, shipping v0.8.0 as Atoll. This ticket does the **identity + build/packaging core + Rust runtime strings** only. It does NOT touch the front-end brand surfaces (index.html, styles.css, src/i18n.ts, src/share.*, src/mock.ts, src/share-panel.ts) — those are T-921/T-922. Full plan + rationale: `docs/ROUND-v090.md`.

## Exact changes (each verified to exist by scout + plan-verifier)

### Build / identity
- `src-tauri/tauri.conf.json`: `productName` "TokenBar"→"Atoll" (L3); `identifier` "com.qqq01.tokenbar"→"com.qqq01.atoll" (L5); main window `title` "TokenBar"→"Atoll" (L17).
- `src-tauri/Cargo.toml`: package `name` "tokenbar"→"atoll" (L2); `[lib] name` "tokenbar_lib"→"atoll_lib" (L9).
- `src-tauri/src/main.rs`: `tokenbar_lib::run()`→`atoll_lib::run()` (L5) — MUST match the new lib name.
- `package.json`: `name` "tokenbar"→"atoll" (L2). (Leave package-lock.json name; `npm install` isn't run here — if tsc/vitest need it consistent, update the top-level `name` in package-lock.json L2/L8 too, but do NOT regenerate the lockfile.)
- `scripts/collect-installers.mjs`: outName "TokenBar-release"→"Atoll-release" (L15); exe path `tokenbar.exe`→`atoll.exe` (L43); portable "TokenBar-portable.exe"→"Atoll-portable.exe" (L44); version regex `/^TokenBar_(\d+\.\d+\.\d+)_/`→`/^Atoll_(\d+\.\d+\.\d+)_/` (L62).
  - Note (do not "fix"): Windows filenames are case-insensitive, so whether Tauri emits `atoll.exe` or `Atoll.exe`, the lowercased `atoll.exe` here matches. Do not add case handling.

### Settings dir + one-time migration (config.rs)
- `src-tauri/src/config.rs` L274: change the hardcoded `.join("TokenBar")` → `.join("Atoll")` so the path becomes `%APPDATA%\Atoll\settings.json`.
- **Add a one-time migration** so the user's existing settings survive. Put it inside `config::load()` (called from `lib.rs:388`), BEFORE the `read_to_string` of the new path: if the new `%APPDATA%\Atoll\settings.json` does NOT exist but the old `%APPDATA%\TokenBar\settings.json` DOES, copy old→new once (create the Atoll dir first). Then proceed to read the new path as before.
  - Three states, all must hold: (a) old exists, new absent → copy, then load migrated values; (b) new exists → never overwrite from old; (c) neither → fall through to defaults.
  - Add unit test(s) covering the three states (use a temp dir / injectable base path if the code allows; otherwise test the decision helper you factor out — keep the copy decision pure and testable, mirroring how the codebase tests logic).

### Rust runtime strings (lib.rs) — user-facing
- `src-tauri/src/lib.rs`: tray id `with_id("tokenbar")`→`"atoll"` (L444); menu `"Quit TokenBar"`→`"Quit Atoll"` (L441); tray tooltip init `"TokenBar — starting…"`→`"Atoll — starting…"` (L449); tooltip fallbacks `"TokenBar — no data"` (L708) and the `vec!["TokenBar"...]` first line (L710)→"Atoll"; share-preview window `.title("TokenBar Share Preview")`→`"Atoll Share Preview"` (L369); notification titles `.title("TokenBar")` (L829, L877)→"Atoll".

### Rust user-visible provider strings (anthropic.rs) — DO NOT MISS (plan-verifier gap A/B)
- `src-tauri/src/providers/anthropic.rs` L102: the `user_hint` string `"Claude's response wasn't recognized; TokenBar may need an update."` → replace "TokenBar" with "Atoll" (this shows in the UI on schema failure).
- **COUPLED**: `src-tauri/src/providers/anthropic.rs` L734: `let scan = h.replace("TokenBar", "").to_lowercase();` → change the replace target to `"Atoll"`. Reason: this strips the product name out of the hint before scanning for jargon words (the jargon list at L736 contains `"token"`); if L102 now says "Atoll" but the replace still targets "TokenBar", the strip misfires. Update both together.

### User-Agent (cosmetic, low-risk)
- `src-tauri/src/providers/anthropic.rs` L426 and `src-tauri/src/providers/codex_live.rs` L66: `.set("User-Agent", "tokenbar")`→`"atoll"`. (UA only; does not affect rate limiting, which is client_id-based.)

### Build-critical docs
- `CLAUDE.md` and `AGENTS.md`: the pre-build kill line `taskkill /IM tokenbar.exe /F` + `taskkill /IM TokenBar-portable.exe /F` → **new names** `atoll.exe` / `Atoll-portable.exe`; and any `..\TokenBar-release\` → `..\Atoll-release\`. (Prose "# TokenBar — 專案指引" title can also flip to Atoll; keep it minimal — build-critical lines are the must.)

## Out of scope (do NOT touch — owned elsewhere)
- Front end: `index.html`, `src/styles.css`, `src/i18n.ts`, `src/share.ts`, `src/share.css`, `src/share-panel.ts`, `src/mock.ts` → T-921/T-922.
- `src-tauri/capabilities/` — **does not exist as source** (only gitignored build output). No change.
- App icon (`icon-source.png`, `icons/`) → T-923.
- README / other docs prose → defer.

## Gotchas / notes
- autostart: `tauri-plugin-autostart` binds an OS Run-registry entry to app identity + exe path. Changing identifier/exe means the old autostart entry may orphan. You cannot fix the OS registry here; just be aware and do not add migration for it (it's an orchestrator/user real-machine check).
- Do not regenerate lockfiles or run `npm install`.

## Done criteria
- `npx tsc --noEmit -p tsconfig.json` clean; `npx vitest run` all green; `export PATH="$HOME/.cargo/bin:$PATH" && cargo test --manifest-path src-tauri/Cargo.toml` all green (Cargo/lib rename compiles; migration tests pass).
- Report: files touched, the migration implementation + how the three states are tested, confirmation anthropic.rs L102+L734 both updated, and anything needing a live Tauri/real build to verify (exe name, installer, autostart) — be explicit; orchestrator verifies live.
- Do not commit.
