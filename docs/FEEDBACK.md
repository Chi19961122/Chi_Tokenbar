# FEEDBACK

> 試用時發現的問題寫在這裡，一行一條，格式：`F-001 [func|visual] 現象（在哪頁、做了什麼、看到什麼）`。
> 這個檔只是輸入，不是真相來源 —— 每條會先被翻成 docs/tickets/T-9xx 修正票（附改法與驗收）才會動工。
> `[func]`（按鈕壞、錯 route、資料不出）永遠排在 `[visual]` 前面處理。

## 2026-07-18 真機驗收回饋（v0.6 輪）

**本輪已修（5 commit）**

- F-01 [func] Claude Code 額度卡不顯示。**根因**：Anthropic 用量端點 `/api/oauth/usage` 對重度帳號回持續性 HTTP 429（TokenBar 與 Claude Code 共用同一 OAuth client_id/限流桶；非本專案 bug、非 v0.6 改動所致）。原 `poll()` 一失敗就把快取覆蓋成 util=0/SourceFailed，丟掉上次成功值 → 卡片變「—／Unavailable」。**修**：anthropic.rs 加 `last_good` + `reconcile()`，暫時性失敗（429/5xx/連線/json/schema）保留上次值標 Stale、只有終端認證失敗才 blank+relogin。→ `8defba6`。（429 於當日稍後自行解除，Claude 額度已恢復 Normal。）
- F-02a [visual] 灰色字讀不清。次級文字 token `--faint #A1A1AA`（≈2.5:1，AA 不過）/`--muted #71717A`（≈4.6:1）在亮底太淡 → 加深為 `--muted #5D5D65`(≈6.3:1)、`--faint #6C6C75`(≈4.9:1)，三層皆過 WCAG AA。→ `3b7d099`。
- F-02b [visual] 多餘文案。移除 section header 的 serif 裝飾副標（「What's left in the tank」/「How the work adds up」）。→ `f0aad9a`。
- F-03 [func] 跑很慢整個程式很卡。**根因**：1s tick 每秒 `renderCards()` 全量重建整個 Limits 面板 innerHTML（+island），讓 gauge 700ms transition 永遠重啟、serif 每秒重排版。**修**：signature guard，island/cards 只在可見輸出真的會變時重建（snapshot/UI/分鐘桶；reset 前最後一分鐘降秒桶），重建次數降到約 1/60；renderRefresh 維持每秒。→ `49e71c5`。
- F-04 [visual] 移除 3D 熱力圖。連 three.js 相依一起清除，保留 2D。→ `c73eeab`。

**下輪待辦（較大工程／需先決策，尚未動工）**

- F-02c [visual] 分析頁出現 scrollbar、下方仍有空白。**非一行修正**：`#analytics` 固定 300px + 內捲是刻意 anti-jank（切 subtab 不 resize 視窗）；使用者大螢幕上顯得過小。正解要**解耦「視窗高 vs analytics 高」+ 螢幕感知**，與 anti-jank 相衝、需真實資料量測。→ 併 F-02e 一起做（同屬版面/視窗架構）。
- F-02d [visual] 設定下拉選單要重新設計（互動重構）。
- F-02e [visual] 設定改整頁模式，不要在現有頁面往上疊加（架構改動：contextmenu overlay → 整頁 route/shell）。
- F-02f [func] 亮暗切換預設跟隨系統，且**亮色與暗色兩種模式都要**（使用者 2026-07-18 確認）。推翻方向 D「light-only」定案，需補整套 `.dark` token + 六戰報暗色驗證。

**額外觀察（待使用者確認）**

- GaugeCard 每格「X% left」出現兩次（60px hero 大字 + 下方細字重複同值），屬多餘。未動（是剛出的 T-ui-201 卡片設計一環），可依使用者意見於下輪一併收斂。
