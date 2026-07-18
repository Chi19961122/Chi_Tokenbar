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

## 2026-07-18 二次驗收回饋（v0.6 輪）

**本輪已修（3 commit，verifier 全數 CONFIRMED）**

- F-05 [func] 分析頁面效能卡頓還是很差。**根因**：`get_analytics` 是同步 Tauri command（Tauri 2 同步 command 跑在主執行緒），而 `compute_with` 每次呼叫都重掃解析範圍內全部 session log（本機 ~/.claude/projects 173MB + ~/.codex/sessions 228MB；mtime 剪枝擋不住「今天被碰過的大檔整檔重掃」）。掃描期間整個 app 凍結——拖曳、island、所有 IPC——且每 60s（island tok/min 更新）+ 每次分析頁互動都觸發。**修**：command 改 async + `spawn_blocking`，掃描移到 blocking worker，payload 不變。→ `119ad06`。（掃描本身的 CPU 成本仍在，增量掃描留下輪。）
- F-06 [visual] 從分析頁切回限額頁會跑版。**根因**：`applyCompact()` 換分頁時先 `fitWindow()` 量高度、卡片變體（完整列表 vs 摘要行）卻要等下一個 1s tick 才重繪 → 視窗高度用舊內容量的，完整列表被塞進摘要行大小的視窗，直到下次切換模式都不自癒。**修**：量測前先 `renderCards()`。→ `3fb3379`。
- F-07 [visual] 每時長條圖 X 軸只有 0h 和 23h，中間要自己數。**修**：補 6h/12h/18h 標籤（置中對齊各自長條）。→ `8d3b4fd`。

**答覆（非 bug）**

- 「ICON 那行右側的 X% left 是顯示最緊急的？」——是。狀態膠囊取**所有視窗中剩餘 % 最小**者顯示（panel.ts `statusPill`：known limits 取 `min(pctLeft)`），顏色跟著整體最差狀態（locked > near > stale > safe）。（該行已於三次驗收依使用者要求整行移除，見 F-10。）

## 2026-07-18 三次驗收回饋（v0.6 輪）

**本輪已修（3 commit，verifier 全數 CONFIRMED）**

- F-08 [visual] GaugeCard「X% left」大字下方重複同值細字（使用者核准移除）。detail 行只留 Unavailable／estimate／stale 徽章。→ `0d8f38e`。
- F-09 [func] 總覽切去其他頁籤會卡、載入很慢。**根因**：快取 key 含 snapshot `updated_at`，每輪新 snapshot 都讓 key 失效 → 頁籤一點就 await 數秒的後端掃描，舊畫面死在原地無回饋。**修**：stale-while-revalidate——同 range/filter 只是世代較舊 → 立刻畫舊資料、背景刷新；全冷 → 立刻出 skeleton；同 key 掃描去重；落地重繪防 range／filter／report 超越；重繪保留捲動位置。→ `0e9d8a7`。（掃描本身增量化仍留下輪。）
- F-10 [visual] ϟ 圖示 + 右側 min % left 膠囊整行移除（使用者要求；與量測列表一眼可見的資訊重複）。相關 CSS 全清，首個 section head 去頂線避免與 header 髮絲線疊雙。→ `d2a4662`。
