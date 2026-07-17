# T-fix-002 — Codex 累計轉增量 + fork replay 防重
status: done

`只實作本票行為與資料。可沿用現有樣式。禁止大規模 redesign。對照 PLAN flow。`

## 目標

Codex 用量從「每 session tail-read 最後累計值、整包歸單一時間點」改為「逐 token_count 事件累計轉增量、按事件時間歸屬」，並防 fork replay 重複計數。修掉兩個失真：跨午夜長 session 全塞同一天/同一小時；fork 出的新 rollout 檔把同一段歷史再算一次。

## 範圍（只准動這些檔案）

* `src-tauri/src/analytics.rs`

## 規格

1. `scan_codex`（analytics.rs:441 起）改為逐行掃描每個 rollout 檔（先用 `line.contains("token_count")` 粗篩再 parse，同 scan_claude 模式），收集該檔案的 token_count 事件序列：`(timestamp, total_token_usage)`。
2. **累計轉增量**：同一檔案內按出現順序，`delta_i = saturating_sub(total_i, total_{i-1})`（首筆 delta = total_0）。delta 為 0 的事件跳過。每筆 delta 以**該事件的 timestamp** 呼叫 `acc.add(...)` 歸屬（day 桶 + hourly）。
3. **fork replay 防重**：全域 `HashSet<(i64, u64)>`（timestamp 秒 + 累計 total）。事件的 (ts,total) 已見過 → 該事件不產生 delta 也不更新前值基準？——正確語意：fork 檔複製了母檔前綴，重複的 (ts,total) 事件整筆跳過，但仍作為該檔案內下一筆 delta 的基準值（避免 fork 檔的新增量算成從 0 起跳）。
4. by_project 歸屬（`first_cwd_basename`）與 kind=None 的既有語意不變；session 計數 `sessions` 邏輯不變。
5. `total_token_usage` 若是物件（input/cached/output 細項），總量取用既有欄位邏輯（沿用 `last_total_usage` 現在讀的欄位）；細項分開累計留給 T-fix-003，本票只要求總量歸屬正確。
6. 效能：檔案可能很大，逐行掃描取代 tail-read 是本票有意的取捨；只掃 `mtime >= start` 的檔案（既有過濾保留）。
7. 單元測試：
   - 單檔累計序列 [100, 250, 250, 400] → deltas [100,150,0(跳),150]，各歸自己的 timestamp。
   - 跨午夜：兩事件分屬兩天 → daily 兩天各有量（tail-read 舊行為會全歸一天）。
   - fork replay：母檔 [100,250]、fork 檔 [100,250,400] → 總量 400 不是 750。
   - 倒退（total 變小，理論不應發生）→ saturating 為 0，不 panic、不負數。

## SPEC / PLAN 依據

* docs/PLAN.md 功能 Delta「scan_codex fork replay 防重 + 累計轉增量逐筆歸屬」
* CLAUDE.md 鐵則：providers/codex.rs 的快照語意**不碰**（本票只動 analytics 掃描）

## Out of scope（這張票不碰）

* providers/codex.rs、providers/codex_live.rs（額度側，語意勿回退）
* Claude/OpenCode/Gemini 掃描
* 前端、定價

## Build / Verify

    前置:   PATH 加 %USERPROFILE%\.cargo\bin
    檢查:   cargo test --manifest-path src-tauri\Cargo.toml
    前端:   npm test && npm run build

驗收：

| 檢查 | 做什麼 | 期望看到 |
| --- | --- | --- |
| cargo test | 跑後端測試 | 新增 ≥4 個測試全過，既有測試不壞 |
| 語意檢查 | 讀 providers/codex.rs diff | 零 diff（保護檔） |
