# ROUND v0.7.0 — 亮暗雙主題 + 設定頁重構 + 分析頁高度

> 來源：docs/FEEDBACK.md 的 F-02c/d/e/f（2026-07-18 真機驗收、使用者已確認範圍）。
> 順序有相依性：T-901 主題基建先行，T-902/903 設定頁重構直接做成雙主題，T-904 獨立。

## 全局決策（改動前先讀）

- **theme 設定**：`settings.theme = "system" | "light" | "dark"`，預設 `"system"`。解析：`dark = theme==="dark" || (theme!=="light" && prefers-color-scheme: dark)`；`.dark` class 掛在 `<html>`；theme==="system" 時要監聽 matchMedia change 即時切換。`.dark` 同時設 `color-scheme: dark`（原生捲軸/表單跟著暗）。
- **不動區**：`--island-*` token 區塊與 island 樣式（三 release variant 外觀一致，保護區）；六張戰報模板的自帶配色（對外分享物，外觀不得隨觀看者主題變）——只驗證它們「坐在暗面板上」時周邊 chrome 的可讀性。
- **圖表色**：analytics.ts / colors.ts 內寫死的 hex（#18181B、#D4D4D8、#EC4899、#F4F4F5、kindColor 陣列等）改為 CSS 變數（inline SVG 在 HTML 中支援 `fill="var(--x)"`），亮暗各給一組值。
- **對比鐵則**：兩主題所有文字層級過 WCAG AA（4.5:1）。暗色參考起點（executor 可微調但要記錄）：panel #131316、surface #1A1A1E、text #F4F4F5、muted ≈#9A9AA3、faint ≈#7F7F88、border rgba(255,255,255,.10)、accent 亮色沿用 #EC4899、暗色提亮為 #F472B6。

## 票

### T-901 [F-02f] 亮暗雙主題基建（先行）
- Rust `Settings` 加 `theme: String`（serde default "system"）；前端 types/settings 表單（顯示與通知群組加「主題」三選，暫用 select，T-903 會重造型）＋ i18n zh/en。
- styles.css：`:root` 全部顏色 token 盤點 → `:root.dark` 對應組；散落硬編碼色（panel/analytics/settings/contextmenu/skeleton/quota-summary/gauge 等）收斂進 token。
- main.ts：applyTheme() at boot + 設定變更 + matchMedia listener。
- 驗收：兩主題下 限額/總覽/佔比/每時/統計/戰報/設定/右鍵選單 全部可讀、AA 過；island 與戰報卡逐 px 不變。

### T-902 [F-02e] 設定整頁化
- settings 從 overlay（.settings-open 摺疊 analytics）改成整頁換頁：開啟時 cards/subtabs/toggles/analytics 全隱藏，只剩 header＋設定頁；gear 或返回列關閉回原頁。
- renderCards 的 `settingsOpen → variant full` 耦合移除（整頁模式下列表不可見）。
- fitWindow 量設定頁自然高。

### T-903 [F-02d] 設定控件重設計
- ≤3 個固定選項的 select（語言/顯示平台/展開預設/輔助讀數/重設顯示/主題/Claude refresh/Codex 來源）→ 沿用 `.seg` segmented 按鈕語彙；動態 pin 下拉保留 select 但重造型（自繪箭頭、token 化配色）。
- readSettingsForm 對應改寫；雙主題可讀。

### T-904 [F-02c] 分析頁高度解耦
- `#analytics` 300px 固定 → 進場時計算：`clamp(300, screen.availHeight − 其餘內容高 − 邊距, 640)`，寫入 CSS 變數後再 fitWindow；subtab 切換仍不 resize（anti-jank 保留）。
- 內容不足時不留大片空白（box 高度也受內容上限），超出仍內捲。

## 驗收路徑
每票：executor 實作 → tsc/vitest/cargo 全綠 → verifier CONFIRMED → commit。整輪完成後起 dev 給使用者真機驗收（注意與已安裝 0.6.0 的 single-instance 衝突，屆時先關）。
