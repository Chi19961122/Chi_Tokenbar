# PLAN — TokenBar v0.6 輪（3D 熱力圖 + 視覺改版）

> scenario: C（功能也調，含視覺換皮；D 環境驗收 2026-07-17 全綠：Node v24.11.1、build 903ms、codex CLI 在、git main 乾淨）
> 行為/狀態機真相仍是 `Ai_Assistant/TokenBar UX Spec v3.md`；本檔只管這一輪的範圍與視覺流程。

## 受眾

使用者本人（Windows 上同時跑 Claude Code / Codex 的重度 AI coding 使用者）。常駐監控額度 runway，偶爾開面板看用量分析、匯出戰報。

## 核心功能（這一輪）

| 功能 | 優先級 | 說明 |
| ---- | --- | ---- |
| Claude log 去重 | P0 | scan_claude 按 message/request ID 去重（resume/fork session 的副本現在會重複計數，Usage 全維度偏高）— 參考 brrrn 做法 |
| Codex 歸屬修正 | P0 | fork replay 防重複計數；累計轉增量逐筆歸屬，修跨午夜 session 的日桶/每時失真 |
| 3D 熱力圖 | P0 | three.js WebGL 柱體（高度=當日 token），與現有 2D 日曆圖 toggle 切換，預設 2D，選擇記進設定 |
| 定價精準化 | P0 | input/output/cache_read/cache_creation 分開累計，vendored 靜態價目表分項計價，取代 blended $/Mtok 粗估（不外連） |
| PR 個人紀錄/連勝 | P0 | max hour/max day/連續活躍天數、「PR NOW」badge；資料吃既有 hourly/daily；戰報 Share 可引用（不含專案名） |
| 視覺換皮 | P0 | 依 Claude Design「TokenBar Design System」選定方向，換 island pill + 面板 + 選單的視覺 token 與元件外觀 |
| 熱力圖真機版面驗證 | P1 | HANDOFF 掛著的 380px 真視窗版面驗證（2D+3D 一起驗） |

## Non-goals（明確不做什麼）

* 後端只動 analytics 掃描/聚合層（去重、分項計價、增量歸屬）；providers、額度引擎、狀態機一律不碰
* 定價表不外連（不抓 LiteLLM 線上表）— vendored 靜態表隨版本更新
* 不做社交排行/pit board、不架任何後端服務、維持「絕不上傳」
* 不動戰報 Share 格式；§0 硬限制不變（專案名絕不進戰報，`buildShareData` 禁令照舊）
* 不動額度演算法、狀態機、providers（UX Spec v3 管的行為一律不碰）
* 熱力圖仍只在 Usage overview · month 範圍出現（3D 不擴到其他 range）
* 不做多語系新增、不做手機/網頁版

## 頁面清單（Tauri 桌面 app，無 route — 以視圖為單位；驗收走 1420 mock preview）

| 視圖 | 進入方式 | 用到的 P0 功能 |
| ---- | ---- | ---- |
| Island pill | 常駐懸浮 | 視覺換皮 |
| 面板 · Limits | 點 island 展開 | 視覺換皮 |
| 面板 · Usage(overview·month) | 面板切 tab | 3D 熱力圖、視覺換皮 |
| 設定選單 | 面板選單 | toggle 記憶、視覺換皮 |
| 戰報 Share 預覽 | Usage → share | 視覺換皮（格式不動） |

## 資料模型與頁面流轉

* 3D 熱力圖資料 = 既有 `Analytics.daily`（30 日桶）→ `heatCells()` 已產出 `{date, weekdayRow, weekCol, intensity}` → 3D 版直接複用同一 grid，intensity 映射柱高。無新資料模型。
* 新增前端設定一項：`heatmap_view: "2d" | "3d"`（預設 `"2d"`）。存放位置跟現有前端設定同一套機制（實作票內確認：settings 後端 or localStorage，不得自創第三套）。
* 面板 `#analytics` 300px 高度契約不變；3D 檢視在同一區塊內切換，不得撐高。

## 權限與角色

單機單人工具，無角色權限。唯一等同「權限」的鐵則：機密 token 檔絕不印出；專案名絕不進戰報。

## Stack (locked) — 中途不換

* Framework: Tauri 2 + vanilla TypeScript（無前端框架）
* 樣式方案: vanilla CSS，token 集中在 `src/styles.css` :root（樣式現況＝集中，無需收攏票）
* 元件庫: 無（手寫 DOM/HTML string）
* 3D: three.js（本輪唯一新依賴；tree-shake、只 import 用到的模組）
* 字體/Icon: Geist / Geist Mono（`public/fonts/`）；island 品牌 icon 沿用 lobe-icons 方向
* Package manager: npm
* Node 版本: v24（機器現況 v24.11.1）
* 測試: vitest（前端）+ cargo test（後端，本輪不應有後端變更）

## 視覺方向（定案後回填）

* chosen_direction: <待選 — Claude Design 專案「TokenBar Design System」已有六方向比稿>
* 參考圖路徑: `design/refs/direction-chosen.png`

* * *

## （情境 B 部分）換皮邊界

    Redesign mode: visual-only（僅適用 Wave2 視覺票）
    - 不動: 行為/狀態機/演算法（UX Spec v3）、tauri 視窗尺寸契約、文案 key（i18n.ts）
    - 必換: 色 / 字階 / 間距 / 元件外觀（依選定方向的 token）
    - 可換: 面板排版節奏、區塊分隔形態
    - 成功: 五視圖像新方向，舊功能全可點；三發行版外觀一致（CONFIG.md §7）
    - 樣式現況: 集中（styles.css :root tokens + 共用 class）→ 不需收攏票
    - 基準圖: design/screenshots/baseline/（Wave2 開工前用 mock preview 補截五視圖）

## （情境 C 部分）功能 Delta

| 動作 | 項目 | 優先級 |
| --- | --- | --- |
| 修改 | scan_claude 加 message/request ID 去重（HashSet，跨檔案） | P0 |
| 修改 | scan_codex fork replay 防重 + 累計轉增量逐筆歸屬（日桶/每時） | P0 |
| 修改 | 掃描層分開累計 input/output/cache_read/cache_creation；vendored 價目表分項計價 | P0 |
| 新增 | PR 個人紀錄（max hour/max day/連勝天數 + PR NOW badge），戰報可引用 | P0 |
| 新增 | 3D 熱力圖（three.js 柱體、拖曳旋轉/縮放、hover 顯示日期+tokens） | P0 |
| 新增 | 熱力圖 2D/3D toggle + `heatmap_view` 設定記憶 | P0 |
| 刪除 | 無 |  |

* 關鍵 flow 舊 vs 新: Usage overview·month 原本只有 2D 日曆格 → 新增右上小 toggle（2D｜3D）；切 3D 後同區塊渲染 WebGL canvas，拖曳旋轉、滾輪縮放、hover tooltip 同 2D 資訊（date · tokens）；切換選擇下次開啟仍記得。去重/計價修正對 UI 無新元件，只讓既有數字變準（數字會變小或變準，戰報/tiles 同步受惠）。
* API / DB 改什麼: 無對外 API；後端限 analytics.rs 掃描/聚合層（去重、增量、分項計價）+ 新 PR/streak 聚合欄位；相容（沒有 migration，cache 版本號 bump 即可）。
* 波次策略: Wave0 正確性（去重×2 + 分項計價；先修數字）→ Wave1 功能（PR/streak → 3D 熱力圖）→ Wave2 視覺（tokens → shell → 各視圖）。
* 測試: 去重/增量/計價/streak 各補單元測試（假 log 樣本）；cargo test + vitest 全綠為 build gate。
* 風險與回滾:
  - three.js 讓 JS bundle 從 ~36KB gzip → ~186KB gzip（約 5 倍）：用動態 import 只在切到 3D 時載入，island/Limits 路徑零影響。
  - WebView2 的 WebGL/ANGLE 行為未實測：mock preview（Chrome）先驗，真機 tauri dev 再驗；GPU 不可用時 fallback 顯示 2D 並藏 toggle。
  - 300px 高度契約：3D canvas 固定吃現有熱力圖區塊高度，超出即算驗收失敗。
  - 回滾：toggle 預設 2D，出問題僅影響 opt-in 的 3D 檢視；整包可 revert 單一 feature commit。
