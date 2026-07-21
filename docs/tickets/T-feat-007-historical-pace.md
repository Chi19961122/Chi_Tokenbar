# T-feat-007 — Historical pace:額度歷史落地 + 歷史配速投影
status: todo

`只實作本票行為與資料。可沿用現有樣式。禁止大規模 redesign。對照 PLAN flow。`

> 來源:Nanako0129/TokenBar 借鏡評估(2026-07-21)。對手在 ≥2 個完整額度週期後改用歷史配速曲線(`expectedUsedPercent` / `etaSeconds` / `runOutProbability`),比純線性投影準。
> 現況:`Engine::history`(engine.rs:31)只在記憶體,重啟即空;runway 由 burnrate.rs 線性投影。週視窗要 2 個完整週期 = 至少兩週資料,**沒有落地就永遠學不起來**,故本票分兩步,落地是前置。

## 目標

1. 額度樣本落地磁碟,跨重啟保留。
2. 累積 ≥2 個完整週期後,配速/runway 從線性投影升級為歷史配速(同視窗時間點的歷史用量曲線);不足門檻時行為與現在完全一致。

## 範圍(只准動這些檔案)

* `src-tauri/src/engine.rs`(history 落地/載回)
* `src-tauri/src/burnrate.rs`(歷史模式計算,新純函式)
* `src-tauri/src/lib.rs`(wiring + 快照欄位)
* `src/`(pace 行顯示,最小改動)+ `src/i18n.ts`(新文案 key)

## 規格

### A. 歷史落地(前置,獨立可驗)

1. 檔案:`%APPDATA%\Atoll\quota-history.json`。內容:per limit id 的 `(ts, util%)` 序列 + `resets_at` 變化點。
2. 寫入:每輪引擎 push 後 append(可整檔重寫,量小);**原子寫**(temp + rename,同 credentials 寫回的既有做法)。degraded/stale 樣本不記——沿用 engine 現行不變量(engine.rs 測試 `degraded_limits_are_not_recorded_in_history` 的精神,落地層同樣適用)。
3. 保留上限:每 limit 最多 5 個完整週期或 35 天(先到為準),啟動載入時修剪;檔案壞/缺 → 當空檔重新累積,不炸。
4. 隱私:檔內只有時間戳與 util%,無 token、無專案名、無任何 log 內容。

### B. 完整週期切分

5. 週期邊界判定(純函式,可測):`resets_at` 前進到新值,或 util 由高處(>20%)跌回低處(<5%)視為 reset。一段從邊界到邊界、且首尾樣本間隔 ≥ 視窗長度的 80% 才算「完整週期」(樣本稀疏的殘段不算)。
6. 每個完整週期正規化為「視窗進度 0..1 → util%」曲線(固定桶數,如 48 桶,線性內插)。

### C. 歷史配速輸出

7. `historical_pace(limit_id, 視窗進度 t)`:取各完整週期在 t 的 util 中位數 → `expected_util`;runway:以歷史曲線由目前 util 外插至 100% 的時間;`run_out_probability` = 歷史週期中「撞到 100 / 進 locked」的比例(0..1)。
8. 啟用門檻:該 limit ≥2 個完整週期 → historical;否則線性(現行 burnrate 路徑一字不動)。**不可倒退**:歷史模式輸出缺料(如 t 超出樣本)時退回線性,不得出現空白。
9. 快照新欄位(serde default,舊前端不炸):`pace_basis: "linear" | "historical"`、`run_out_probability: Option<f64>`。
10. UI:配速行沿用現有文案;basis=historical 時行尾加小標 `hist`(tooltip 全文),`run_out_probability ≥ 0.5` 時 runway 行帶琥珀(沿用既有 in-deficit 色,不加新色)。文案維持估算的誠實標註。UX Spec §7「投影不足」態行為不變。

### D. 測試

- 週期切分:合成 3 個週期樣本(含一個殘段)→ 恰 2 個完整週期。
- 歷史 expected:兩條已知曲線 → t=0.5 中位數斷言。
- 門檻:1 個週期 → basis=linear 且輸出與現行 burnrate 逐位一致(回歸鎖)。
- 落地 round-trip:寫 → 載 → 修剪(35 天界)斷言。
- 壞檔:亂寫 JSON → 空歷史、不 panic。

## Out of scope(這張票不碰)

* 不動 providers/(資料來源不變)
* 不做 UI 新圖表(歷史曲線視覺化另議)
* Codex 本機快照的 stale 語意(CLAUDE.md 鐵則)不變
* 不改通知門檻邏輯

## Build / Verify

    前置:   PATH 加 %USERPROFILE%\.cargo\bin
    檢查:   cargo test --manifest-path src-tauri\Cargo.toml
    前端:   npm test && npm run build

驗收:

| 檢查 | 做什麼 | 期望看到 |
| --- | --- | --- |
| cargo test | 後端測試 | 新增 ≥5 測試全過;既有 burnrate 測試零改動全過 |
| 手動 | 首次啟動(無歷史檔) | 行為與現版完全相同,%APPDATA%\Atoll\ 出現 quota-history.json |
| 手動 | 刪掉/寫壞歷史檔後啟動 | 不炸,重新累積 |
| grep | quota-history.json 內容 | 只有 ts/util/resets,無 token 數與專案資訊 |
