# ROUND v0.8 — 分享改版 + 圖表升級 + 供應商多選（2026-07-18 使用者八項回饋）

> 前置：v0.7 各票已完成待總驗收。本輪票號 T-908 起。視覺項目套 taste 原則（對比、不截斷承載資訊、留白節奏、tooltip 只當補充）。

## 立即修（小、驗收阻斷）
- **T-908 [fix]** 拆分頁捲到底「其他專案」被擋住 — 捲動容器底部空間/遮擋問題。
- **T-909 [visual]** 暗色模式限額頁 safe 綠（#22C55E）太螢光 — 調暗/降飽和，維持 AA。

## 功能票
- **T-910 [feat] 30 秒自動更新**：新設定「更新頻率」（30s/60s/3min），額度 API 輪詢跟隨。**風險註記**：本帳號曾因與 Claude Code 共用限流桶而持續 429（F-01）；30s 是現行 Claude 輪詢 6 倍請求量。配套：429 指數退避（F-01 當時延後的項目）+ 既有 last_good/Stale 保底。預設值維持保守，30s 由使用者自選。
- **T-911 [chart] 圖表軸線與數值**：總覽日圖 X 軸補中間日期刻度（同 hourly 的 6h/12h/18h 修法）；所有長條圖加 Y 軸（刻度線+數值）；hover 直接顯示數值（自訂 tooltip，不靠原生 title 的延遲）。圖表需留出左側軸距。
- **T-912 [feat] 活動類型細分**：classify_kind 從 Edit/Read/Run/Other 擴為 Edit/Read/Search(Grep/Glob)/Run(Bash/PS)/Web(WebFetch/WebSearch)/Agent(Task/子代理)/MCP/Other（實際以 log 中工具名盤點為準）；donut 色板與 i18n 對應擴充。
- **T-913 [design] 分享模板全面重設計 → HTML 預覽**：先做**獨立 HTML 預覽檔**給使用者挑（不動真 code）：六款模板重設計（16:9+9:16 雙尺寸並排）。使用者定案後開 **T-915** 實作。
- **T-914 [arch]** 「戰報」全面改稱「分享」；從分析 subtab 移出 → header 齒輪旁新增分享 icon，開啟為**整頁模式**（同 T-902 設定頁架構）。與 T-915 實作合併執行（同一批動 share 檔）。
- **T-916 [feat] 供應商多選 + Grok**：providers 從三選一改**多選**（claude/codex/opencode/gemini/grok），吸收現有 tool_opencode/tool_gemini 開關；grok 為 usage-only 來源（同 opencode/gemini 定位）。**前置偵察**：本機 grok CLI log 位置與 schema（若機器上沒有 grok 資料，先實作介面+掃描器骨架、標 insufficient data）。設定 UI 用多選 chip/checkbox 群（T-903 語彙延伸）。

## 執行順序
1. T-908/909（主 session 直修）＋ T-913 設計預覽（executor）＋ grok 偵察（scout）——並行
2. T-910（codex，後端）——與 1 並行
3. T-911 → T-912（都動 analytics.ts，依序）
4. T-913 使用者定案 → T-914+T-915（分享架構+新版面，一批）
5. T-916（依偵察結果）
6. 總驗收 → bump 0.8.0 打包（0.7.0 不單獨出包，直接併入）
