# T-fix-003 — 分項計價：input/output/cache 分開累計 + vendored 價目表

`只實作本票行為與資料。可沿用現有樣式。禁止大規模 redesign。對照 PLAN flow。`

> 依賴：T-fix-001、T-fix-002 已完成（本票在正確的去重/增量資料上計價）。

## 目標

成本估算從 blended $/Mtok 粗估改為分項精算：Claude 訊息的 input / output / cache_creation / cache_read 分開累計、按 vendored 價目表分項計價。Claude Code 是 cache 重度型態，cache read 只有 input 價的 0.1×，blended 估算誤差極大。**不外連**（不抓 LiteLLM），價目表寫死在程式裡隨版本更新。

## 範圍（只准動這些檔案）

* `src-tauri/src/analytics.rs`
* `src/i18n.ts`（僅在需要微調 est. cost 文案時；非必要不動）

## 規格

1. 新增 vendored 價目表（$/Mtok，來源：Anthropic 官方 API 定價，快取日期 2026-06-24；cache write 取 5m TTL 費率）：

   | model 家族（substring 匹配，同現有 rate_per_mtok 風格） | input | output | cache_read | cache_write_5m | cache_write_1h |
   | --- | --- | --- | --- | --- | --- |
   | fable-5 / mythos-5 | 10.00 | 50.00 | 1.00 | 12.50 | 20.00 |
   | opus（4.5~4.8） | 5.00 | 25.00 | 0.50 | 6.25 | 10.00 |
   | sonnet（4.5/4.6/5） | 3.00 | 15.00 | 0.30 | 3.75 | 6.00 |
   | haiku（4.5） | 1.00 | 5.00 | 0.10 | 1.25 | 2.00 |

2. **Claude 掃描**：`message.usage` 分開讀 `input_tokens`、`output_tokens`、`cache_creation_input_tokens`、`cache_read_input_tokens`，cost = Σ(各類 tokens × 對應費率)。若 `cache_creation` 物件有 `ephemeral_5m_input_tokens` / `ephemeral_1h_input_tokens` 細項 → 分別用 5m/1h 費率；否則整包當 5m。**token 總量統計欄位（daily/by_model/…）維持現在的口徑不變**——本票只改 cost 計算與新增分類累計，不改變「total tokens」的定義（避免圖表全部跳動）。
3. **Codex**：`total_token_usage` 若有 `input_tokens` / `cached_input_tokens` / `output_tokens` 細項 → cost = (input−cached)×既有 blended input 估價 + cached×0.1×blended + output×blended（無官方精確表，維持估算但反映 cache 折扣）；無細項 → 沿用現行 blended。增量計價：對 T-fix-002 的每筆 delta 事件，細項也做前後差分（同 saturating 規則）。
4. 未知 model → 沿用現有 `rate_per_mtok` fallback（估算）。
5. UI 文案維持「est. cost / 估算成本」（誠實標註估算不變）；不加新 UI 元件。
6. 單元測試：
   - Claude 訊息 usage {input:1000, output:500, cache_read:100000, cache_creation:2000}，opus 費率 → 精確金額斷言（手算：0.005+0.0125+0.05+0.0125=0.08）。
   - cache_creation 帶 1h 細項 → 用 2× 費率。
   - Codex 有 cached 細項 → cache 折扣生效；無細項 → 舊 blended 結果不變。

## SPEC / PLAN 依據

* docs/PLAN.md 功能 Delta「分開累計 + vendored 價目表分項計價」＋ Non-goal「定價表不外連」
* analytics.rs §11（cost estimate 註解）

## Out of scope（這張票不碰）

* 不抓任何線上價目表、不加網路請求
* 不改戰報版面與 buildShareData 結構（金額數字自然變準即可）
* providers/、engine/

## Build / Verify

    前置:   PATH 加 %USERPROFILE%\.cargo\bin
    檢查:   cargo test --manifest-path src-tauri\Cargo.toml
    前端:   npm test && npm run build

驗收：

| 檢查 | 做什麼 | 期望看到 |
| --- | --- | --- |
| cargo test | 跑後端測試 | 新增 ≥3 個計價測試全過（含手算金額斷言） |
| grep | 搜尋 reqwest/http 於 diff | 零新增網路呼叫 |
