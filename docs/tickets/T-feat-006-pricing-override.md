# T-feat-006 — 可重載 pricing / alias 表(vendored 預設 + 本機 override)
status: todo

`只實作本票行為與資料。可沿用現有樣式。禁止大規模 redesign。對照 PLAN flow。`

> 來源:「TokenBar 優化建議(初版)」檢視結論(2026-07-21)+ Nanako0129/TokenBar(tokscale-core)pricing 管線借鏡。
> 依賴:T-fix-003(vendored 分項價目表)已完成,本票在其上加 override 層。

## 目標

改價不用重新編譯:維持 T-fix-003 的 vendored 價目表為預設與最終 fallback,新增使用者可編輯的本機 override 檔,啟動時與檔案變更時重載。**零外連原則不變**——本票不新增任何網路請求;遠端價目目錄(LiteLLM 等)明確 out of scope。

## 範圍(只准動這些檔案)

* `src-tauri/src/analytics.rs`(查價入口改走 lookup 鏈)
* `src-tauri/src/pricing.rs`(新檔:override 載入/驗證/快取)
* `src-tauri/src/lib.rs`(僅 wiring:把 override 傳入掃描)

## 規格

1. **Override 檔**:`%APPDATA%\Atoll\pricing.json`(與 settings.json 同目錄)。格式:

   ```json
   {
     "version": 1,
     "models": {
       "fable-5":  { "input": 10.0, "output": 50.0, "cache_read": 1.0, "cache_write_5m": 12.5, "cache_write_1h": 20.0 },
       "my-proxy-model": { "blended": 4.0 }
     }
   }
   ```

   單位一律 $/Mtok。條目允許兩種形態:分項五欄(缺欄用 input 比例推不出就取 blended 規則)或單一 `blended`。
2. **查價順序**:override 精確 model id(不分大小寫)→ override key 作 substring 匹配(同現有 family 風格)→ vendored `claude_rates()` 家族表 → 現有 `rate_per_mtok` blended fallback。順序寫成單一函式,測試逐層斷言。
3. **容錯載入(借鏡 tokscale custom.rs)**:單一條目壞(缺欄、非數字、負數)→ 跳過該條目、stderr 記 `[tb] pricing override: skipped <key>`,其餘條目照用;整檔 parse 失敗或 >1MB → 整包退回 vendored,不炸、不清空。**任何情況下成本欄位不得因 override 檔壞而變 0 或消失。**
4. **重載時機**:每輪掃描前 stat 檔案 mtime,變了才重讀(同 codex 快照的 mtime 思路);檔案不存在 = 純 vendored,零成本路徑。
5. 檔案不存在時**不要**自動建立範本檔(避免多數使用者目錄多一個沒用的檔);格式寫在本票與 CONFIG.md 即可。
6. 單元測試:
   - override 精確命中蓋過 vendored(金額斷言)。
   - 壞條目跳過、好條目生效(混合檔)。
   - 整檔壞 JSON → 全部退 vendored,結果與無檔一致。
   - substring 匹配優先序:精確 > override substring > vendored。

## Out of scope(這張票不碰)

* 任何網路請求(LiteLLM / OpenRouter 遠端目錄——若未來要做,另開票且必為 opt-in 設定)
* model alias 顯示層合併(byModel 分組)——只管計價,不管顯示歸組
* UI 設定介面(檔案手編即可)

## Build / Verify

    前置:   PATH 加 %USERPROFILE%\.cargo\bin
    檢查:   cargo test --manifest-path src-tauri\Cargo.toml
    前端:   npm test && npm run build

驗收:

| 檢查 | 做什麼 | 期望看到 |
| --- | --- | --- |
| cargo test | 跑後端測試 | 新增 ≥4 個 override 測試全過 |
| grep | 搜尋 reqwest/ureq 新呼叫於 diff | 零新增網路呼叫(ureq 僅既有 providers 用途) |
| 手動 | 放一個壞 JSON 到 %APPDATA%\Atoll\pricing.json 後刷新 | 成本照常顯示(vendored),stderr 有 skip 記錄 |
