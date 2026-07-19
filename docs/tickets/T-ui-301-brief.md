# T-ui-301 [arch/frontend] 分析頁 2 鏡頭 IA + 控件 + 邏輯 + accounts 遷移 + i18n + 測試

先讀 `docs/RUNBOOK.md` 與 `AGENTS.md` 硬邊界。真相：`docs/DESIGN-SPEC-analytics-2lens.md`（本票規格）+ 選定比稿 `design/previews/analytics-C1-detail.html`（DOM/class 契約與版面意圖）。行為語意仍歸 `Ai_Assistant/TokenBar UX Spec v3.md`。

## 目標
把分析頁從 4 個 subtab（overview/hourly/share/stats）改成**兩鏡頭上下堆疊、一路捲**的單一面：Trends 在上、Breakdown 在下，**移除 subtab 切換列**，toggle 內嵌每鏡頭。純前端（`Analytics` payload 已含全欄位，**不動後端/Tauri 指令**）。

## 範圍（硬白名單，只准動這些檔）
- `src/main.ts`、`src/analytics.ts`、`src/analytics.test.ts`、`src/i18n.ts`、`src/settings-controls.ts`（僅在需要復用 `segmentHtml` 時）、`index.html`（僅 `#subtabs`/`#toggles` 容器相關；分析內容 DOM 由 JS 產）。
- **不碰 `src/styles.css`**（視覺歸 T-ui-302）。你只負責 emit 正確的 class 名（見下方契約），CSS 由 302 實作。
- 不動 island、Limits 面板、戰報、BottomBar、後端 `src-tauri/**`。

## 要做

### 1. 控件/狀態模型（SPEC §1.1）
- `src/main.ts:61-84` `ui` 物件：移除 `subtab` 的「四選一切換」語意；新增 `ui.granularity: "daily"|"hourly"`（預設 daily）。保留 `ui.metric`("tokens"|"price")、`ui.group`("model"|"agent")、`ui.range`("today"|"week"|"month")。
- **移除 subtab 切換列**：`renderSubtabs()`（main.ts:103-113）與 `#subtabs`（index.html:45）廢除或改為不再渲染鏡頭切換鈕（兩鏡頭都常駐）。
- `renderToggles()`（main.ts:115-146）改為**每鏡頭內嵌**：Trends 內嵌 granularity(Daily|Hourly)+metric(Tokens|Cost)；Breakdown 內嵌 group(By model|By agent)+metric；range(Today|Week|Month) 常駐（放頂部或 Trends 頂，擇最低驚訝）。沿用 `segmentHtml()`。
- click handler（main.ts:1032-1050）：subtab 切換邏輯移除；granularity/metric/group/range 變更 → 重繪整個分析面（兩鏡頭）。保留既有 stale-while-revalidate/捲動保留行為。

### 2. `renderAnalytics` 重寫（analytics.ts:484-528）
- `SubTab` enum（analytics.ts:23-26）改為兩鏡頭模型（可改成「一次渲染兩鏡頭」而非 subtab 分支；enum 若保留則為 `trends|breakdown` 供內部分段用）。
- **一次輸出兩鏡頭 DOM**，用比稿 `analytics-C1-detail.html` 的 class 契約：
  `.feature`（每鏡頭外層，`.feature + .feature` 為第二鏡頭）、`.cap`（Lens 標）、`.kick`（serif kicker）、`.toggles/.seg/.seg button.on`、`.hero/.hero .eyebrow/.hero .fig/.hero .fig .u/.hero .sub`、`.support/.support .lbl`、`.chart/.chart .bar/.bar.strong/.bar.today`、`.xaxis`、`.rows/.row/.row.top/.row .meta/.row .nm/.row .vl/.track/.track i`、`.donutsec/.donutsec svg/.legend/.legend i/.legend b`、`.comp/.compbar/.compbar i/.complegend`、`.footnote`。
- **Trends 鏡頭**：hero=本期 total tokens（`.fig` + 單位 `.u`「M tokens」）+ metric-aware；serif kick；granularity=daily → 既有 `stackedDaily`（吃 range 窗）+ 月份時 `heatmap`；granularity=hourly → 既有 `hourly`（24h）。footnote 收 Peak day / Busiest hours / sessionsThisWeek / tokPerMin（來自舊 statsView 的 records/sessions/tokPerMin）。
- **Breakdown 鏡頭**：hero=領先 model/agent 名 + 佔比；serif kick；`shareBars(group)` 排行（#1 = `.row.top`）；`donut(byKind)` 活動類型（**全灰階**，見 SPEC §2，不上洋紅）；`projectBars(byProject)`；composition 分段條（`breakdown` input/cached/output/reasoning，灰階 `.compbar`）。
- **每鏡頭洋紅 ≤1**：Trends=today 長條；Breakdown=`.row.top` 底線。donut/composition/projects/hero 數字皆非洋紅。
- 保留既有純函式（`sharePct/shareLabel/axisTicks/dailyXTicks/monthStartNote/heatCells/kindColor/kindLabel`）與 chart tooltip 佈線（`wireChartTip`）。空/stale 態沿用現行慣例（無資料的區段不渲染）。

### 3. stats 拆解
- 舊 `statsView`（analytics.ts:451-482）拆掉：composition→Breakdown；records(maxDay/maxHour/streak)→Trends footnote（**與頂部 Peak/Streak 去重**，不要兩處都出現）；sessionsThisWeek/tokPerMin→Trends footnote；**accounts→設定頁**（見 4）。

### 4. accounts 遷移設定頁
- `main.ts renderSettings()`（560-679）新增一個 `.sgroup`「Accounts / 帳號」，沿用 `.sgroup > .lsec-head + .srow` 結構，每帳號一列唯讀顯示 `client · account · plan`。
- 資料：擇 renderSettings 現有資料流最低耦合方式取得 accounts（若無 analytics payload 可用，沿用已抓的 analytics 或既有帳號來源）。
- 分析頁不再渲染 accounts。

### 5. i18n（i18n.ts，en:81-126 / zh:298-340）
- 新增：`subtab.trends`/`subtab.breakdown`（鏡頭 caption）、`toggle.daily`/`toggle.hourly`、accounts 區標（`settings.accounts` 或 `analytics.accounts`）。
- 移除不再用：`subtab.overview`/`subtab.stats`（`subtab.share`/`subtab.hourly` 若語意併入 granularity 也一併清）。保留 `toggle.*`、`analytics.kind*`、composition 段標。en+zh 同步。

### 6. 測試（analytics.test.ts）
- 20+ 處 `renderAnalytics(box, a, {subtab:...})` 呼叫改用新模型（兩鏡頭渲染或 `trends/breakdown` + granularity/group）。純函式測試（sharePct/shareLabel/axisTicks/dailyXTicks/monthStartNote/heatCells）**保留不動**。必要時補「兩鏡頭皆渲染」「accounts 不在分析輸出」的斷言。

## Build / Verify（commit 前必過）
- `npx tsc --noEmit` 乾淨。
- `npm test`（vitest）全綠。
- 手動語意自檢：兩鏡頭皆渲染於同一捲動面、無 subtab 切換鈕殘留；granularity 切 Daily/Hourly、metric 切 Tokens/Cost、group 切 model/agent、range 切 today/week/month 皆即時且無殘影；月熱力圖僅 month；accounts 出現在設定頁且分析頁無；空/stale 態不崩。

## 模式宣告
一般實作（需局部判斷），不動後端、不動 styles.css、不動 island/戰報。違反範圍白名單＝工單作廢重來。
