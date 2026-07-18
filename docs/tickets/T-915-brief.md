# T-915 六款分享卡照比稿重設計實作 (share.ts + share.css) — implementation brief

Implement in this repo (Tauri 2 + vanilla TS). Do NOT commit. Do NOT run `tauri dev` or touch the running dev instance (port 1420). Browser sanity check: throwaway vite on port ≥5200 only, killed after. Runs AFTER T-914 (share moved to full-page); if T-914 is already applied, keep working on top of it — this ticket only touches `src/share.ts`, `src/share.css`, `src/types.ts` (additive), `src/i18n.ts` (additive), and possibly `public/fonts/` + `src/fonts.css` (serif — see §Fonts). Do NOT touch the shell/routing (main.ts/index.html) — that's T-914.

## Why
The six share cards get a full visual redesign, approved by the user against the pixel spec **`design/refs/share-redesign-preview.html`** (open it in a browser — it is the SOURCE OF TRUTH for markup + CSS of all six templates in both 16:9 and 9:16). Reimplement the six existing renderers to match it, wired to REAL analytics data. The mockup already bakes in the four user decisions below — do not deviate from it.

## User decisions (already reflected in the mockup — implement exactly)
1. **Quota %  = USED %** on the island_card gauge (e.g. `38% used / 42% used / 55% used`), fills `#18181B / #52525B / #71717A`. ⚠️ This is the ONE place a subscription % appears, and it uses **util (used)** — opposite to the app's `% left` convention on purpose. Do NOT apply `% left` here; the share card is its own vocabulary. All OTHER cards show token/cost/share%, no quota.
2. **Quota gauge appears ONLY on island_card.** The other five templates have no quota rows.
3. **Signature date = month-year, uppercase mono `JUL 2026`** (from the period's last day; fixed month table — never `toLocale*`).
4. **All six templates ship** (statement / diagnostics / minimal / fuel / island_card / wa), 16:9 + 9:16.

## Current code (what you're replacing)
- `src/share.ts` — `ShareData` interface (line 39), `buildShareData()` (165), six renderers: `statementCard` (282) `.shst-card`, `diagnosticsCard` (323) `.shdx-card`, `minimalCard` (368) `.shmn-card`, `fuelCard` (402) `.shfl-card`, `islandCard` (450) `.shic-card`, `waCard` (493) `.shwa-card`. `renderShareCard()` (235) switch + `sh-916` class for story size. `BATTERY_SVG` (276). `esc/grouped/money/splitAbbrev/barPct/recordsCaption` helpers.
- `src/share.css` — `--share-*` tokens (theme-invariant, pinned light), six `.shXX-card` blocks, `.sh-916` portrait variants, `.sharep` panel chrome (leave `.sharep` and tokens alone except where a card needs a new `--share-*`).
- Class-prefix mapping mockup → repo: `.st`→`.shst-card`, `.dx`→`.shdx-card`, `.mn`→`.shmn-card`, `.fl`→`.shfl-card`, `.ic`→`.shic-card`, `.wa`→`.shwa-card`. Mockup portrait `.p` (e.g. `.st.p`) → repo `.sh-916` (e.g. `.shst-card.sh-916`). The mockup's `.stage16 > .card{ transform:scale(.5) }` is preview-only chrome — IGNORE it; the real card renders at true 1200×675 / 360×640 and the export pipeline scales it.

## Data plumbing to ADD (all fields exist in Analytics — thread them through)
`src/types.ts:152 Analytics` already has: `sessionsThisWeek` (182), `hourly: number[]` 24 buckets (168), `records.maxHour {hour,tokens}` (163), `records.maxDay` + `records.streakDays` (162-164). Extend `ShareData` + `buildShareData()`:
- `sessionCount: number` ← `a.sessionsThisWeek`. (It is week-scoped; that's the only session metric the backend gives — use it as-is, label neutrally "sessions"/"場次". Don't fabricate a per-range count.)
- `hourly: number[]` ← `a.hourly` (24). For the diagnostics **sparkline**: bar heights = `value / max(hourly) * 100`, the peak bar gets `.pk` (accent). If `hourly` is all-zero, render flat/empty gracefully.
- `peakHour: number` ← `a.records.maxHour.hour`. Render `HH:00` (zero-pad, fixed — e.g. `14:00`). Used by diagnostics/minimal "peak 14:00".
- `genMonthYear: string` ← uppercase `MON YYYY` from the period's last daily date via the existing `MONTHS_EN` table (e.g. `JUL 2026`). Fall back to the first/any available daily date; if `daily` empty, omit the date suffix rather than guessing.
- **Structured quota gauge** (island_card only): replace the single `quotaNote` string path with `quotaGauge?: { label: string; util: number }[]` (≤3 rows). Build from `BuildOpts.limits` when `includeQuotaNote` is on: Claude 5h, Claude week, Codex week — whichever exist, in that order. `label` like `"Claude · 5h" / "Claude · week" / "Codex · week"` (brand fixed English; the "5h/week" descriptor may follow locale per the existing `buildQuotaNote` concept). `util` = the limit's `util` (0-100, used). Keep the existing §0 reasoning: this is the sanctioned single exposure of subscription %. You may keep `buildQuotaNote` or refactor it into `buildQuotaGauge` — island is the only consumer now.

## The six templates — reimplement markup+CSS to match the mockup
Port each template's markup and CSS from `design/refs/share-redesign-preview.html` under the repo class prefixes, wired to `ShareData`. Highlights per template (see mockup for exact structure/spacing/type-scale):
- **statement** `.shst-card`: serif masthead "Usage Statement" + `::after` inset hairline border, right meta `This week · Jul 12–18` + mono `NO. TB-YYYY-MMDD` doc number (derive from period last day), serif 92px total tokens with small `M` unit + subline `across N agents · K sessions · streak Nd · peak X/day`, right cost cell, dotted-leader ledger rows (byAgent), unified footer signature.
- **diagnostics** `.shdx-card`: macOS titlebar (accent first dot), mono `$ tokenbar --report …` + blinking-block cursor (static ok), 64px `TOTAL_TOKENS`, KV cost/sessions, **24h sparkline** (`hourly`, peak `.pk`, label `peak HH:00`), inline-bar table (byAgent), `— EOF —` + signature.
- **minimal** `.shmn-card`: 224px total with accent unit `M`, caption `tokens · period · streak/sessions`, two-column ultra-thin split tracks (byAgent), footer `Peak HH:00` + signature.
- **fuel** `.shfl-card`: accent canopy + faint grate, black **pump display panel** (mono glowing total `324.9M` + `$` total sale), grade rows `01–04` (byModel default, `fuelGroup` toggle keeps byAgent) with dotted leaders + tabular tokens + %, footer `K sessions · streak Nd` + signature.
- **island_card** `.shic-card`: raised black island pill (battery + `TokenBar` + accent `LIVE`), period, 96px total + cost + sessions on one baseline, **the quota gauge** (3 pill tracks, used %, fills per decision #1), hairline footer + signature. This is the only card reading `quotaGauge`.
- **wa** `.shwa-card`: left vertical serif column `Cumulative Ledger` + vertical rule, serif 100px total, cost, 1px hairline split tracks (byAgent), bottom-left signature, bottom-right accent `量` seal with double white inset stroke.

**Unified signature system** (all six, per mockup `.sig`/`.sig-r`): `BATTERY_SVG` + `TokenBar` + mono `· JUL 2026` (`genMonthYear`). Consolidate the footer markup so all six share one signature vocabulary.

## Fonts (decide + note)
The mockup leans on a **serif (Playfair Display)** for statement masthead, minimal isn't serif, wa numbers, etc. The project bundles Geist / Geist Mono (`public/fonts/`, `src/fonts.css`) but likely NOT Playfair. Check: if Playfair is not bundled, either (a) bundle it locally (woff2 in `public/fonts/` + `@font-face` in `fonts.css`, self-hosted — NO network/CDN, the cards export offline) or (b) fall back to a bundled serif and note the visual compromise. Pick one, do it, and say which in the report. Do NOT introduce any external font URL.

## Hard rules (do not break)
- **§0 privacy**: never read `Analytics.byProject`, project/host names, or conversation content. Only totals, cost, byAgent, byModel, hourly, records, sessionsThisWeek, limits.
- **Theme-invariant**: cards use only `--share-*` (pinned light); never `--ink-*` / `.dark` / app theme tokens. Add new `--share-*` tokens if a color is needed (mockup uses `--accent:#EC4899` = keep existing share accent).
- **Export resolution unchanged**: auto → 1200×675, story → 1080×1920 (share-panel export pipeline; you only change card content, geometry stays 1200×675 / 360×640 CSS px).
- Injection-safe: keep `esc()` on all data-derived names.
- Keep `renderShareCard()` signature + `sh-916` story mechanism + `fuelGroup` option.

## i18n
Add the new human-readable strings both locales (en + zh) in `src/i18n.ts`: statement/report labels, "sessions/場次", "peak/尖峰", "quota used this cycle", "cumulative usage", "fuel dispensed", "total sale", "cumulative ledger", "est", etc. Keep terminal/mono tokens literal & untranslated (`TOTAL_TOKENS`, `EST_COST_USD`, `SESSIONS`, `EOF`, `NO. TB-…`, grade numbers, the `量` seal, `TOKEN STATION`). Follow the existing `share.*` key style; reuse existing keys where they already fit (many do). zh translations for the human labels, techy literals stay as-is.

## Done criteria
- `npx tsc --noEmit -p tsconfig.json` clean; `npx vitest run` all green (update/extend `src/share.test.ts` if it asserts on old markup — keep decision-logic tests meaningful: quota gauge build, genMonthYear, sparkline scaling, splits); `export PATH="$HOME/.cargo/bin:$PATH" && cargo test --manifest-path src-tauri/Cargo.toml` all green.
- All six cards render in both 16:9 and 9:16 matching the mockup, with real data wired (verify in a throwaway vite mock preview — mock.ts scenarios drive data; screenshot each if feasible, but DOM assertions are fine given the 1s-tick screenshot flakiness).
- Report: files touched, the ShareData additions, the font decision, any place the mockup couldn't be matched 1:1 with real data (be explicit), and what needs live-Tauri/real-machine confirmation (export PNG fidelity, Geist rendering). Orchestrator + user verify visuals live.
- Do not commit.
