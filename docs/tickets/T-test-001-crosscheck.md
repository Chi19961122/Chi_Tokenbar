# T-test-001 — Rust↔TS 交叉驗證:共用 fixture 防兩端邏輯漂移
status: todo

`只實作本票行為與資料。純測試票,不改任何產品行為。對照 PLAN flow。`

> 來源:Nanako0129/TokenBar 借鏡評估(2026-07-21)。對手用一份 fixture JSON(provider-quota-pace-v3.json)餵 Rust 與 Swift 兩端,逐欄位 diff 掛 CI,防跨語言縫的計算漂移。
> Atoll 的同型縫:後端算 util%/pace/runway(burnrate.rs、engine.rs),前端算顯示決策與格式化(island.ts pickIslandLimit/islandText、fmtResetRel)。兩端各有測試,但**沒有共用案例**,同一情境兩邊答案可以悄悄分岔。

## 目標

一份共用 fixture,cargo test 與 vitest 各自載入、各算各的、斷言到同一組期望值。任何一端邏輯改動而未同步另一端時,至少一邊的測試變紅。

## 範圍(只准動這些檔案)

* `fixtures/crosscheck-v1.json`(新檔,repo 根層新目錄;兩端都讀得到的中立位置)
* `src-tauri/src/`(僅新增測試模組,如 `crosscheck_tests.rs`,`#[cfg(test)]`)
* `src/crosscheck.test.ts`(新測試檔)
* `vitest.config.ts` / Cargo 不動(fixture 用相對路徑 include/讀檔)

## 規格

1. **Fixture 結構**:`{ "version": 1, "cases": [ { "name", "input": {…}, "expect": {…} } ] }`。input 描述一個 limit 快照情境(util%、window 長度、resets_at 相對秒數、歷史樣本序列、locale);expect 只放**兩端都算得出的欄位**,分兩節:
   - `expect.backend`:pace / runway 秒數(容差 ±1s)、status 分級(safe/near/locked)。
   - `expect.frontend`:pickIslandLimit 選誰、islandText 逐字、fmtResetRel 逐字(zh-TW 與 en 各一)。
2. **案例集(≥12 案)**,必含:
   - 正常配速、in-deficit、剛好門檻 75%/90% 邊界。
   - locked(util=100)與 resets 倒數跨日/跨週(fmtResetRel 的日字表邊界)。
   - 釘選 limit 無資料 →「—」(絕不退 auto,v0.3.0 鐵則)。
   - stale / degraded 樣本(不進 pace,島嶼淡出行為)。
   - 樣本不足 → 投影不足態(UX Spec §7)。
3. **時間全部相對化**(fixture 內只有「距 now 的秒數」),兩端測試各自用固定的假 now 換算——**fixture 禁止絕對時間戳**,否則案例會過期。
4. 兩端測試各自實作薄 loader;斷言訊息帶 case name,一眼看出哪個案例分岔。
5. 期望值來源:以**現行行為**為準灌入(本票不改變任何計算);灌值時若發現兩端已經分岔,**不要在本票修**——記進 docs/FEEDBACK.md 開 F-xxx,fixture 先記真實現狀並註明。

## Out of scope(這張票不碰)

* 不改 burnrate / island / fmt 任何實作(發現分岔走 FEEDBACK 流程)
* 不做 CI workflow(本地 cargo test + npm test 為準;CI 另議)
* T-feat-007 落地後的 historical 案例(它完成後在 fixture 加 v2 節,另張票)

## Build / Verify

    前置:   PATH 加 %USERPROFILE%\.cargo\bin
    檢查:   cargo test --manifest-path src-tauri\Cargo.toml
    前端:   npm test

驗收:

| 檢查 | 做什麼 | 期望看到 |
| --- | --- | --- |
| cargo test | 後端載 fixture 跑 | ≥12 案全過 |
| npm test | 前端載同一 fixture 跑 | 同案例集全過 |
| 手動 | 故意改一個 expect 值 | 兩端至少一邊紅(證明真的在讀同一份檔) |
