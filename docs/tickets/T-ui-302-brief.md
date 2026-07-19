# T-ui-302 [visual] 分析頁 Magazine 版面（styles.css，雙主題）

先讀 `docs/RUNBOOK.md` 與 `AGENTS.md` 硬邊界。真相：`docs/DESIGN-SPEC-analytics-2lens.md` §2 + 選定比稿 `design/previews/analytics-C1-detail.html`（像素/值來源）。**依賴 T-ui-301 已落地**（DOM/class 由 301 emit）。

## 目標
把分析頁 CSS 改成 C1 Magazine 版面，對上比稿 `analytics-C1-detail.html`，亮暗雙主題都對。**只動 `src/styles.css`**。

## 範圍（硬白名單）
- 只准動 `src/styles.css`（分析頁相關區塊 + 清理廢除的 `.subtabs`）。
- 不動 island、Limits、戰報、BottomBar、設定頁樣式。不動任何 `.ts`/`.html`/後端。
- **不新增 design token**：全走既有 `:root` 的 `--g1..g5`/`--accent`/`--text/--dim/--muted/--faint`/`--line`/`--card`/`--bg`。暗色自動跟隨既有 `:root.dark`。

## 要做（值抽自 `analytics-C1-detail.html`，見 SPEC §2）
- `.feature` 鏡頭區隔：`.feature + .feature` margin-top 52px + padding-top 42px + 1px `--line` 頂線。
- `.cap` 9.5px/600/uppercase/+.14em/faint。
- `.kick` Playfair Display italic 15px/muted/lh1.55/max-width300（serif 已由 `src/fonts.css` 提供 `--serif`；沿用，勿內嵌新字）。
- `.toggles/.seg`：膠囊 `--g1` 底、圓角 999、padding 3px；`.seg button` 10.5px/600/muted/padding 5px 12px；**`.seg button.on` = `--text` 底 + `--bg` 字（反白）**。
- `.hero .eyebrow`(10px/600/+.06em/uppercase/faint)、`.hero .fig`(Geist 600/-.045em/lh.9；Trends total 70px、Breakdown 名 56px；tabular-nums)、`.hero .fig .u`(29px/500/g3)、`.hero .sub`(13px/muted，`b`→text)。
- `.support .lbl`(10px/600/+.06em/uppercase/faint)、`.chart`(flex,align-end,gap2,h70)+`.bar`(`--g2`,圓角1)+`.bar.strong`(`--g4`)+`.bar.today`(`--accent`)、`.xaxis`(9px/faint)。
- `.rows`(gap18)+`.row .meta`(13px)+`.row .nm`(500)+`.row .vl`(muted/600/tabular)、`.track`(2px `--g1`)+`.track i`(`--g3`)、`.row.top .track i`(`--accent`)。
- `.donutsec`(flex,gap20,align-center)+`.legend`(11.5px，`i`8px方點2r，`b`右對齊/text/600/tabular)。donut 段色走灰階 ramp（由 301 inline style 給；CSS 僅排版）。
- `.comp`+`.compbar`(h8,r2,flex)+`.compbar i`+`.complegend`(10.5px/muted wrap，`b`→text/600/tabular)。
- `.footnote`(11px/faint/lh1.6，`b`→dim/600)。
- 清理：舊 `.subtabs`/`.subtabs button` 若 301 廢除則移除；舊 analytics 專屬樣式（統計頁 `.accounts/.acct/.kv/.records` 等已遷移/併入者）清乾淨，勿留死 CSS。

## 雙主題複驗（硬）
- 亮色與暗色（`:root.dark`）各檢一次：hero 大字對比、灰階 donut 段在暗色反轉 ink ramp 下仍可辨、洋紅（亮 #EC4899 / 暗 #F472B6）在兩底都夠、segmented 反白選中兩主題都對、髮絲線可見。

## Build / Verify（commit 前必過）
- `npx tsc --noEmit` 乾淨、`npm test` 綠（CSS 改動不應影響，但跑確認無連帶）。
- 對照比稿 `analytics-C1-detail.html`：380px 下版面、字級節奏、間距、每鏡頭洋紅 ≤1 一致；無橫向溢出。
- 亮暗雙主題各驗。

## 模式宣告
純視覺（CSS-only），不動邏輯/DOM/token。違反範圍白名單＝作廢重來。
