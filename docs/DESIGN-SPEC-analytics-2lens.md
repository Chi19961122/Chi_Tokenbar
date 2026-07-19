# DESIGN-SPEC — 分析頁 2 鏡頭改版（C1 Magazine · detailed）

> 定稿 2026-07-19：使用者選定方向 **C1 Magazine + 補細項（option B）**。
> 像素/數值真相 = `design/previews/analytics-C1-detail.html`（選定 HTML 比稿）。跟任何截圖/舊 SPEC 打架，以本檔 + 該比稿為準。行為/資料語意仍歸 `Ai_Assistant/TokenBar UX Spec v3.md`。
> 本檔只管**分析頁**；不動 island、Limits 面板、戰報版面、設定頁既有控件（僅新增 accounts 區塊）。Design token 沿用 `src/styles.css` :root（Atoll 亮暗雙主題）—— 本改版**不新增/不覆寫** token，只重排版與收斂 IA。

## 0. 目標與非目標

**目標**：把現行 4 個 subtab（overview / hourly / share / stats）收斂成 **2 個鏡頭**，並把 accounts 移出分析頁。

**非目標（硬邊界）**：
- 不動 island、Limits、戰報版面結構、BottomBar。
- 不改後端資料 payload / Tauri 指令（純前端重排；`Analytics` 物件既有欄位全沿用）。
- 不新增 design token（色/字/間距全走既有 :root，亮暗雙主題都要對）。
- 不動 SEAL_MARK / 品牌記號（那是別的工單）。
- 圖表維持灰階 5 級 + 洋紅僅 today/#1；**每鏡頭洋紅最多一處**。

## 1. 資訊架構：4 subtab → 2 鏡頭

| 舊 subtab | 去向 |
| --- | --- |
| `overview`（日長條 + 月熱力圖） | → **鏡頭 1 Trends**（日粒度） |
| `hourly`（24h 長條） | → **鏡頭 1 Trends**（時粒度，用 granularity toggle 切換） |
| `share`（model/agent 排行 + kind donut + project bars） | → **鏡頭 2 Breakdown** |
| `stats`（組成 breakdown + records + sessions/tok-min + **accounts**） | 組成→**Breakdown**；records→頂部摘要（去除與 Peak/Streak 重複）；sessions/tok-min→**Trends** footnote；**accounts→設定頁** |

**鏡頭 1 · Trends（「何時」）**：
- 控件：granularity toggle `Daily | Hourly`；metric toggle `Tokens | Cost`。
- 內容：hero 巨型數字（本期 total tokens）+ 一句 serif kicker；主圖（Daily=30d 堆疊長條 + 月熱力圖／Hourly=24h 長條）；footnote 收 Peak day / Busiest hours / sessions this week / tok per min。

**鏡頭 2 · Breakdown（「去哪」）**：
- 控件：group toggle `By model | By agent`。
- 內容：hero（領先 model/agent 名 + 佔比）+ serif kicker；排行長條（#1 洋紅底線）；活動類型 **donut**（kind，全灰階 + 圖例%）；**By project** 長條（灰階）；**Token composition** 分段條（input/cached/output/reasoning，灰階）。

**去重**：Streak / Peak 只出現在頂部摘要一次（舊 stats 的 records maxDay/maxHour/streak 與頂部 tiles 重複 → 收斂）。

## 2. 版面與字排（C1 Magazine，值抽自 C1-detail 比稿）

- 畫布：380px 寬固定；高度隨內容（現行 `#analytics` flex:1 內捲不變）。
- 鏡頭區隔：`.feature + .feature` 間 52px + 頂 42px padding + 1px `--line` 髮絲線。
- **caption 標**：9.5px / 600 / uppercase / +.14em / faint（`Lens 1 · Trends`）。
- **serif kicker**：Playfair Display italic 15px / muted / line-height 1.55 / max-width 300px。
- **hero 數字**：Geist 600 / letter-spacing -.045em / line-height .9；Trends total = 70px（單位「M tokens」29px/500/g3）；Breakdown 名稱 = 56px。tabular-nums。
- **toggle（segmented，SPEC 編輯部黑白）**：膠囊底 `--g1`、圓角 999px、padding 3px；鈕 10.5px/600/muted/padding 5px 12px；**選中 = `--text` 底 + `--bg` 字**（反白），非發明新態。
- **排行列**：名 13px/500、值 muted/600/tabular；軌 2px `--g1`、填 `--g3`；`.row.top` 填 `--accent`（**這是 Breakdown 唯一洋紅**）。
- **donut**：SVG viewBox 56、環寬 7、段間 gap 2、半徑 20；顯示 92px；段色走灰階 ramp（edit `--g5`→other `--g1`，中間穿插 `--dim`），**不上洋紅**；底環軌 `--g1`；右側圖例（8px 方點 + label + 右對齊 % 粗體）。
- **composition 分段條**：高 8px、圓角 2px、四段灰階（input g5 / cached g4 / output g3 / reasoning g2）；下方 wrap 圖例（8px 方點 + label + % 粗體）。
- **chart 長條**：日長條 `--g2`、>60% 強柱 `--g4`、today `--accent`（**Trends 唯一洋紅**）；柱距 2px、圓角 1px。
- **footnote**：11px / faint / line-height 1.6；`b` 用 `--dim`/600。

> 亮暗雙主題：所有值走既有 token，暗色自動跟隨（`:root.dark` 已定義 `--g*`/`--accent #F472B6`/文字階）。verifier 需雙主題各驗一次。

### 1.1 控件模型（收斂後）

| 控件 | 值 | 範圍 | 落點 |
| --- | --- | --- | --- |
| **range** | Today \| Week \| Month | 全域資料視窗（既有行為，兩鏡頭都受影響，因 payload 依 range 抓） | 頂部常駐 |
| **metric** | Tokens \| Cost | 跨鏡頭（Trends 圖 + Breakdown 排行都可切） | 兩鏡頭常駐 |
| **granularity** | Daily \| Hourly | 僅 Trends（Daily 走 range 窗＋月熱力圖；Hourly=24h 固定） | Trends |
| **group** | By model \| By agent | 僅 Breakdown | Breakdown |

> `ui.subtab` enum 由 `overview/hourly/share/stats` → **`trends/breakdown`**；新增 `ui.granularity`（Daily/Hourly，取代舊 overview↔hourly 的分頁切換）。`ui.metric`/`ui.group`/`ui.range` 沿用。

## 3. accounts 遷移（→ 設定頁）

- 現 `analytics.ts statsView`（L458-460）的 `a.accounts` → `<div class="acct"><b>{client}</b> · {account} · {plan}</div>` 清單，移到**設定整頁**（`main.ts renderSettings()` L560-679）**新增一個 `.sgroup`**「Accounts / 帳號」，沿用既有 `.sgroup > .lsec-head + .srow` 結構（帳號列用唯讀 `.srow` 呈現 client·account·plan）。
- 資料來源：settings 頁若無 `Analytics` 物件，需把 `accounts` 餵進設定 render（最省：沿用已抓的 analytics payload，或另拉帳號來源——**實作者依 renderSettings 現有資料流擇低耦合者**）。
- 分析頁 `statsView` 整個併入 Breakdown（見 §1 對照）後不再單獨存在；accounts 不在分析頁渲染。
- 新增 i18n key `settings.accounts`（或 `analytics.accounts` 沿用）en+zh。

## 4. i18n

- 新增/改動 label：鏡頭 caption（Trends/Breakdown）、granularity（Daily/Hourly）、group（By model/By agent）、metric（Tokens/Cost）、composition 段標（input/cached/output/reasoning，若尚無）、accounts 區標。en + zh 都要。
- 移除：舊 subtab 專屬 label（overview/stats 等，若不再用）——**待 scout 列出既有 key 後精確增刪**。

## 5. 實作落點（scout 已定，開票依此）

- **控件/狀態**：`main.ts:61-84 ui` 物件（加 `granularity`）；`renderSubtabs()` L103-113（2 鏡頭）；`renderToggles()` L115-146（依鏡頭條件顯示 granularity/group，range+metric 常駐）；`segmentHtml()` `settings-controls.ts:6-14` 沿用；click handler L1032-1050；`coerceSubTab()` 更新；`renderAnalyticsInto()` L237-246 傳 opts。
- **enum/邏輯**：`analytics.ts:23-26 SubTab` → `trends|breakdown`；`renderAnalytics()` L484-528 改兩分支（trends：granularity daily→`stackedDaily`+月`heatmap`／hourly→`hourly`；breakdown：`shareBars(group)`+`donut`+`projectBars`+composition segmented）。舊 `statsView` 拆解：composition→breakdown、records/sessions/tokPerMin→trends hero/footnote、accounts→設定。
- **資料**：單一完整 payload（`main.ts:97,328` `fetchAnalytics`→`analyticsCache`；`types.ts:152-185 Analytics` 含全欄位）→ **純前端合併，不動後端/Tauri 指令**。
- **設定頁**：`main.ts renderSettings()` L560-679，`.sgroup` 新增 Accounts 組。
- **測試**：`analytics.test.ts` 20+ 處 `renderAnalytics()` 寫死舊 subtab 字串 → 全數改 `trends/breakdown` + 對應 granularity/group；純函式測試（`sharePct`/`shareLabel`/`axisTicks`/`dailyXTicks`/`monthStartNote`/`heatCells`）**保留不動**。
- **i18n**：新增 `subtab.trends`/`subtab.breakdown`、`toggle.daily`/`toggle.hourly`、accounts 標；移除不再用的 `subtab.overview`/`subtab.stats`（`toggle.*`/`analytics.kind*`/composition 標沿用）。en+zh 同步。既有 key：`i18n.ts:81-126`(en)/`298-340`(zh)。

## 自檢

- [x] IA：4→2 鏡頭對照表 + 控件模型
- [x] 每鏡頭洋紅 ≤1（Trends=today、Breakdown=#1 row）
- [x] 版面/字排值抽自選定比稿 C1-detail
- [x] accounts 遷移落點寫死（設定 .sgroup）
- [x] 非目標/硬邊界列出（不動 island/後端/token/雙主題）
- [x] 實作落點 file:line 全定（控件/enum/資料/設定/測試/i18n）
