# T-ui-202 — Usage 視圖：圖表灰階化（bar/熱力/donut/專案條/tiles/3D）

`視覺遷移模式。只改外觀 / tokens / 共用元件樣式。禁止改 API、資料流、route path。服從 DESIGN-SPEC.md。`

> 依賴：T-ui-011 done（201 可並行但本專案一次一票）。

## 目標

Usage 全圖表換灰階編輯部風：30 日長條（高量深、today 粉紅）、熱力圖 5 級灰階+粉紅 today、donut 灰階+粉紅、專案條（第一名粉紅）、stat tiles（Est.Cost 反白）、3D 柱色對齊同色階。

## 範圍（只准動這些檔案）

* `src/analytics.ts`（圖表 markup/class 與色映射；聚合邏輯不動）
* `src/heat3d.ts`（僅柱色/hover 高亮色值）
* `src/styles.css`（analytics 區樣式；island 區塊禁改）
* `src/colors.ts`（byAgent/byModel 圖表色映射改灰階序；provider 家族色停用於圖表）
* `src/analytics.test.ts`（斷言同步）

## 規格

照 DESIGN-SPEC §共用元件清單對應列：

1. BarChart30：56px 高、柱距 2px、radius 1px；>60% 最大值 `#18181B`、其餘 `#D4D4D8`、today `#EC4899`；軸標只留「30d ago / today」9px（today 粉紅 600）。堆疊 byAgent 改單色量值（總量），**不再按工具上色**——工具佔比資訊由 donut/legend 承載。
2. 熱力圖：格色 `#F4F4F5/#D4D4D8/#A1A1AA/#52525B/#18181B`、today `#EC4899` + `#EC489960` 外圈 1.5px offset 1；legend less→more 靠右 9px。
3. 3D（heat3d.ts）：柱色同 5 級、hover emissive 用粉紅系高亮；其他行為不動。
4. donut：段色序 `#18181B/#71717A/#D4D4D8/#EC4899`（第 4 段起循環灰階）、環寬 7、gap 2、56px；legend 10px、數值靠右 tabular。
5. ProjectBars：名稱欄 72px 截斷、軌 3px、第一名 `#EC4899` 其餘 `#18181B`、數值 10px tabular。
6. StatTiles：三 tile 6px 圓角 12px 內距；Est.Cost 反白（`#09090B` 底/`#FAFAFA` 值/label `#71717A`）、其餘白底 1px 框；值 22px/800/-0.04em；PR NOW badge 用 accent（004 已做，僅對色）。
7. `#analytics` 300px 契約、內部捲動、380px 無溢出不變；空資料整組不渲染慣例不變。
8. 圖表 label 全走 10px uppercase +0.12em faint（DAILY TOKENS 等，沿用既有 i18n key）。

## SPEC / PLAN 依據

* DESIGN-SPEC §BarChart30/HeatmapCalendar/ActivityDonut/ProjectBars/StatTiles、§Do/Don't（圖表全灰階）

## Out of scope（這張票不碰）

* heatCells()/聚合/subtab 邏輯、share（203）、island、後端

## Build / Verify

    檢查:   npm test && npm run build && cargo test --manifest-path src-tauri\Cargo.toml

驗收：

| 開哪個 URL | 做什麼 | 期望看到 |
| --- | --- | --- |
| http://localhost:1420 | Usage 各 subtab + month/week 切換 | 全灰階+粉紅 today；tiles 反白正確；空情境不渲染 |
| 同上 | 切 3D | 柱色同灰階五級、hover 粉紅高亮 |
