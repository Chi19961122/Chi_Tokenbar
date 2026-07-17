# T-ui-201 — Limits 視圖：GaugeCard 編輯部化（hero 數字 + 狀態色 + 彩色品牌 icon）

`視覺遷移模式。只改外觀 / tokens / 共用元件樣式。禁止改 API、資料流、route path。服從 DESIGN-SPEC.md。`

> 依賴：T-ui-011 done。

## 目標

每工具一張 GaugeCard：header（**providerIcon() 彩色真 icon** + 大寫工具名 + 狀態字），雙欄 hero（5h 視窗、週上限），60px/800 狀態色巨大「剩餘%」數字 + serif 斜體「left」、3px 細 gauge（填剩餘）、detail 與 reset 句。

## 範圍（只准動這些檔案）

* `src/panel.ts`（Limits 區 markup/class；資料計算不動）
* `src/styles.css`（gauge 卡樣式；island 區塊禁改）
* `src/panel.test.ts`（斷言同步）
* `src/colors.ts`（若狀態色取用邏輯在此，僅換色值來源）

## 規格

照 DESIGN-SPEC §GaugeCard 狀態矩陣與 §字級表：

1. hero 數字＝**剩餘%**（油量隱喻不變）；60px/800/-0.055em/lh0.82，色=該 gauge 狀態色；「%」15px/700 同色；「left」13px serif italic faint（zh-TW 對應詞照 i18n）。
2. kicker 9px uppercase +0.16em faint（5-HOUR WINDOW / WEEKLY，沿用既有 i18n 文案）。
3. gauge 軌 3px `#E4E4E7` 圓頭、填剩餘、700ms transition；detail 10px `#52525B`、reset 句 9px faint。
4. header：providerIcon(p, 14) **彩色照真**（Claude 橘、Codex 藍紫漸層；icons.ts 不動）；工具名 11px/600 uppercase +0.12em；右側狀態字 10px + 1.5px 圓點，取兩 gauge 最差態。
5. 狀態→色映射照 SPEC 矩陣（safe 綠/near 琥珀/locked 紅/stale 灰/degraded 沿最後已知+caption）；locked 短標行為不變（UX Spec v3 管）。
6. 卡片透明底、底部髮絲線分隔；px-20 py-24。
7. OpenCode/Gemini 等無 Limits 工具的既有呈現規則不變（行為歸 UX Spec v3），僅換樣式。

## SPEC / PLAN 依據

* DESIGN-SPEC §GaugeCard 狀態矩陣、§字級、第五裁決（彩色品牌 icon）
* UX Spec v3：行為/狀態機禁動

## Out of scope（這張票不碰）

* Usage 圖表（202）、share（203）、island、icons.ts、後端

## Build / Verify

    檢查:   npm test && npm run build && cargo test --manifest-path src-tauri\Cargo.toml

驗收：

| 開哪個 URL | 做什麼 | 期望看到 |
| --- | --- | --- |
| http://localhost:1420 | devbar 切 safe/near/locked/stale/degraded | hero 數字與 gauge 色隨狀態變；icon 保持彩色；油量=剩餘 |
| 同上 | 380px 檢查 | 雙欄 hero 不溢出、數字不裁切 |
