# T-912 activity-type taxonomy expansion — implementation brief

Implement in this repo (Tauri 2 + vanilla TS + Rust). Do NOT commit. Do NOT touch the running dev instance (port 1420); cargo may wait on the target lock. Files you may touch: src-tauri/src/analytics.rs (classification only — nothing in the scan/credential/cost paths beyond the kind mapping), src/analytics.ts (donut colors/labels), src/i18n.ts (new kind labels, BOTH locales), and the matching test files.

## Why
User feedback: 「拆分頁籤的活動類型需要顯示多一點類型」— the Breakdown donut only shows Edit/Read/Run/Other because `classify_kind` in analytics.rs buckets every Claude tool name into four kinds. Expand the taxonomy.

## Work
1. **Backend taxonomy** (`classify_kind` and its call sites): expand to:
   - `edit` — Edit, Write, NotebookEdit
   - `read` — Read
   - `search` — Grep, Glob
   - `run` — Bash, PowerShell (and any shell-execution names already mapped)
   - `web` — WebFetch, WebSearch
   - `agent` — Task, Agent, and names starting with "Agent"
   - `mcp` — names starting with `mcp__`
   - `other` — everything else
   First INVENTORY what tool names actually appear: grep the existing mapping/tests and (read-only) sample a couple of real log lines' tool `name` fields from ~/.claude/projects if helpful — names only, never message content. Adjust the buckets to reality; keep the hard rule 無法分類就不出假類別 (Codex stays excluded from by_kind).
2. **Ordering**: by_kind output should sort by tokens desc as today (verify; keep whatever the current contract is — check the existing tests).
3. **Frontend donut**: `kindColor` currently cycles 4 CSS-var colors; extend to 8 distinct theme-safe slots using EXISTING tokens only (the --ink ramp, --prov-claude, --prov-codex, --accent — pick an ordering with adjacent-slice contrast in both themes; note your choice). `kindLabel` gains the new kinds via i18n keys (both locales: search=搜尋, web=網路, agent=代理, mcp=MCP — en: Search/Web/Agent/MCP). Legend must not overflow the panel with 8 entries × zh strings — check the .donut-legend wrap behavior and adjust the analytics CSS region minimally if needed.
4. **Tests**: backend — classification cases for each new bucket + the mcp/agent prefix rules; frontend — donut renders new labels; i18n parity is compile-enforced.

## Done criteria
`npx tsc --noEmit -p tsconfig.json` clean; `npx vitest run` green; `export PATH="$HOME/.cargo/bin:$PATH" && cargo test --manifest-path src-tauri/Cargo.toml` green. Report: the final name→kind table, color slot ordering with contrast reasoning, files touched. Do not commit.
