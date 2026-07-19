# T-ui-302 [visual] 分析頁 Magazine 版面（styles.css，雙主題）

先讀 `docs/RUNBOOK.md` 與 `AGENTS.md` 硬邊界。真相：`docs/DESIGN-SPEC-analytics-2lens.md` §2 + 選定比稿 `design/previews/analytics-C1-detail.html`（像素/值來源）。**依賴 T-ui-301 已落地**（DOM/class 由 301 emit）。

## 目標
把分析頁 CSS 改成 C1 Magazine 版面，對上比稿 `analytics-C1-detail.html`，亮暗雙主題都對。**只動 `src/styles.css`**。

## ⚠ Token 對照（比稿用了不存在的 `--g*`，改用真 token）
比稿 `analytics-C1-detail.html` 的 `:root` 是我另造的簡化名，**真 `styles.css` 沒有 `--g*`**。一律換成既有 token（T-301 已把 render 內 inline 灰階映射到 `--ink-*`，你只需讓 CSS 一致）：

| 比稿名 | 真 token |
| --- | --- |
| `--g5`(最深) | `--ink-900` |
| `--g4` | `--ink-700` |
| `--g3` | `--ink-500` / `--ink-400` |
| `--g2` | `--ink-300` |
| `--g1`(最淺) | `--ink-100`（donut 軌用 `--donut-ring`） |
| `--line` | `--hairline`（或 `--border`） |
| `--card` | `--card-bg` |
| `--bg` | `--panel-bg` |
| `--dim` | `--text-dim` |
| `--muted`/`--faint`/`--text`/`--accent`/`--serif` | 同名既有 |

暗色**自動跟隨**（`:root.dark` 已反轉 `--ink-*`、`--accent #F472B6`、文字階、`--donut-ring`）。**segmented 選中反白直接用既有 `--seg-on-bg`/`--seg-on-fg`**（已含亮暗兩套，勿自寫黑白）。

## 範圍（硬白名單）
- 只准動 `src/styles.css`（分析頁相關區塊 + 清理廢除的 `.subtabs`/舊統計頁 `.accounts/.acct` 樣式）。
- 不動 island、Limits、戰報、BottomBar、設定頁既有樣式（accounts 已用既有 `.sgroup/.srow`，不需新樣式；如需 `.snote` 微調可加）。不動任何 `.ts`/`.html`/後端。
- **不新增 design token**：全走上表既有 token。

## 要做（值抽自 `analytics-C1-detail.html`，見 SPEC §2）
- `.feature` 鏡頭區隔：`.feature + .feature` margin-top 52px + padding-top 42px + 1px `--line` 頂線。
- `.cap` 9.5px/600/uppercase/+.14em/faint。
- `.kick` Playfair Display italic 15px/muted/lh1.55/max-width300（serif 已由 `src/fonts.css` 提供 `--serif`；沿用，勿內嵌新字）。
- `.toggles/.seg`：膠囊 `--g1` 底、圓角 999、padding 3px；`.seg button` 10.5px/600/muted/padding 5px 12px；**`.seg button.on` = `--text` 底 + `--bg` 字（反白）**。
- `.hero .eyebrow`(10px/600/+.06em/uppercase/faint)、`.hero .fig`(Geist 600/-.045em/lh.9；Trends total 70px、Breakdown 名 56px；tabular-nums)、`.hero .fig .u`(29px/500/g3)、`.hero .sub`(13px/muted，`b`→text)。
- `.support .lbl`(10px/600/+.06em/uppercase/faint)、`.xaxis`(9px/faint)。**注意**：Trends 主圖是 T-301 沿用的既有 **SVG**（`.chart.daily-chart` / `.daily-bar` / hourly `.chart`），**不是**比稿的 `.bar` div——其樣式（`.daily-bar.is-today`=accent、`.is-strong`=ink-900、grid/axis/tooltip）已在 styles.css 既有，維持即可，只需確保容器間距與新版面協調；月熱力圖 `.hm*` 與 tooltip `.chart-tip` 既有樣式不動。
- `.rows`(gap18)+`.row .meta`(13px)+`.row .nm`(500)+`.row .vl`(muted/600/tabular)、`.track`(2px `--g1`)+`.track i`(`--g3`)、`.row.top .track i`(`--accent`)。
- `.donutsec`(flex,gap20,align-center)+`.legend`(11.5px，`i`8px方點2r，`b`右對齊/text/600/tabular)。donut 段色走灰階 ramp（由 301 inline style 給；CSS 僅排版）。
- `.comp`+`.compbar`(h8,r2,flex)+`.compbar i`+`.complegend`(10.5px/muted wrap，`b`→text/600/tabular)。
- `.footnote`(11px/faint/lh1.6，`b`→dim/600)。
- 清理：舊 `.subtabs`/`.subtabs button` 若 301 廢除則移除；舊 analytics 專屬樣式（統計頁 `.accounts/.acct/.kv/.records` 等已遷移/併入者）清乾淨，勿留死 CSS。

## 收斂雙洋紅（verifier 指出）
Month+Daily 的 Trends 目前有**兩處洋紅**：`.daily-bar.is-today`（styles.css:1207 `fill:var(--accent)`）＋熱力圖 `.hm-cell.hm-today`（styles.css:1424 `background:var(--accent)`）。違「每鏡頭 ≤1 洋紅」。**改法**：保留 daily-bar today 為 Trends 唯一洋紅；熱力圖 today cell 改**中性描邊/外框**（例：`outline`/`box-shadow` 用 `--text` 或 `--ink-900`，`background` 走該格灰階 `--ink-*`），不再用 accent 填。確保 month+daily 視圖洋紅只剩 today 長條一處。

## 雙主題複驗（硬）
- 亮色與暗色（`:root.dark`）各檢一次：hero 大字對比、灰階 donut 段在暗色反轉 ink ramp 下仍可辨、洋紅（亮 #EC4899 / 暗 #F472B6）在兩底都夠、segmented 反白選中兩主題都對、髮絲線可見。

## Build / Verify（commit 前必過）
- `npx tsc --noEmit` 乾淨、`npm test` 綠（CSS 改動不應影響，但跑確認無連帶）。
- 對照比稿 `analytics-C1-detail.html`：380px 下版面、字級節奏、間距、每鏡頭洋紅 ≤1 一致；無橫向溢出。
- 亮暗雙主題各驗。

## 模式宣告
純視覺（CSS-only），不動邏輯/DOM/token。違反範圍白名單＝作廢重來。
