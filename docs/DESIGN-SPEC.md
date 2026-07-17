# DESIGN-SPEC — TokenBar v0.6 視覺改版（方向 D 極簡編輯部）

> 這份是「合同」：跟任何文件、圖、skill 打架時，以這份為準。所有數值**從 v0.app 匯出碼抽**（非 Stitch；圖只拿來理解意圖）。
> 行為/狀態機仍歸 `Ai_Assistant/TokenBar UX Spec v3.md`；本檔只管視覺。

## 來源

* 匯出碼路徑: `design/stitch/export/token-bar-overlay-design/`（v0.app 產出，Next.js + Tailwind v4）
* 關鍵檔: `app/globals.css`（tb-* tokens）、`components/tokenbar/*.tsx`（10 個元件）
* 選定方向: **D**（= PLAN chosen_direction；文字版 design system 見 `design/refs/ds-D.md`）
* 參考圖: `design/stitch/export/token-bar-overlay-design/public/final.png`、`refined.png`

## Design Tokens

### 色（hex 全部抽自 globals.css `--color-tb-*` 與元件內聯值）

| Token | 值 | 用途 |
| --- | --- | --- |
| bg | `#FAFAFA` | 面板底（**不透明**，捨棄現行玻璃血糊） |
| surface | `#FFFFFF` | 卡片、tile 底 |
| ink（text） | `#09090B` | 主文字、重點數字 |
| primary | `#18181B` | 深色塊（logo 方塊、accent tile 底、圖表深柱） |
| text-secondary | `#52525B` | 次要文字（detail、編輯部斜體句） |
| text-muted | `#71717A` | 三級文字（label、legend） |
| text-faint | `#A1A1AA` | kicker、caption、stale |
| border | `#E4E4E7` | 髮絲線、卡片框（全案唯一框線色） |
| accent | `#EC4899` | 粉紅點綴：today 標記、頂部 logo 閃電、每視圖**最多一處** |
| status-safe | `#16A34A` | 綠 |
| status-near | `#D97706` | 琥珀（near limit） |
| status-locked | `#DC2626` | 紅（locked / destructive 共用） |
| status-stale | `#A1A1AA` | 灰（與 text-faint 同值） |
| chart-scale | `#F4F4F5 → #D4D4D8 → #A1A1AA → #52525B → #18181B` | 熱力圖 5 級／圖表灰階（3D 柱同色階） |

### 字體

| 角色 | 匯出碼 | 落地決策 |
| --- | --- | --- |
| sans（一切文字與數字） | Inter（400/600/700/800，tabular-nums） | **以既有 bundle 的 Geist 承接**（幾何近似 Inter、零新資產、離線鐵則）；若你堅持像素級 Inter 再另開票 subset |
| serif italic（編輯部點綴） | Playfair Display Italic | **新增本地 subset**（僅 italic、僅 latin，仿 `gen:noto` 腳本產 woff2）；只用於 kicker 句與「left」字樣 |
| mono | （匯出碼未用） | Geist Mono 保留給 devbar/debug，不再用於面板數字（改 sans tabular-nums） |

### 字級（全部抽自元件內聯值）

| Token | 大小 / 行高 / 字重 / 字距 | 用途 |
| --- | --- | --- |
| hero | 60px / 0.82 / 800 / -0.055em，色=狀態色 | GaugeRow 剩餘 % 巨大數字 |
| hero-unit | 15px / 1 / 700 | 「%」後綴 |
| hero-left | 13px serif italic / text-faint | 「left」字樣 |
| tile-value | 22px / 1 / 800 / -0.04em | StatTile 數值 |
| section-title | 13px / 700 / uppercase / +0.16em | 「LIMITS」「USAGE」 |
| editorial | 15px serif italic / text-secondary | 區段副句（如 What's left in the tank） |
| name | 11px / 600 / uppercase / +0.12em | 工具名（CLAUDE CODE） |
| pill | 11px / 700 / tabular | 頂部「62% left」 |
| label | 10px / uppercase / +0.12em / text-faint | 圖表小標（DAILY TOKENS） |
| caption | 9px（kicker 用 uppercase +0.16em） | kicker、reset 句、legend、軸標 |

### 間距階（抽自 Tailwind 類）

| Token | 值 | 用途 |
| --- | --- | --- |
| space-1 | 4px | 微間隔（legend、dot gap） |
| space-2 | 8px | 元件內小距（tile gap、icon gap） |
| space-3 | 12px | tile 內距 |
| space-4 | 16px | 列垂直距（pill py） |
| space-5 | 20px | **面板左右 gutter（px-5，全案統一）** |
| space-6 | 24px | 卡片垂直距（GaugeCard py-6）、欄間距 |
| space-7 | 28px | 區段頂距（SectionHeader pt-7） |

### 圓角、陰影

| Token | 值 |
| --- | --- |
| radius-sm | 2px（熱力格、bar chart 柱 1px 可併入） |
| radius-md | 6px（tile、icon 按鈕） |
| radius-lg | 10px（面板外框）；膠囊用 full |
| shadow-panel | `0 8px 40px rgba(0,0,0,.10), 0 1px 4px rgba(0,0,0,.05)`（全案唯一陰影；卡片一律 1px border 無陰影） |

## 共用元件清單（名稱對齊匯出碼元件）

| 元件 | 狀態要寫齊 | 備註 |
| --- | --- | --- |
| StatusPill | safe / warn / locked / stale | 膠囊底=`{狀態色}12`、框=`{狀態色}30`、ping 慢脈動點（2.5s）；左側 24px 黑方塊 + 粉紅閃電 |
| GaugeCard | 見下方矩陣 | 最複雜元件 |
| SectionHeader | 一態 | 編號（01/02）+ 大寫標題 + serif 斜體副句；頂髮絲線 |
| BarChart30 | 有資料 / 空 | 高度 56px、柱距 2px；>60% 深柱 `#18181B`、一般 `#D4D4D8`、today `#EC4899`；軸標僅「30d ago / today」 |
| HeatmapCalendar | 有資料 / 空 / 3D 模式 | 格 10px、gap 3px、5 級灰階、today 粉紅+外圈；legend 右下 less→more；**3D toggle 掛此區塊標題列**（T-feat-005），3D 柱色同 5 級灰階 |
| ActivityDonut | 有資料 / 空（整組不渲染） | 56px SVG、環寬 7、段間 gap 2；段色 `#18181B/#71717A/#D4D4D8/#EC4899` |
| ProjectBars | 有資料 / 空（整組不渲染） | 名稱欄 72px 截斷、軌 3px、第一名粉紅其餘 `#18181B` |
| StatTiles | 一態 | 三 tile：Est.Cost（**反白**：`#09090B` 底 `#FAFAFA` 字）、Peak Day、Streak（T-feat-004 資料） |
| BottomBar | idle / refreshing / shared | 左：綠點慢脈動 + 時鐘（tabular）；右：refresh（轉 360°）/ share（✓ 綠 1.5s）/ 分隔線 / settings；按鈕 28px hover `#F4F4F5` active scale .95 |
| Menu（設定選單） | default / hover / 選中 | **匯出碼未含**：沿本表 token 自組——白底、1px border、6px 圓角、11px 條目、選中=左側 2px 粉紅指示，不得發明新形態 |
| SharePreview（戰報） | 六模板 | 版面結構不動（§0），僅換皮：底 `#FAFAFA`、字色/灰階/粉紅點綴照本表 |

### GaugeCard 狀態矩陣（最複雜元件）

| 屬性 | safe | warn(near) | locked | stale | degraded |
| --- | --- | --- | --- | --- | --- |
| hero 數字色 | `#16A34A` | `#D97706` | `#DC2626` | `#A1A1AA` | 沿最後已知狀態色 + stale 標示 |
| gauge 填色（填**剩餘**） | 同上 | 同上 | 同上 | 同上 | 同上 |
| header 狀態字 | healthy 綠 | near limit 琥珀 | locked 紅 | stale 灰 | 附「degraded」caption |
| 卡片底/框 | 透明 / 底部髮絲線 | 同 | 同 | 同 | 同 |
| gauge 軌 | `#E4E4E7` 3px 圓頭 | 同 | 同 | 同 | 同 |
| 動態 | 填充 700ms transition | 同 | 同 | 無 | 無 |

工具識別：名稱 + 單色 glyph（◆ Claude、○ Codex、其餘工具依現有 icon 單色化 `#18181B`）；**gauge 不再用 provider 家族色**（emerald/tanzanite 廢止，改狀態色示意）。

## 視圖 → 元件對照表（Tauri app，無 route；驗收走 1420 mock）

| 視圖 | 用到的元件 | 區塊順序 |
| --- | --- | --- |
| Island pill | —（**不動**，`--island-*` 原樣保留，CONFIG §7） | — |
| 面板 · Limits | StatusPill → SectionHeader(01) → GaugeCard×N | 頂→下 |
| 面板 · Usage | SectionHeader(02) → BarChart30 → HeatmapCalendar → Donut+ProjectBars 並排 → StatTiles | `#analytics` 300px 契約不變、內部捲動 |
| 設定選單 | Menu | — |
| 戰報 Share | SharePreview | 六模板結構不動 |
| BottomBar | BottomBar | 面板底部固定 |

## 每視圖三態

| 視圖 | loading | empty | error/stale |
| --- | --- | --- | --- |
| Limits | 顯示上次快照（現行行為），數字灰 | 「no data yet」caption + 灰 gauge 軌 | stale：灰化 + stale 標籤；locked：紅 hero + 短標 |
| Usage | 圖表區塊骨架（灰階 `#F4F4F5` 塊） | byKind/byProject 整組不渲染（現行慣例）；daily 空→起始日標註 | 同現行 degraded 註記，色改 text-faint |
| 戰報 | 產圖中 spinner（既有） | 無資料不可開（現行） | toast 文案照舊 |

## 深色模式與 RWD

* 深色模式: **不做**。面板固定 light（`color-scheme: light`）；island 維持既有深色不透明外觀不受影響。匯出碼的 `.dark` 區塊**不採用**。
* RWD: 無斷點。固定 380px 寬桌面視窗；禁止橫向溢出。
* 選單/戰報視窗沿用各自現有尺寸。

## Do / Don't（≤5 條）

* Do: 髮絲線 `#E4E4E7` 是唯一分區手段；數字一律 tabular-nums；粉紅每視圖最多一處。
* Do: 圖表一律灰階 5 級 + 粉紅 today，不上彩色。
* Don't: 漸層、玻璃模糊（面板玻璃廢止）、卡片陰影（只有面板外框有影）。
* Don't: gauge/圖表用 provider 家族色或品牌橘（`--claude` 橘仍僅限 providerIcon）。
* Don't: 動 island、動戰報版面結構、發明匯出碼沒有的新元件形態。

## 對比度自檢

* [x] 主文字 `#09090B` on `#FAFAFA` ≈ 19.2:1 ≥ AA
* [x] 反白 tile `#FAFAFA` on `#09090B` ≥ AA
* [x] hero 狀態色最弱者 `#D97706` on `#FAFAFA` ≈ 4.6:1 ≥ AA（大字，AAA 級距）
* [ ] `#A1A1AA` caption on `#FAFAFA` ≈ 2.6:1 —— **僅限 9-10px 輔助字**，不得用於承載必要資訊（必要資訊至少用 `#71717A`）

## （情境 B 部分）舊 → 新對照表

> Wave2 票照這張表機械替換 `src/styles.css` :root，不自由發揮。island 區塊整段**不改**。

| 舊 token / 值 | 新值 | 備註 |
| --- | --- | --- |
| `--panel-bg: rgba(18,21,27,.72)` | `#FAFAFA` | 玻璃→不透明 light |
| `--panel-blur: 14px` | 移除（0） | backdrop-filter 一併移除 |
| `--border: rgba(255,255,255,.1)` | `#E4E4E7` | |
| `--hairline: rgba(255,255,255,.09)` | `#E4E4E7` | 與 border 合一 |
| `--card-bg: rgba(255,255,255,.045)` | `#FFFFFF` | 卡片改白底+1px border |
| `--track: rgba(0,0,0,.35)` | `#E4E4E7` | gauge 軌 |
| `--text: #dfe3e8` | `#09090B` | |
| `--text-dim: #8b939e` | `#52525B` | |
| `--muted: #6f7883` | `#71717A`（新增 `--faint: #A1A1AA`） | |
| `--near: #e0a63a` | `#D97706` | |
| `--locked: #e05e58` | `#DC2626` | |
| `--prov-claude: #2fa87e` 等家族色 | 廢止於 gauge/圖表（保留變數供 icon 過渡） | 狀態色取代，新增 `--safe: #16A34A` `--stale: #A1A1AA` |
| `--accent: #3d82d9` | `#EC4899` | focus ring 同步換 |
| `--seg-on-bg/--seg-on-fg` | `#09090B` / `#FAFAFA`（選中反白） | segmented 改編輯部黑白 |
| `--island-*` 全部 | **不動** | CONFIG §7 |
| `--mono/--sans` | 保留；面板數字改 `--sans` tabular-nums | Playfair italic 新增 `--serif` |

## 填完自檢

* [x] 色 / 字級 / 間距 / 圓角陰影全部從匯出碼抽的
* [x] 元件狀態矩陣：StatusPill + BottomBar + GaugeCard（最複雜）展開
* [x] 每視圖三態都有寫
* [x] 深色模式（不做）、RWD（無斷點）寫死了
* [x] Do/Don't ≤ 5 條
* [x] 匯出路徑、參考圖路徑在「來源」節
* [x] 跟 PLAN 的 chosen_direction（D）一致
* [x] （情境 C）新功能入口有對應：3D toggle 掛熱力圖標題列、Streak tile 進 StatTiles
