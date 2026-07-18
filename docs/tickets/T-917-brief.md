# T-917 Grok context-fill limit card + source list slim-down — implementation brief

Implement in C:\Coding\TokenBar\TokenBar-Src (Tauri 2 + vanilla TS + Rust). Do NOT commit. Do NOT touch the running dev instance (port 1420); never run tauri dev; mock preview ports ≥5200 only, kill yours after.

## Hard boundaries
- providers/anthropic.rs: untouched (credential file). providers/codex.rs semantics protected.
- Island pill: **must NOT render Grok** — the island stays quota-sources-only (Claude/Codex, user decision 2026-07-18). Island styles/`--island-*` untouched; verify grok limits can never leak into island rendering (pickIslandLimit/worstOf filter by provider — keep that airtight).
- Six share cards untouched; §0 privacy holds.

## User decisions (verbatim intent)
- 勾選的供應商要出現在限額頁。Grok 的卡片用 **context 填充率**。
- **把 OpenCode/Gemini 從供應商整個移除**。
- 島嶼只留額度來源（Claude/Codex）。

## Work
### A. Sources slim-down (claude/codex/grok only)
1. `KNOWN_SOURCES` → ["claude","codex","grok"]; sanitize drops opencode/gemini from stored files; migration no longer maps tool_opencode/tool_gemini (legacy fields may remain in the struct for downgrade write-back — write them as false — or drop entirely if serde default covers old files; choose and note).
2. Delete `scan_opencode`/`scan_gemini` (+ their record parsers, opencode_bases, accounts entries, related tests) from analytics.rs; compute gating handles the two fewer sources. Remove `settings.toolOpencode/toolGemini/toolNote` i18n keys and any UI remnants. Frontend `ALL_SOURCES` → three; chips row shows three.

### B. Grok context-fill limit (new provider surface)
1. **Model**: `Provider` gains `Grok` (Rust model.rs + TS types). Limit: id `grok.ctx`, label "Grok·Context", provider grok.
2. **New provider module** `providers/grok.rs` (local file read only, NO network): find the NEWEST session dir under `~/.grok/sessions/*/*/signals.json` (by mtime); parse `contextTokensUsed` + `contextWindowTokens` (+ `primaryModelId` for the label hint if cheap); util = used/window ×100 clamped 0..100. Freshness semantics mirror the codex local source's spirit: signals.json mtime older than ~15min → keep value but status Stale/Idle (pick the closer semantic and note it); missing/no sessions → insufficient_data placeholder, NEVER a fake 0%. resets_at = 0 (context has no reset schedule). status thresholds: reuse the warn/crit settings for near coloring (approaching a full context window is exactly what warning colors are for). Poll every scheduler round gated on "grok" in sources (file stat + small JSON read = cheap; no backoff needed).
3. **Panel**: PROVIDER_META/PROVIDER_ORDER gain grok (name "Grok", a simple brand-ish icon in icons.ts — a minimal ⨯/𝕏-like mark or the letter G, monochrome currentColor like the others); LIMIT_NAME_KEYS: `grok.ctx` → i18n "Context window"/「Context 視窗」; the row note line: instead of a reset time, show a per-session hint (i18n: "Per-session; resets on new session"/「單一 session 用量,新對話歸零」) so the semantic difference from subscription quotas is honest and visible.
4. **Quota summary digest** (buildQuotaSummary/windowShort): grok group shows short "ctx" (fixed English). Island: verify grok never appears (mode derivation unchanged — quota pair only).
5. **visibleLimits** (main.ts): "grok" in sources → provider "grok" limits visible; backend apply_provider_filter likewise.
6. Settings: island pin rows stay Claude/Codex only (grok has one limit, nothing to pin).

## Done criteria
`npx tsc --noEmit -p tsconfig.json` clean; `npx vitest run` green; `export PATH="$HOME/.cargo/bin:$PATH" && cargo test --manifest-path src-tauri/Cargo.toml` green — new tests: grok signals parsing (fresh/stale/missing → correct status), util math + clamp, sources sanitize drops opencode/gemini, migration matrix updated, island exclusion (renderIsland with a grok limit in snapshot must not render it). Report: files touched, semantic choices (stale vs idle, migration write-back), judgment calls, anything needing live verification. Do not commit.
