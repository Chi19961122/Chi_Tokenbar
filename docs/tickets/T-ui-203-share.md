# T-ui-203 — 戰報 Share 換皮（六模板 token 對齊，版面結構不動）

`視覺遷移模式。只改外觀 / tokens / 共用元件樣式。禁止改 API、資料流、route path。服從 DESIGN-SPEC.md。`

> 依賴：T-ui-010 done（011/201/202 非必要依賴）。Wave2 最後一張。

## 目標

六個戰報模板與 share 面板換編輯部皮：light 底、墨字、灰階圖形、粉紅點綴；**版面結構、欄位、尺寸、匯出流程一律不動**（§0 專案名禁令照舊）。streak/maxDay 數字（T-feat-004 已入 ShareData）若模板有自然欄位槽可帶入。

## 範圍（只准動這些檔案）

* `src/share.css`（模板樣式主檔）
* `src/share.ts`（僅模板內 class/色 token 引用；buildShareData 與資料欄位不動——顯示 streak/maxDay 除外）
* `src/share-panel.ts`（share 面板容器樣式 class）
* `src/share.test.ts`（斷言同步）
* `src/i18n.ts`（若 streak 顯示需文案 key；en/zh-TW 齊）

## 規格

1. 色/字/間距全依 DESIGN-SPEC token 表：底 `#FAFAFA`、卡 `#FFFFFF`、字 `#09090B`/`#52525B`/`#71717A`、髮絲線 `#E4E4E7`、粉紅點綴每模板 ≤1 處、狀態/圖形灰階。
2. 大數字模板走 hero 風（800 重、緊字距、tabular）；小字 label uppercase +0.12em。
3. serif italic 僅作單一點綴詞（如 "left"／模板既有 slogan），不得整段。
4. PNG 匯出（html-to-image）在 light 底下輸出正確——特別驗字體渲染與背景不透明。
5. streak/maxDay：僅在 stats 型模板加一行小字（如「14d streak · peak 81k」樣式對齊該模板既有 caption），其他模板不動；無資料不顯示。
6. §0 紅線：任何模板不得出現專案名；`buildShareData` 不動。

## SPEC / PLAN 依據

* DESIGN-SPEC §SharePreview、§Design Tokens、§Do/Don't
* 三樣態計畫 §0、§階段 D（模板結構）

## Out of scope（這張票不碰）

* 模板版面重排、新模板、匯出流程/檔名邏輯、island、後端

## Build / Verify

    檢查:   npm test && npm run build && cargo test --manifest-path src-tauri\Cargo.toml

驗收：

| 開哪個 URL | 做什麼 | 期望看到 |
| --- | --- | --- |
| http://localhost:1420 | share 面板逐一預覽六模板 | 全部 light 編輯部皮、無專案名、粉紅 ≤1 處 |
| 同上 | 匯出 PNG | 輸出圖底不透明、字體正確 |
