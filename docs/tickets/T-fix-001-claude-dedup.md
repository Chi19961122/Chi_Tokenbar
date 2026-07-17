# T-fix-001 — Claude log 掃描去重（resume/fork 副本不再重複計數）

`只實作本票行為與資料。可沿用現有樣式。禁止大規模 redesign。對照 PLAN flow。`

## 目標

`scan_claude` 對同一則 assistant 訊息只計一次。Claude Code 的 resume/fork 會把舊訊息複製進新的 `<uuid>.jsonl`，目前逐行照收 → Usage 全維度（daily/hourly/by_model/by_agent/by_kind/by_project/cost）偏高。做完後：重複訊息跨檔案只計一次，數字下降或不變，絕不上升。

## 範圍（只准動這些檔案）

* `src-tauri/src/analytics.rs`

## 規格

1. 在 `scan_claude`（analytics.rs:571 起）加全域去重：掃描開始時建 `HashSet<String>`（跨所有 project 目錄、所有檔案共用一個 set）。
2. 每行解析出 usage 後，取去重 key，優先序：頂層 `requestId` → `message.id` → 頂層 `uuid`。三者皆無 → **照常計數**（不丟資料）。
3. key 已在 set 內 → skip 該行；否則 insert 後照原流程 `acc.add(...)`。
4. key 只存在記憶體、絕不寫 log / stderr / 任何輸出（隱私鐵則）。
5. 檔案掃描順序不影響結果正確性（同 key 無論先後只計一次）——不要求特定順序。
6. 若 analytics 結果有 cache（階段 C 的快取機制），確認本改動後快取會自然失效或 bump 版本，避免舊的偏高數字被繼續供應。
7. 單元測試（仿現有 fake log 測試寫法）：
   - 兩個檔案含相同 `message.id` 的行 → 只計一次。
   - `requestId` 相同但 `message.id` 不同 → 只計一次（requestId 優先）。
   - 無任何 id 的行 → 全部照計。

## SPEC / PLAN 依據

* docs/PLAN.md 功能 Delta「scan_claude 加 message/request ID 去重」
* 參考：brrrn 的 dedup 設計（按 message/request ID 去重恢復會話副本）

## Out of scope（這張票不碰）

* Codex/OpenCode/Gemini 掃描（Codex 見 T-fix-002）
* providers/、engine/、額度與狀態機
* 前端任何檔案
* 定價（T-fix-003）

## Build / Verify

    前置:   PATH 加 %USERPROFILE%\.cargo\bin
    檢查:   cargo test --manifest-path src-tauri\Cargo.toml   ← 全綠含新測試
    前端:   npm test && npm run build                          ← 不應有變動仍須綠

驗收：

| 檢查 | 做什麼 | 期望看到 |
| --- | --- | --- |
| cargo test | 跑後端測試 | 新增 ≥3 個去重測試全過，既有 121+ 測試不壞 |
| 隱私掃描 | grep 改動處 | 無任何 id/token 進 println!/eprintln!/log |
