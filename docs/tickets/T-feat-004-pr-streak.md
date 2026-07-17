# T-feat-004 — PR 個人紀錄與連勝（max day / max hour / streak / PR NOW）

`只實作本票行為與資料。可沿用現有樣式。禁止大規模 redesign。對照 PLAN flow。`

> 依賴：T-fix-001、T-fix-002（紀錄要建立在去重後的正確數字上）。

## 目標

Usage 的 stats 子頁多一組健身房式個人紀錄：單日最高、單小時最高、連續活躍天數，以及「PR NOW」即時徽章（本小時已破歷史單時紀錄）。資料全部來自既有掃描，不加資料源。

## 範圍（只准動這些檔案）

* `src-tauri/src/analytics.rs`（聚合 + 新欄位）
* `src/types.ts`（型別）
* `src/analytics.ts`（stats 子頁 tiles + PR NOW badge）
* `src/i18n.ts`（en + zh-TW 文案 key）
* `src/analytics.test.ts`、`src/mock.ts`（測試與 mock 情境）
* `src/styles.css`（badge 樣式，用既有 token）
* `src/share.ts`（buildShareData 加欄位；2026-07-17 補列——初版漏列，Codex 正確拒動票外檔）
* `src/share.test.ts`（若存在，對應測試）

## 規格

1. 後端 `Analytics` 新增 `records` 欄位：
   - `max_day: { date: String, tokens: u64 }`（範圍內最高單日）
   - `max_hour: { date: String, hour: u8, tokens: u64 }`（範圍內最高「單日單時」——注意既有 `hourly[24]` 是跨日彙總不能用；掃描時另 accumulate `HashMap<(date, hour), u64>`，容量 ≤ 31×24，可接受）
   - `streak_days: u32`（截至今天（本地日）連續有活動的天數；今天尚無活動則從昨天起算，昨天也無 → 0）
   - `pr_now: bool`（本小時 (今天,當前時) 的量 > 排除本小時後的歷史 max_hour；範圍資料不足 30 天時照樣算，不假裝更長歷史）
2. 空資料 → `records` 各值為 0/空字串，前端整組 section 不渲染（同 byKind/byProject 慣例）。
3. 前端 stats 子頁新增三個 tile（沿用既有 `.tile` 樣式）：Max day（值＋日期）、Max hour（值＋日期+時）、Streak（N days）。`pr_now == true` 時在 Usage header 或 stats 區顯示「PR NOW」badge（樣式用既有 accent token，不發明新色）。
4. i18n：`analytics.maxDay`、`analytics.maxHour`、`analytics.streak`、`analytics.prNow` en/zh-TW 都補（`satisfies` 檢查會抓漏）。
5. 戰報：`buildShareData` 增加 `streakDays` 與 `maxDayTokens` 兩個數字欄位（**不含日期以外的任何識別資訊、絕不含專案名**，§0 照舊）；六模板**版面不動**，僅 stats 型模板若有現成數字欄位槽可自然帶入才帶，沒有就先不顯示（顯示層留給 Wave2 視覺票）。
6. 測試：
   - Rust：streak 邊界（今天無活動回退昨天起算；中斷歸零）、max_hour 取 (date,hour) 而非跨日彙總、pr_now 排除本小時比較。
   - 前端：records 空 → 不渲染；有值 → tiles 文案正確（仿既有測試風格）。
   - mock.ts 至少一個情境帶 records 值供 preview 驗收。

## SPEC / PLAN 依據

* docs/PLAN.md 功能 Delta「PR 個人紀錄（max hour/max day/連勝天數 + PR NOW badge）」
* 三樣態計畫 §0：專案名絕不進戰報

## Out of scope（這張票不碰）

* 不改六戰報模板版面、不加新模板
* 不做通知/音效
* providers/、engine/

## Build / Verify

    前置:   PATH 加 %USERPROFILE%\.cargo\bin
    檢查:   cargo test --manifest-path src-tauri\Cargo.toml && npm test && npm run build

驗收：

| 開哪個 URL | 做什麼 | 期望看到 |
| --- | --- | --- |
| http://localhost:1420（`npm run dev`，mock 模式） | 面板 → Usage → stats 子頁 | 三個紀錄 tile 有值；devbar 切 empty 情境 → 紀錄區整組消失 |
| 同上 | mock 帶 pr_now=true 的情境 | PR NOW badge 顯示 |


### Attempt 1

    非實作失敗：票面範圍欄漏列 src/share.ts，與規格第 5 點衝突，
    Codex 停下請求授權、零改動退出（正確行為）。
    修正：範圍補列 src/share.ts 與其測試檔後重跑。
