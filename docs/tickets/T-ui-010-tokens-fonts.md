# T-ui-010 — Foundation：token 機械替換 + Playfair Italic 本地 subset

`視覺遷移模式。只改外觀 / tokens / 共用元件樣式。禁止改 API、資料流、route path。服從 DESIGN-SPEC.md。`

> Wave2 第一張（Foundation）。此票後全 app 變 light 但版式尚未重排——**過渡期允許醜、不允許壞**（一切可點、可讀、build 綠）。

## 目標

`src/styles.css` :root 照 DESIGN-SPEC「舊→新對照表」逐條機械替換（light 編輯部 palette）；新增 `--serif` 與 Playfair Display Italic 本地 subset。island 區塊一個字元都不改。

## 範圍（只准動這些檔案）

* `src/styles.css`（僅 :root token 值與 panel 玻璃相關宣告；`--island-*` 區塊禁改）
* `src/fonts.css`（新增 Playfair Italic @font-face）
* `scripts/gen-playfair-subset.mjs`（新檔，仿 gen-noto-subset.mjs 用 subset-font 產 woff2）
* `package.json`（加 `gen:playfair` script；**不加 runtime 依賴**）
* `public/fonts/`（產出 playfair_italic_sub.woff2）

## 規格

1. 照 `docs/DESIGN-SPEC.md`「（情境 B 部分）舊 → 新對照表」逐條替換；表沒列的 token 不動。
2. 新增：`--serif`、`--faint: #A1A1AA`、`--safe: #16A34A`、`--stale: #A1A1AA`（照 SPEC token 表）。
3. 移除 panel 的 `backdrop-filter`/blur 與玻璃 rgba 底（`--panel-bg` → `#FAFAFA` 不透明；`box-shadow` 換 SPEC 的 shadow-panel）。
4. Playfair subset：字集只需 `A-Za-z0-9 '’,.—%` 與票面已知字樣（left、What's left in the tank、Thirty days of consumption 等），italic 400 單檔；下載來源用 npm 套件 `@fontsource/playfair-display`（devDep）或 Google Fonts 靜態檔**一次性下載進 repo**——runtime 零外連。
5. `:focus-visible` outline 色跟 `--accent`（新粉紅）。
6. 驗收允許的過渡瑕疵：元件間距/字級未重排、深色殘影；不允許：文字不可讀（對比崩壞）、island 變樣、任何互動壞掉。

## SPEC / PLAN 依據

* DESIGN-SPEC §Design Tokens、§舊→新對照表、§字體
* CLAUDE.md：字體本地 bundle 慣例（gen:noto 前例）

## Out of scope（這張票不碰）

* 版式重排（011/201/202/203 的事）、island.ts/island 樣式、share.css、後端

## Build / Verify

    產字:   node scripts/gen-playfair-subset.mjs（或 npm run gen:playfair）
    檢查:   npm test && npm run build && cargo test --manifest-path src-tauri\Cargo.toml

驗收：

| 開哪個 URL | 做什麼 | 期望看到 |
| --- | --- | --- |
| http://localhost:1420 | 全面板走一遍 | light 底、墨色字、無玻璃模糊；island 完全沒變；無 console 錯誤 |
| build 輸出 | 看 dist | playfair woff2 進 bundle；無外連 fonts.googleapis |
