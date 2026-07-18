# T-916 provider multi-select + Grok source — implementation brief

Implement in C:\Coding\TokenBar\TokenBar-Src (Tauri 2 + vanilla TS + Rust). Read docs/ROUND-v080.md (T-916) and docs/RUNBOOK.md-equivalent context below. Do NOT commit. Do NOT touch the running dev instance (port 1420 / tokenbar.exe); never run tauri dev; cargo may wait on the target lock. Mock preview only on ports ≥5200, kill yours after.

## Hard boundaries
- src-tauri/src/providers/anthropic.rs: do NOT touch (credential file — any need to change it means STOP and report).
- providers/codex.rs snapshot semantics protected.
- Island pill appearance: layout may derive from which quota providers are selected (existing island-both/single modes), but island styles/`--island-*` stay untouched.
- §0 privacy: project names never reach share surfaces; Grok data follows the same rule.
- Six share cards + their CSS untouched.

## Why
User: 「供應商加入grok、所以目前有claude、codex、opencode、gemini，可以使用多選的選單設計」。Today `providers` is a 3-way (both/claude/codex) plus two separate checkboxes (tool_opencode/tool_gemini). Unify into ONE multi-select of five sources; add Grok as a usage-only source.

## Grok data source (recon already done — trust these findings)
- Location: `~/.grok/sessions/<url-encoded-cwd>/<session-id>/`
- `updates.jsonl`: one JSON object per line; `timestamp` = unix epoch SECONDS; `_meta.totalTokens` = u64 CUMULATIVE from session start; `_meta.modelId` (e.g. "grok-4.5"). Convert cumulative→per-event deltas exactly like scan_codex_lines does for Codex (same monotonic-diff pattern; guard non-monotonic resets by treating a drop as a new baseline, not a negative delta).
- Project attribution: URL-decode the `<encoded-cwd>` path segment, take its basename (usage-tab only; §0 keeps it out of shares automatically since shares never read by_project).
- Model: `_meta.modelId` per line (fallback "grok"). Agent name: "Grok CLI".
- NO input/output/cache breakdown (only totals): put the whole delta in the `input` slot? NO — follow the honest convention: pass it as a total-only record. Look at how scan_codex_lines feeds add_with_cost and mirror the least-lying mapping; document your choice in a comment. Breakdown tiles must not fabricate categories for Grok.
- NO pricing known: cost contribution is 0.0 with a comment (est. cost undercounts when Grok is included — a deliberate honesty choice, do not invent a rate).
- by_kind: Grok is NOT classifiable → excluded, like Codex (硬規定:無法分類就不出假類別).
- mtime prefilter by `start` like the other scanners; files can be large-ish, BufReader lines.

## Settings model + migration
1. New `sources: Vec<String>` on Settings (config.rs), default `["claude","codex","opencode","gemini","grok"]`. Unknown values dropped on load; empty vec allowed (means: nothing polled/scanned — honest empty UI).
2. **Migration** (in config load, with tests): if the stored file has no `sources` but has legacy fields, derive: providers "claude"→ claude only of the quota pair (codex excluded), "codex"→ codex only, else both; append opencode/gemini per tool_opencode/tool_gemini; grok defaults ON for fresh installs but OFF when migrating a legacy file (the user never had it — don't surprise-add a source silently... EXCEPTION: this user asked for grok, so migrating to grok ON is actually the request. Decide: migrate grok ON, note it). Keep writing the legacy fields for one version (write-back both) so a downgrade doesn't explode; read prefers `sources`.
3. Backend plumbing: quota scheduler polls anthropic iff sources contains "claude", codex(+live) iff "codex". analytics compute_with takes the sources list (replace the `filter`+2 bools signature); scan_claude/scan_codex/scan_opencode/scan_gemini/scan_grok each gated by membership. get_analytics + lib.rs settings snapshot accordingly.
4. Frontend: `sources: string[]` in types + DEFAULT_SETTINGS; `visibleLimits()` filters by sources membership (claude→anthropic limits, codex→codex limits); island mode derives: both quota providers selected→"both" (stacked), one→that single, none→empty-state pill; `collapsedSize()` follows. Analytics cache key slice uses the sorted sources joined (replaces the providers segment).
5. **Settings UI**: replace the providers seg AND the two tool checkboxes with ONE multi-select chip row (five toggle chips: Claude / Codex / OpenCode / Gemini / Grok) in the 顯示與通知 group — extend the T-903 segmented vocabulary into a `.seg-multi` variant (chips toggle independently, `.on` state, commit on every click via the existing commitSettings path). i18n keys both locales for any new label (the five names themselves stay English brand names). The 更新頻率/refresh note rows stay.
6. Island context menu (contextmenu.ts) has a Provider switch entry — update it to the new model minimally (e.g. quota-pair toggles only) and note what you chose.

## Done criteria
`npx tsc --noEmit -p tsconfig.json` clean; `npx vitest run` green; `export PATH="$HOME/.cargo/bin:$PATH" && cargo test --manifest-path src-tauri/Cargo.toml` green — including new tests: grok line parsing (cumulative→delta, reset guard, epoch-seconds), migration matrix (legacy providers×tools → sources), gating (a source absent from the list is never scanned). Report: files touched, migration table, the total-only mapping choice, island-mode derivation, judgment calls, anything unverifiable without live Tauri. Do not commit.
