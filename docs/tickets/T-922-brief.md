# T-922 分享卡六款新方向 → Atoll (share.ts + share.css) — implementation brief

Implement in `C:\Coding\TokenBar\TokenBar-Src`. Do NOT commit. Do NOT run `tauri dev`/`build` or touch port 1420. Throwaway vite ≥5202 only for a mock sanity check (kill after). This runs alongside/after T-920 (identity, disjoint files). You OWN: `src/share.ts`, `src/share.css`, `src/share.test.ts`, and the card-related label keys in `src/i18n.ts`. Do NOT touch index.html / styles.css / share-panel.ts / mock.ts (T-921), or any Rust/build (T-920).

## Why
Six share cards get a full visual redesign for Atoll. **Pixel truth: `design/refs/atoll-share-preview.html`** — open it in a browser. It has all six in 16:9 (1200×675) and 9:16 (360×640), already browser-verified for no overflow. Reimplement the six renderers to match it, wired to the REAL analytics data (the T-915 pipeline is already in place — reuse it).

## The six directions map onto the existing six ShareStyle keys (keep keys stable — they're persisted in `share_style`)
Do NOT rename the `ShareStyle` union keys (`statement|diagnostics|minimal|fuel|island_card|wa`) — they persist in settings. Change only what each RENDERS + its display label:
| ShareStyle key | New design (mock class) | Display label (en / zh) |
|---|---|---|
| `island_card` | **Atoll ring gauge** `.at` — flagship/default | Atoll / 環礁儀表 |
| `statement` | **Ledger** `.lg` | Ledger / 結算單 |
| `diagnostics` | **Terminal** `.tm` (`atoll --report`) | Terminal / 終端 |
| `minimal` | **Minimal** `.mn` | Minimal / 極簡 |
| `fuel` | **Sounding** `.sd` (lagoon depth) | Sounding / 測深 |
| `wa` | **Seal** `.sl` (◎ ring seal) | Seal / 環印 |
- Default `share_style` should resolve to the flagship (`island_card` = Atoll ring). If there's a hardcoded default elsewhere, leave the key but confirm it lands on the Atoll ring visually.
- If the report/share panel renders a style picker with these labels, update the labels (via i18n) to the new names. Find where the picker labels come from; if they're in i18n keys you own the card-label ones — update en+zh. If a label lives in share-panel.ts, note it for T-921 (do not edit share-panel.ts).

## Class-name mapping (mock → repo)
Port the mock's per-card CSS + markup under the existing `.shXX-card` prefixes: mock `.at`→`.shic-card` (island_card slot), `.lg`→`.shst-card`, `.tm`→`.shdx-card`, `.mn`→`.shmn-card`, `.sd`→`.shfl-card`, `.sl`→`.shwa-card`. Mock portrait `.p` (e.g. `.at.p`) → repo `.sh-916` (added by `renderShareCard` for story size). The mock's `.stage16 > .card{ transform:scale(.52) }` is preview chrome — IGNORE; real cards render at true 1200×675 / 360×640 and the export pipeline scales.

## Data — reuse the existing T-915 pipeline (already in `share.ts`)
`buildShareData()` already provides: `totalTokens, totalCostUsd, byAgent, byModel, sessionCount, hourly, peakHour, genMonthYear, docNo, quotaGauge (≤3: Claude 5h/week, Codex week, used%), streakDays, maxDayTokens, agentCount, periodLabel`. Wire the mock's data slots to these. Specifics:
- **Atoll ring** (`island_card`): the 3 concentric arcs = `quotaGauge` (used %); center lagoon label static ("Quota used / this cycle"); big total, cost, sessions, streak. Arcs via SVG `pathLength="100"` + `stroke-dasharray="{util} 100"`, colors `#18181B / #52525B / #71717A` per row (mock). If <3 gauge rows, render only those arcs+legend rows.
- **Terminal** (`diagnostics`): sparkline from `hourly` (bar heights = v/max*100, peak bar `.pk`), `peak HH:00` from `peakHour`, TOTAL_TOKENS/COST/SESSIONS. Command reads `atoll --report`.
- **Sounding** (`fuel`): the depth-profile area path — derive from `hourly` if feasible (map 24 buckets to a smoothed area/curve; the mock hand-authored a path — a reasonable data-driven curve from `hourly` is the goal, peak marker at `peakHour`). If a faithful data curve is hard, a simpler `hourly`-driven area (bars→area) is acceptable; note what you did. Layers = byAgent top-2 with %.
- **Ledger/Minimal/Seal**: total, cost, byAgent rows, period, `genMonthYear` signature.
- All six: unified signature = ◎ ring-mark SVG + "Atoll" + mono `genMonthYear` (e.g. "JUL 2026"). Replace the old `BATTERY_SVG` with the ◎ ring mark (two concentric circles + center dot, `currentColor`).

## Hard rules
- **§0 privacy**: never read `byProject`/project/host/conversation. Only the ShareData fields above.
- **Theme-invariant**: cards use `--share-*` only; never app theme tokens. Keep the pinned light palette. (Add `--share-*` vars if a new color is needed; accent stays `#EC4899`.)
- **Export resolution unchanged**: auto 1200×675, story 1080×1920. Card geometry stays 1200×675 / 360×640.
- Keep `renderShareCard()` signature + `sh-916` story mechanism + `fuelGroup` option (fuel/Sounding may still accept the model/agent group toggle, or drop it if Sounding doesn't use it — note the choice).
- Injection-safe: keep `esc()` on all data-derived names.
- **Zero em/en dashes** in any card copy (use hyphen). Rationed middots. (The mock already follows this.)
- **Serif**: Ledger masthead + Seal numerals use serif; the Playfair roman subset is already bundled (T-915). Reuse it.

## Detail refinement (user-authorized)
The mock is direction-truth; you may reasonably converge pixels to real data. Explicitly asked: give the Atoll 9:16 footer a bit more space between "Atoll" and the date. Otherwise polish spacing/scale as needed to fit real values without overflow (all cards `overflow:hidden`).

## i18n
Add/repurpose the card label keys (style names above + any new visible strings: "Quota used", "Lagoon depth", "Cumulative usage", "sessions", "peak", "Cumulative Ledger", etc.) in `src/i18n.ts`, en + zh. Keep terminal/mono literals untranslated (`TOTAL_TOKENS`, `EOF`, `atoll --report`, the ◎ seal, `NO. AT-…`).

## Done criteria
- `npx tsc --noEmit -p tsconfig.json` clean; `npx vitest run` all green (update `share.test.ts` to new markup; keep decision-logic tests meaningful: quotaGauge/genMonthYear/sparkline scaling/splits/§0-no-byProject); `export PATH="$HOME/.cargo/bin:$PATH" && cargo test --manifest-path src-tauri/Cargo.toml` green.
- Sanity-render the six in a throwaway vite (mock mode drives data) and confirm no overflow in 16:9 and 9:16; the flagship (Atoll ring) is the default.
- Report: files touched, the six mappings, how Sounding's curve is data-driven, the fuelGroup decision, any place the mock couldn't be matched 1:1 with real data, what needs live-Tauri/real-machine confirmation (PNG export fidelity, Geist rendering). Do not commit.
