# ROUND v1.0 — 分析頁 2 鏡頭改版 + 品牌記號統一（Atoll IA/打磨輪，2026-07-19）

> 使用者定案：分析頁改 **C1 Magazine + 補細項（option B）**；品牌 ◎ 統一為安裝檔額度弧。
> 真相：`docs/DESIGN-SPEC-analytics-2lens.md` + 選定比稿 `design/previews/analytics-C1-detail.html`（像素/token 來源）。

## 已拍板

- **分析頁 IA**：4 subtab（overview/hourly/share/stats）→ **2 鏡頭 Trends / Breakdown**，且比稿為**上下堆疊一路捲**（**移除 subtab 切換列**，toggle 每鏡頭內嵌），非兩個可切頁籤。
- **控件模型**：range(全域窗) + metric(跨鏡頭) 常駐；granularity(Daily/Hourly) 僅 Trends；group(model/agent) 僅 Breakdown。（見 SPEC §1.1）
- **stats 拆解**：composition→Breakdown、records/sessions/tokPerMin→Trends hero/footnote、**accounts→設定頁**。
- **視覺**：Magazine（大 hero 數字 + serif kicker + 編輯部黑白 segmented + 灰階 donut/composition/projects），每鏡頭洋紅 ≤1（Trends=today、Breakdown=#1 row）。亮暗雙主題都要對。
- **純前端**：`Analytics` payload 一次給齊，不動後端/Tauri 指令。

## 工單與波次

**已完成：**
- **T-931 [visual] 品牌 ◎ 統一額度弧** — `index.html` header + `share.ts` RING_MARK 改開口弧對上安裝檔；SEAL_MARK 不動；tsc+139 測試綠。→ `8a6692c`（F-14）。✅

**Wave 1（序列，非平凡、大改）：**
- **T-ui-301 [arch/frontend] 2 鏡頭 IA + 控件 + 邏輯 + accounts 遷移 + i18n + 測試**
  - 擁有：`src/main.ts`、`src/analytics.ts`、`src/analytics.test.ts`、`src/i18n.ts`（必要時 `src/settings-controls.ts`）。
  - 移除 subtab 切換列（`renderSubtabs` 廢除或改為無切換）；兩鏡頭常駐渲染；toggle 內嵌每鏡頭。
  - `SubTab` enum → `trends|breakdown`（或直接改為「兩鏡頭皆渲染」的模型）；`ui` 加 `granularity`。
  - `renderAnalytics` 重寫：一次輸出兩鏡頭 DOM，用**比稿 `analytics-C1-detail.html` 的 class 名為契約**（`.feature/.cap/.kick/.toggles/.seg/.hero/.fig/.eyebrow/.sub/.support/.lbl/.chart/.bar/.rows/.row/.track/.donutsec/.legend/.comp/.compbar/.complegend/.footnote`）。
  - stats 拆解重分配；accounts 移設定頁 `.sgroup`。
  - `analytics.test.ts` 20+ 處 subtab 字串更新；純函式測試保留。
  - i18n 增 `subtab.trends`/`breakdown`、`toggle.daily`/`hourly`、accounts 標；刪不再用者。en+zh。
  - **不碰 `styles.css`**（視覺歸 T-302；只保證 emit 正確 class 名）。

**Wave 2（序列，依賴 T-301 的 DOM）：**
- **T-ui-302 [visual] 分析頁 Magazine 版面（styles.css）**
  - 擁有：`src/styles.css`（分析頁相關區塊；`.subtabs` 若廢除則清理）。
  - 照 `analytics-C1-detail.html` 實作/替換分析頁 CSS：hero 字級、serif kicker、segmented 黑白選中反白、donut/composition/projects/feature 間距。
  - **亮暗雙主題**都對（走既有 `--g*`/`--accent`/文字階 token，不新增 token）。
  - 不動 island/Limits/戰報/BottomBar 樣式。

## 驗證閘

每票非平凡 → tsc + vitest(前端) 綠 → **fresh-context verifier 對抗驗證**（REFUTE 姿勢；亮暗雙主題各驗、空/stale 態、range/metric/granularity/group 切換、月熱力圖僅月、accounts 已移設定且分析頁不再現）→ 一票一 commit（訊息帶為什麼）。

## 收尾（使用者硬閘）

T-301+302 綠 + verifier CONFIRMED → **使用者真機驗收**（跑 dev 看兩鏡頭捲動、toggle、雙主題、accounts 進設定）→ 若過，本輪可併入下次打包。

## 風險/待議

- **IA 讀法**：比稿為「堆疊捲動、無 subtab 切換」。若使用者其實要「兩個可切頁籤」，需回頭改 T-301 控件模型（開票前已於對話標明此讀法）。
- **range 與 granularity 交互**：Daily 吃 range 窗、Hourly 固定 24h；Hourly 時 range 是否隱藏 → 實作者依比稿與最低驚訝原則定，verifier 檢查切換無殘影。
- **accounts 資料流**：設定頁若無 analytics payload，需擇低耦合來源餵帳號（見 SPEC §3）。
- **雙主題**：Magazine 大字/灰階 donut 在暗色對比要複驗（donut 段灰階在 `:root.dark` 反轉 ink ramp 下仍需可辨）。
