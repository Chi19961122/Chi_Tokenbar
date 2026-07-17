# T-<編號> — <一句話標題>

> 編號規則：`T-0xx` UI 基礎（tokens/theme/shell）→ `T-1xx` 元件 → `T-2xx` 頁面 → `T-9xx` 修正（回鏈 F-xxx）。情境 C 的功能票用 `T-feat-*`、API 票用 `T-api-*`，不占用 T-0xx。Foundation 沒過 build 不開 Page；一次只跑一張。

## 模式宣告（票首固定句，照情境選一句放最上面）

* 視覺票: `視覺遷移模式。只改外觀 / tokens / 共用元件樣式。禁止改 API、資料流、route path。服從 DESIGN-SPEC.md。`
* 功能票: `只實作本票行為與資料。可沿用現有樣式。禁止大規模 redesign。對照 PLAN flow。`

## 目標

<做完這張票，什麼東西會變成什麼樣。一兩句。>

## 範圍（只准動這些檔案）

* `src/...`
* `src/...`

## 規格

<具體要做什麼。數值、行為、狀態，寫到 Codex 不用猜。>

## SPEC / PLAN 依據

* DESIGN-SPEC.md § <哪一節>
* 參考圖: `design/refs/...`

## Out of scope（這張票不碰）

* <例：不動 API>
* <例：不改其他頁>

## Build / Verify（沒這節等於沒用）

    安裝:   <pnpm install>
    啟動:   <pnpm dev>
    檢查:   <pnpm lint && pnpm build>   ← 寫精確指令，不寫「跑一下測試」

驗收：

| 開哪個 URL | 做什麼 | 期望看到 |
| ------- | --- | ---- |
| `/`     |     |      |

## 回鏈（來自回饋才填）

* 來源: F-<xxx>

* * *

## Attempts（失敗才填，Codex 用）

> 每次重試從上一個 commit 的乾淨狀態重來。stderr 只留尾 80 行。2～3 次仍敗就停，交給人。

### Attempt 1

    <stderr 尾 80 行>
