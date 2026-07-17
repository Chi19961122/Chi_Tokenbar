# T-feat-005 — 3D 熱力圖（three.js 柱體、2D/3D toggle、預設 2D）

`只實作本票行為與資料。可沿用現有樣式。禁止大規模 redesign。對照 PLAN flow。`

> 依賴：T-fix-001~003（畫的是修正後的數字）。Wave1 最後一張。

## 目標

Usage overview·month 的熱力圖區塊多一個 2D/3D 小切換：3D 為 three.js WebGL 柱體（高度=當日 token），可拖曳旋轉、滾輪縮放、hover 顯示「日期 · tokens」。預設 2D，選擇被記住。island/Limits 路徑 bundle 零影響。

## 範圍（只准動這些檔案）

* `package.json` / `package-lock.json`（新依賴 three；`npm i three`，devDep 加 `@types/three`）
* `src/heat3d.ts`（新檔：全部 three.js 邏輯集中此檔）
* `src/analytics.ts`（toggle UI + 掛載點 + 動態 import）
* `src/i18n.ts`（toggle 文案 key，如 `analytics.heatmapView2d/3d`）
* `src/styles.css`（toggle 與 canvas 容器樣式，用既有 token）
* `src/analytics.test.ts`（決策邏輯測試）
* `vite.config.ts`（僅在需要確保 three 分包時）

## 規格

1. **依賴紀律**：`three` 只允許被 `src/heat3d.ts` import，且 `analytics.ts` 用 `await import("./heat3d")` 動態載入（首次切到 3D 才載）。驗收檢查主 chunk gzip 增量 < 5KB（three 必須在獨立 async chunk）。
2. **toggle**：熱力圖 section 標題列右側小型 segmented toggle「2D | 3D」。選擇存入既有前端 UI 狀態持久化機制——先查 `analytics.ts` 現在怎麼記 subtab/range（沿用同一套；若無持久化則用 localStorage key `tokenbar.heatmap_view`，值 `"2d"|"3d"`，預設 `"2d"`）。不新增後端 settings。
3. **3D 場景**（heat3d.ts，導出 `mountHeat3d(container, grid, totals, opts)` 與 `disposeHeat3d()` 之類的最小介面）：
   - 資料直接吃既有 `heatCells()` 的 `HeatGrid` + 每日 total（與 2D 同源，不重算）。
   - 每 cell 一根 Box 柱：x=weekCol、z=weekdayRow、高度 = intensity 映射（最高柱 ≈ 場景高度上限；intensity 0 給極矮平板不是 0 高，讓格子存在感保留）。
   - 顏色沿用 2D 的 5 級色（從 CSS `--hm-*` 或 hm-l0..4 對應的 token 讀值/硬編同值），不發明新色。
   - 相機：透視相機，初始等角視角；OrbitControls（`three/examples/jsm/controls/OrbitControls.js`）拖曳旋轉 + 滾輪縮放（距離 clamp），無 pan。
   - hover：raycaster 命中柱體 → 顯示 tooltip「YYYY-MM-DD · fmtTokens」（DOM tooltip 跟游標，不用 sprite 文字）；命中柱微亮（emissive）。
   - 打光：ambient + 一盞 directional，夠辨識立體即可。
   - `renderer.setPixelRatio(Math.min(devicePixelRatio, 2))`；渲染用 on-demand（controls change / hover 變化才 render），**不跑 60fps 常駐 rAF**（常駐監控 app，省電）。
4. **版面契約**：canvas 尺寸 = 現有熱力圖區塊寬 ×（高度沿用 2D 區塊高度或 ≤160px），`#analytics` 300px 高度契約不變、內部捲動不變、380px 寬不橫向溢出。
5. **生命週期**：analytics 面板每 tick 重渲染 —— 3D 實例**不可每秒重建**。掛載點給穩定 DOM（container id），資料未變不重建場景、只在 grid 內容變化時更新柱高；切回 2D、切走 subtab、面板關閉時 `dispose()`（geometry/material/renderer 全釋放，防 WebGL context 洩漏）。實作方式參考現行 render 週期後自行選擇（例如 render 後 re-attach 既有 canvas），但驗收標準是：mock 模式下開著 3D 60 秒，console 無 context lost / 記憶體無明顯攀升。
6. **降級**：`WebGLRenderer` 建構失敗或 `webgl2/webgl` context 拿不到 → 自動回 2D、隱藏 toggle（不報錯彈窗）。
7. **決策邏輯抽純函式並測試**（CLAUDE.md 慣例：排版不測、決策要測）：如 `heatBarHeight(intensity)`、view 持久化 read/write、fallback 判斷。i18n en/zh-TW 補齊。
8. 軸標維持英文（同 2D 的語系洩漏哲學）；3D 版可省略軸標（hover 已有日期），但不得出現中文。

## SPEC / PLAN 依據

* docs/PLAN.md 功能 Delta「3D 熱力圖 + toggle + 記憶」＋ 風險節（bundle、WebView2、300px 契約、fallback）
* 三樣態計畫 §階段 C+「3D（WebGL 柱體）視效能與需求再議」→ 本輪拍板執行
* CLAUDE.md：Port 1420 互斥、mock 模式驗證、決策邏輯抽函式測試

## Out of scope（這張票不碰）

* 不動 2D 熱力圖行為與 `heatCells()`
* 不擴到 week/其他 range、不做 3D 動畫入場效果（Wave2 視覺再議）
* 不動後端

## Build / Verify

    安裝:   npm i three && npm i -D @types/three
    檢查:   npm test && npm run build && cargo test --manifest-path src-tauri\Cargo.toml
    啟動:   npm run dev（先確認 1420 沒被占用；瀏覽器開 mock 模式）

驗收：

| 開哪個 URL | 做什麼 | 期望看到 |
| --- | --- | --- |
| http://localhost:1420 | Usage → overview → month，點 3D | WebGL 柱體圖出現在同區塊，高度對應深淺 |
| 同上 | 拖曳 / 滾輪 / hover | 旋轉、縮放（有極限）、tooltip 顯示日期+tokens |
| 同上 | 切 2D → 3D → 關面板重開 | 選擇被記住；無 console 錯誤 |
| 同上 | devbar 切 empty | 熱力圖區塊照既有邏輯消失，無殘留 canvas |
| build 輸出 | 看 vite 產物清單 | three 在獨立 async chunk；主 chunk gzip 增量 < 5KB |
