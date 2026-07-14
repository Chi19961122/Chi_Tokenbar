# TokenBar — UX 行為規格 v3 (handoff spec)

> 本文件定義**行為、內容、狀態、切換規則**，並記錄已拍板的**視覺方向**。
> 平台：Windows 11。形態：貼邊常駐 widget + 系統匣(tray)圖示。
> 版本沿革見文末「附錄 A · 決策紀錄」。v2 → v3 主要新增：burn-rate 引擎、client×provider 資料模型、第③層 sub-tab 結構、runway/「% left」框架。

---

## 0. 產品意圖(一句話)

讓使用者**不用動腦就知道自己還能不能繼續跑 AI coding 工具**。
價值在「環境感知(ambient awareness)」——**平常近乎隱形，只在該吼時才吼**。

> **靈魂守則(v3 新增)**：TokenBar 是 **runway 監控**，不是**成本儀表板**。桌面英雄數字永遠是「還能跑多久」；lifetime 花費($)只能待在第③層。任何讓開場變成「總共花了 $X」的設計都偏離定位。

---

## 1. 三層視圖模型

```
① 精簡列(常駐)      → 一眼:我快被切斷了嗎?
      ↓ 點擊展開
② 面板(所有限制條)   → 掃視:哪個工具/池吃緊、還能跑多久?
      ↓ 點某條限制 / 切 sub-tab
③ 細節(拆解/趨勢/成本) → 分析:token 怎麼花的、趨勢、花費
```

---

## 2. 精簡列(①)— 單行,只顯示「最危險一條」

### 2.1 內容 slot(由左到右)

```
🟠   Codex·週    12% left    · ~25m
色    是哪條      剩餘        runway
```

- **色點**：狀態色,永不省略(最後的救命信號)。
- **限制標識**：`工具·限制`(如 `Codex·週`、`Claude·5h`)。
- **剩餘框架(v3)**：顯示層講 **`% left`(剩餘)**,更貼「還能跑嗎」;內部 canonical 仍用 `utilization %`(門檻/顏色/排名皆以 util% 計)。
- **runway**：`~Xm / ~Xh`,距耗盡的預估時間(見 §4)。**無法投影時**改回落顯示重置倒數(`· resets 2h13m`)。

### 2.2 空間不足時的丟棄順序(右 → 左)

`runway` → `% 數字` → `限制名` → 只剩 `工具 + 色點`。**色點永不丟。**

### 2.3 音量隨危險度遞增(核心互動原則)

| 狀態 | 精簡列表現 |
|---|---|
| 🟢 **安全** | **近乎隱形**：色點 + `% left`,**不顯示 runway**,低對比(預設透明度 ~0.5,可調) |
| 🟠 **接近** | 補上 runway、狀態色描邊、加陰影浮現存在感 |
| 🔴 **鎖定** | 反白填滿 + 緩慢閃爍 + `LOCKED · resets 1h20m` |

### 2.4 視覺樣式

採 **Bold data** 皮膚：膠囊 pill、`Geist Mono` 數字、狀態色描邊(接近)/填滿(鎖定)。詳見 §13。

---

## 3.「最危險一條」選擇演算法 + 防跳動

### 3.1 排名規則

- **主規則**：純比 `utilization %`,最高者上榜。（**排名用 util%,不用 runway** — runway 是估計值,拿來排名會因雜訊亂跳。）
- **唯一例外**：某條 ≥ 警戒門檻 但**數分鐘內即將鬆開**時降級,避免為即將消失的限制報警。
- 不做「% × 重置時間」加權。

### 3.2 遲滯(hysteresis)— 必做

- **黏性**：目前上榜那條保持顯示,**除非另一條超過它 ≥ 5 個百分點**才換人。
- **最短停留**：換上榜後**至少停留 30–60 秒**才允許再換。

---

## 4. Burn-rate 引擎(runway + 配速)— v3 新增

把「用了 88%」翻譯成使用者真正在意的「還能跑多久」。兩個衍生指標,同一套燃速math。

### 4.1 配速 / deficit（`On pace` vs `X% in deficit`）

視窗有已知 `resets_at` 與長度 `length`(5h / 7d / 月),故可精確定義:

```
已流逝比例  f        = (now − (resets_at − length)) / length
on-pace 應用量        = f × 100%
deficit              = 實際 util% − f × 100%
```

- `deficit > 0` → **in deficit**（燒得比「平均撐完整個視窗」快）。
- `deficit ≤ 0` → **on pace**。
- **可行性**：Claude 與 Codex 兩邊都有 `resets_at` + 已知視窗長度 → **精確可算**。
- 用於固定/半固定視窗(首個請求起算)最乾淨；純滾動視窗為近似,可接受。

### 4.2 Runway（`Projected empty in ~X`）

- **Codex**：有實 token 餘額 + 燃速 → `剩餘 tokens / 燃速 = 時間`,最準。
- **Claude**：只有 util% 樣本(oauth/usage 每 ~180s 一筆)→ 取近期斜率外推到 100%。

### 4.3 誠實約束（**不可被視覺推翻**）

1. **一律標 `~`**,文案表述為「**照目前速度**」——不是保證。
2. **rolling window 會回血**：舊用量滾出視窗後 util 會自動下降。線性外推在使用者停手時**高估危險**,必須是「若持續當前速度」的假設,不可講死。
3. **樣本不足不投影**:需累積 ≥ N 筆樣本(建議 ≥3)且時間跨度足夠才顯示 runway；否則落回顯示重置倒數。
4. **閒置/過期時隱藏 runway**:燃速趨近 0 或資料過期,不顯示「empty in ∞」這種噪音,改顯示 `idle` 或重置倒數。

### 4.4 呈現原則(避免資訊過載)

- **runway 當英雄**,配速當**次要**（用顏色/小字表達,如 in-deficit 時 runway 轉琥珀）。
- **不要**像參考圖三個一起塞(`38% in deficit · Projected empty in 25m · 15% left`)。精簡列一行只給:`% left · ~runway`;配速細節留給面板卡片。

---

## 5. 貼邊列 vs 系統匣圖示(分工不重複)

| 元件 | 顯示內容 |
|---|---|
| **貼邊精簡列** | §2 的完整單行(色 + 名 + % left + runway) |
| **系統匣圖示** | **燃料膠囊(fuel capsule)**：一條會排空的電量/油量條,填滿比例 = 最危險一條的 utilization,顏色 = 狀態色 |
| **hover 系統匣** | tooltip **攤開全部限制條**(唯一不受「只顯示一條」限制的出口) |
| **點擊(列或圖示)** | 展開面板 ② |

### 5.1 系統匣圖示規格(3D 燃料膠囊)

- 形狀：橫向圓角膠囊,外框 track 深灰,內部填色條。
- **安全**：深灰外框 + 綠色填色(填至 utilization%)。
- **接近**：深灰外框 + 琥珀填色。
- **鎖定**：**外框轉紅、內部填白、填滿 100%**（最醒目）。
- 縮到 16px 仍需可辨識；顏色是主要訊號,填滿比例是次要。
- 選配：可疊工具字母(C / X)以區分 Claude / Codex。

---

## 6. 面板(②)— 限制卡片

### 6.1 頂部 tab

`All · Claude · Codex`(依 provider 切換,可擴充,見 §8)。

### 6.2 單張限制卡解剖（2A Tinted bar cards）

```
┌─────────────────────────────────────────┐
│ 5h Session                     🟢 58% left │  名稱(左) + 大等寬剩餘 % (右)
│ ▓▓▓▓▓▓▓▓▓░░░░░░░░░░░░░░░                    │  粗進度條(填 = 已用 util%)
│ On pace · empty in ~2h            [估算]   │  配速 + runway(次要)
│ 620K / 1.2M tokens · resets ~3:40 PM       │  絕對值(有才顯示) + 浮動重置
└─────────────────────────────────────────┘
```

- **卡片背景依狀態上色**：安全=中性、接近=琥珀底、鎖定=紅底(含 `LOCKED` 標籤)。
- **大數字用 `% left`**（右上,狀態色,`Geist Mono`）；**進度條填的是已用 util%**（兩者互補,別衝突)。
- **配速 + runway 行**：`On pace / in deficit` + `empty in ~X`,in-deficit 時該行轉琥珀。runway 不可投影時省略,只留重置。
- **絕對值(已用/上限)是選配**：Claude 訂閱基本**只有 %**(官方 2026 起不公布絕對數字),**版面不得為它預留固定位置**。僅 Codex(token)與 extra credit 有實數。
- **重置文案要誠實**：rolling window 浮動,用 `resets ~` + 相對倒數,**不可寫死固定時鐘**。

---

## 7. 狀態機(所有卡片 / 精簡列都須涵蓋)

| 狀態 | 何時 | 精簡列 | 卡片 |
|---|---|---|---|
| **正常** | 有即時數據 | `🟢 58% left`(近乎隱形) | 中性卡 + 綠色 |
| **接近上限** | 過門檻(75 / 90% util) | `🟠 12% left · ~25m` + 通知 | 琥珀底卡 + 琥珀 |
| **已鎖定** | 撞牆 | `🔴 LOCKED · resets 1h20m` | 紅底卡 + `LOCKED` 標籤 |
| **資料過期** | 快取超時 / 無新活動 | 灰階 + `~` 或「5m ago」 | 灰階 + `~%` |
| **投影不足** | 樣本不夠 / 閒置 | 隱藏 runway,顯示重置倒數 | 配速行改顯示 `resets in …` |
| **來源失效** | Claude 非官方 endpoint 掛掉 | **顯示白話失敗原因 + 標「無法取得」**,不得空白 | 虛線邊 + 白話原因提示 |
| **工具未在跑** | Codex/Claude 沒開 | 淡出該工具,**不顯示成 0%** | 淡出,不顯示成 0% |

> 「來源失效」是本產品特有風險:Claude 即時限制 % 押在未公開 API。此態的降級體驗直接決定產品可信度。

> **既存落差(2026-07-14 修正規格,使其描述真實行為):** 本列原本要求「退回本機 token 估算 + 標『估算』」,
> 但**實作從未做過本機估算** —— `degraded_limits` 只回 `util: 0.0` 佔位值。面板卻標著「估算」,
> 等於告訴使用者那個 0% 是算出來的數字,比不顯示訊息更糟。
>
> 因此改為:降級時**不佯稱估算**,改標「無法取得」並顯示依失敗原因而變的白話提示
> (`FailureStage::user_hint()`,見 CONFIG.md §6 對照表)。狀態機不變 —— 仍是這一個「來源失效」狀態,
> 虛線邊樣式也不變,只是那行文案從寫死的字變成依原因而變。
>
> **真正補上本機 token 估算仍是未做的功能**,記在 backlog;在它實作出來之前,規格與畫面都不得再出現「估算」字樣。

---

## 8. 資料模型：client × provider(v3 新增，架構級)

**陷阱**:一個「客戶端」不等於一份「額度池」。例:OpenCode 會走 **Codex 與 Copilot** 後端,其用量須記到**那些池**頭上。若用「一工具 = 一來源」1:1 模型,一接 OpenCode 就破。

**模型:兩軸分離。**

- **client(客戶端 / 誰發出呼叫)**:Claude Code、Codex CLI、OpenCode、…
- **provider / quota pool(額度池 / 記到誰頭上)**:Anthropic(5h / 週 / 週-Opus / extra-credit)、OpenAI-Codex(5h / 週 / credit)、Copilot、…
- **對應關係為多對多**:一個 client 可打多個 provider;一個 provider 可被多個 client 共用。

**用途**:
- **限制卡(§6)綁 provider**(額度是 provider 的屬性)。
- **用量拆解(§11)可按 client 也可按 provider**(對應參考圖的 `Model ↔ Agent` 切換)。
- 即使 MVP 只做 Claude + Codex,**資料層也要現在就用這兩軸建**,晚改很痛。
- **全域「顯示平台」過濾沿 provider 軸切**(settings `providers`,見 §13.10):在排程器單一節點過濾,§6 面板分組、§5 系統匣、§10 通知、§3 排名、§11 分析頁全部跟著縮限。日後加第三個 provider 時,過濾函式的 catch-all 會讓它預設「顯示」,只需在設定加選項即可。

---

## 9. 限制清單(依 provider · 資料來源真相)

### Anthropic（client: Claude Code）— 4 條

| 限制 | 資料來源 | 有絕對值? |
|---|---|---|
| 5h Session | `GET /api/oauth/usage` → `five_hour.utilization` + `resets_at` | 否(僅 %) |
| 週(全模型) | `seven_day.utilization` + `resets_at` | 否 |
| 週(Opus) | `seven_day_opus` | 否 |
| Extra credit(付費備援池) | `extra_usage`(is_enabled / monthly_limit / used_credits) | **是** |

> 現況(2026)：官方 Settings > Usage 同時顯示 5h 與週進度條,週限制**分別列 Opus only 與其他模型**的重置時間。皆為**滾動視窗**。額度在 Pro/Max **跨 Claude Code / 網頁 chat / Desktop 共用**。官方不再公布絕對數字。
> ⚠️ 前三條全靠**未公開 endpoint**(OAuth token + beta header),可能無預警失效 → 見狀態機「來源失效」。

### OpenAI-Codex（client: Codex CLI、OpenCode…）— 3 條

| 限制 | 資料來源 | 有絕對值? |
|---|---|---|
| 5h 視窗 | 本機 `~/.codex/sessions/**/rollout-*.jsonl` 的 `rate_limits` 快照 | 是(token) |
| 週視窗 | 同上 | 是 |
| Credit 餘額 | ❓ 本機檔通常沒有,需 `platform.openai.com` → 顯示為「connect」態 | — |

> Codex 限制 % **寫在本機檔**,不需未公開 API → 最穩,**建議 MVP 起點**。

### (未來)Copilot 等 — 隨 client 擴充,依 §8 模型加 provider 即可。

---

## 10. 通知規則

- 觸發:門檻(75% / 90% util,可設)各一次 + 鎖定時一次。（**可選**:runway < 10 分鐘時亦觸發一次。）
- 通道:Windows 原生 toast。
- **抑制**:同一條限制觸發後**30 分鐘內不重複吵**。
- 文案可帶省量建議(Codex:切 mini;Claude:/compact、換 Sonnet)。

---

## 11. 細節層(③)— sub-tab 結構(v3 擴充)

資料皆來自本機 JSONL,**穩定無風險**。頂部 sub-tabs(藍本取自參考圖):

`Overview · Daily · Hourly · Models · Agents · Stats`

- **頂部 stat tiles**(每個 sub-tab 共用):`Total tokens` · `Total $ (估算)` · `Best day` · `Active days`。
- **兩個切換(廉價高價值)**：
  - **`Tokens ↔ Price`**:數量 vs $ 成本。
  - **`Model ↔ Agent`**:堆疊維度切換(= §8 的 provider/model vs client)。
- **Daily / Hourly**:逐日 / 逐時長條(**2D,不做 3D** — 見 §14 排除項)。
- **Models**:各模型佔比(Opus vs Sonnet / gpt-5-codex vs mini)。
- **Agents**:各 client 佔比。
- **Stats**:input / output / cached(+ Codex reasoning)拆解、「本週 session 數」(由 5h 視窗推算)、`tok/min` 即時吞吐(次要 vanity 指標)。
- **每工具帳號身份**:如 `user@example.com · Plus`,多帳號時顯示。
- **成本估算**:Codex 走 credit 實計;**Claude 訂閱標「估算 · 訂閱已含」**,呈現「若走 API 會花多少」,不可讓人誤以為訂閱在額外扣錢。

---

## 12. 常駐體驗(Windows)

貼邊(上/下緣可選)、置頂、開機自動啟動、可收合成單行。點某條限制展開細節。
> 「選單列(menu-bar)」是 macOS 概念,Windows 對應為**系統匣(tray)+ 貼邊 widget**。

---

## 13. 不可被視覺推翻的行為約束

1. 精簡列只顯示最危險一條,安全態近乎隱形。
2. 色點永不省略。
3. 絕對值版面為選配,Claude 卡片不得因缺絕對值而破版。
4. 重置時間必須呈現為浮動(`~` + 相對倒數)。
5. 「來源失效」必須有降級樣式,不得空白。
6. 遲滯(±5% / 停留 30–60s)必須實作,避免精簡列狂閃。
7. **runway/配速一律標 `~`、表述「照目前速度」;樣本不足或閒置時隱藏 runway,不投影假數**(§4.3)。
8. **桌面英雄永遠是 runway,lifetime $ 只能待第③層**(§0 靈魂守則)。
9. **排名用 util%,不用 runway**(§3.1)。
10. **「顯示平台」是全域的,且只過濾一次**(settings `providers`,2026-07-14 新增,取代原島嶼專用的 `island_mode`):
    - 選定某平台後,**①島嶼、②面板、系統匣 tooltip、通知、排名、③分析頁全部只呈現該平台** —— 不允許出現「島嶼只有 Claude 但通知還在報 Codex」這種局部一致。
    - 實作上**只在排程器過濾一次**(合併 limits 之後、`engine.ingest()` 之前),下游一律吃過濾後的 Snapshot;**不得**在各消費點各寫一份過濾(必漏)。分析頁是唯一例外 —— 它不吃 Snapshot、直接掃本機 JSONL,必須自行依同一設定跳過掃描。
    - **未知值一律「顯示全部」,永不空畫面**:只有完全相符的 `claude`/`codex` 才縮限;`worst`(舊值)、空字串、大小寫不符皆顯示全部。詳見附錄 A 的 `serde(default)` 紀錄。

---

## 14. 視覺方向(已拍板)

| 項目 | 決定 |
|---|---|
| **整體皮膚** | **Bold data**：深色為主、大號 `Geist Mono` 數字、高對比、狀態色底卡片、粗進度條 |
| **面板卡片版面** | **2A Tinted bar cards**：狀態色底 + 單一大數字 + 粗 meter |
| **系統匣圖示** | **3D 燃料膠囊**：會排空的電量條,顏色=狀態(見 §5.1) |
| **狀態三色** | 安全 `#34d399` / 接近 `#fbbf24` / 鎖定 `#f87171`；過期・來源失效 `#8a929d` |
| **主色** | 綠 `#34d399`(可調) |
| **字體** | `Geist`(介面) + `Geist Mono`(數字/倒數/token) |
| **安全態透明度** | 預設 ~0.5,可調 |
| **深/淺色** | 兩者皆支援,含切換 |

### 排除項(參考圖有、刻意不採)

- **3D 圖表切換**:純裝飾且傷可讀性(堆疊柱狀比高度本就難,3D 更糟)。圖表一律 2D。
- **開場放 lifetime $**:那是成本追蹤器的身份,違反 §0 靈魂守則。

---

## 附錄 A · 決策紀錄

**v1 → v2**
- 限制模型修正:移除「月方案」為獨立限制的誤解;Claude 第四欄改為 `extra_usage` 月度 credit 池。
- 視覺定案:Bold data 皮膚 + 2A 面板 + 3D 燃料膠囊。
- 平台確認:Windows 11(tray + 貼邊 widget)。

**v2 → v3**
- **新增 Burn-rate 引擎(§4)**:runway(`empty in ~X`)+ 配速(`on pace / in deficit`),附精確公式與誠實約束。取自參考圖最有價值的兩個標籤。
- **新增 client × provider 資料模型(§8)**:因參考圖揭露 `opencode also taps: Codex · Copilot`,一工具≠一額度池,兩軸分離,避免日後接多後端破模型。
- **第③層結構化(§11)**:sub-tabs(Daily/Hourly/Models/Agents/Stats)+ stat tiles + Tokens↔Price / Model↔Agent 切換 + 帳號身份 + `tok/min`。
- **框架調整**:顯示層改「`% left`(剩餘 runway)」,内部 canonical 仍用 util%。
- **靈魂守則明文化(§0)**:runway 監控 ≠ 成本儀表板;排除 3D 圖表與開場 lifetime $。
