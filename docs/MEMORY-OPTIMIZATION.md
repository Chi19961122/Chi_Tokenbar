# Atoll 記憶體占用分析與優化建議

> 狀態：**規劃檔 v2.3** — **階段 0 + 1A + 1B 已實作（本機）**；2／3 仍待做  
> 日期：2026-07-19（v2.3；0/1A/1B 實作同日）  
> 範圍：`TokenBar-Src/`（Rust backend + WebView 前端）  
> 歷史審查：`docs/MEMORY-OPTIMIZATION-REVIEW.md`（**已凍結**；勿與本檔雙寫）  
> 目的：解釋為什麼記憶體偏高，並依 ROI 排出可做的優化。  
> **共識：** 可當實作依據。**強制：** 先純 baseline 再止血；mutex ≠ single-flight。  
> **下一步：** 階段 2 typed parser（含 Grok 略過巨大 content）→ 再量測 → 必要時 SQLite。

### 證據分級（全文沿用）

| 標記 | 意思 |
|---|---|
| **已確認** | 程式碼路徑或本機量測可直接支持；可作為止血依據 |
| **待量測** | 合理假設，但 peak RSS / Private Bytes / 子進程增量尚未用 release build 驗證 |
| **已修正** | 相對 v1／v2 的錯誤或過滿表述 |
| **工作樹已改** | 已在本地改 code／文件，**尚未 git commit**；勿寫成「已合進／已合併」 |

**本檔定位：**

> **已確認**的重掃效能問題（I/O、JSON parse、allocator churn）+ Share 生命週期缺口 + WebView2 常駐結構成本；  
> 外加若干 **待量測** 的「記憶體尖峰 / 越用越沉」歸因。  
> **不把「讀了 ~500 MB log」寫成「占用 ~500 MB RAM」。**  
> 區分：WebView2 常駐／掃描配置 churn／真正 memory leak（後者 **待量測** 趨勢才談）。  
> Grok 檔量統計正確；sticky model 是正確性旁支（**工作樹已改、待提交**），不改記憶體優先序。

---

## 1. 現況量測（本機 2026-07-19）

### 1.1 行程記憶體（常駐、未開 Share Preview）— **待量測**（樣本，非基線定案）

| 進程 | Working Set（約） | 角色 |
|---|---:|---|
| `atoll.exe` | ~33 MB | Rust host |
| `msedgewebview2.exe`（browser） | ~63 MB | WebView2 主進程 |
| `msedgewebview2.exe`（renderer） | ~76 MB | 頁面渲染 |
| `msedgewebview2.exe`（GPU） | ~51 MB | 合成 / 透明窗 |
| 其他 utility / crashpad | ~15–20 MB | 支援進程 |
| **Atoll 樹 WS 加總（粗估）** | **~230–250 MB** | 參考用，非精確 private |

重點：

- **已確認：** 任務管理員只看 `atoll.exe` 會低估；本機樣本中 WebView2 子進程是 Atoll 常駐大頭。
- **工程判斷（非硬量測）：** ~200 MB 級對 Tauri 2 + WebView2 + 透明小窗屬合理結構成本量級，較不像單純「Rust 漏記憶體」。是否「業界普遍基線」未做跨機／跨版本對照，**不標已確認**。
- **待量測：** 直接加總多個 WebView2 的 Working Set 可能重複計入共享頁面。應同時記 host / 樹的 **Private Bytes**、操作前後 **delta**、重複操作是否單調增長。
- **待量測：** Analytics 掃描與 Share Preview 對 peak RSS 的實際貢獻比例（見 §2）。

### 1.2 本機 session log 體積 — **已確認**（scanner 實際 pattern）

量測會隨 log 增長。下表以審查複核為主；括號為後續抽樣漂移。

| Source（scanner 實際 glob） | 檔案數 | 大小 | 備註 |
|---|---:|---:|---|
| Claude `projects/**/*.jsonl` | 268 | **258.8 MB** | — |
| Codex `rollout-*.jsonl` | 146→147 | **247.5→247.9 MB** | 自然增長 |
| Grok `**/updates.jsonl` **僅此檔名** | 10 | **9.0→9.3 MB** | **已修正** v1 混算目錄內其他 JSONL（全目錄 ~18 MB／50+ 檔，**不進 scanner**） |
| **有效 pattern 合計** | ~424–425 | **~515–516 MB** | **已修正** v1「~525 MB」 |

單檔／單行極端例（參考）：

| 極端 | 約略 |
|---|---|
| Codex rollout 單檔 | **65–69 MB** |
| Claude 單 session | **~28 MB** |
| Grok **單行** `updates.jsonl`（含巨大 content） | 最長約 **360 KB**（**已確認** 2026-07-19） |

Grok 體積遠小於 Claude/Codex，但 **單行極大** → 階段 2 typed serde、略過未用巨大欄位，對 Grok 特別划算（見 §3.2）。

後端註解已寫明重掃代價（`src-tauri/src/lib.rs` → `get_analytics`）：

```text
// The scan re-parses every session log in range (hundreds of MB on a heavy
// machine).
```

### 1.3 依 mtime 閘門，各 range 約會讀的資料 — **已確認**（累計 I/O，不是同時 RAM）

| Range | Claude | Codex | Grok | 合計 |
|---|---:|---:|---:|---:|
| today | 59.2 MB | ~11 MB | ~3 MB | **~73 MB** |
| week | 189.3 MB | ~155 MB | ~9 MB | **~354 MB** |
| month | 258.6 MB | ~236 MB | ~9 MB | **~504 MB** |

若進入 Analytics 後完整 `warmAnalytics` 連抓三 range，約 **~930 MB 累計磁碟讀取**（三次獨立全量掃描疊加）。

**已確認：** 這是顯著 I/O、CPU、短命 allocation 問題。  
**已修正：** 不可寫成「同時持有 930 MB RAM」。Scanner 逐行處理；多數配置在行結束後可釋放，實測 peak 仍 **待量測**。

### 1.4 Grok 資料語意（正確性，非記憶體主線）— **已確認 → 已實作**

GPT 複核 + 本機再驗（2026-07-19）：

| 事實 | 分級 |
|---|---|
| Scanner 只讀 `**/updates.jsonl`（~10 檔／~9 MB） | **已確認**（§1.2 統計正確） |
| Token 累計主路徑：`params._meta.totalTokens` | **已確認**（~1.6k–1.7k 筆級） |
| `modelId` **不在** token 同行；在先前 `params.update._meta.modelId` | **已確認**（同檔 0 筆 co-locate；每檔 model 先於首 token） |
| 舊 scanner 只在「帶 totalTokens 的 `_meta`」找 modelId → 全退 `"grok"` | **已確認**（bug） |
| 舊測試 10/10 假綠：只覆蓋 co-locate 形狀 | **已確認** |
| Token 總量／時間分布大致仍對；**`by_model` 全是泛稱** | **已確認**（修前） |

**修復狀態 — 工作樹已改、待提交**（`src-tauri/src/analytics.rs`；git 顯示 modified，**尚未 commit**）：

1. 每檔 sticky `current_model`（預設 `"grok"`）  
2. 見 `modelId`（優先 `params.update._meta.modelId`）→ 更新  
3. token 行無 model → 用 sticky  
4. 測試：`model update → token`、無 prior 退回 generic、中途換 model  

`cargo test --manifest-path src-tauri/Cargo.toml grok_` → **12 passed**（複核通過）。

**對本檔影響：**

- 記憶體主結論 **不變**（Grok 體積小，不是 I/O 主因）。  
- 階段 2 仍應把 Grok **360 KB 行**當 typed parse 的理由之一。  
- 可選後續：`params.update.usage.totalTokens` 副路徑是否計入（~百筆級）— **未做**，與 model sticky 無關。

---

## 2. 問題拆解（依證據，不是只依直覺尖峰）

### 2.1 Analytics 重複全量掃 JSONL — **已確認（效能）**／**待量測（peak RSS 是否最大）**

**位置：** `src-tauri/src/analytics.rs`（`scan_claude` / `scan_codex` / `scan_grok`）

**行為（已確認）：**

1. `glob` 掃出所有 session 檔。
2. 用 **檔案 mtime** 粗濾：`mtime < range_start` 才跳過整檔。
3. 通過的檔 **整檔逐行讀**（非整檔一次 `read_to_string`）。
4. 候選行 `serde_json::from_str::<Value>` 建成 owned tree。
5. `get_analytics` 在 `spawn_blocking` 跑（不堵 UI thread）；**不同 range 仍可能並行**，互搶磁碟／CPU／記憶體。
6. 結果回前端；前端可 cache 多 slice。

| 問題 | 證據 | 說明 |
|---|---|---|
| 無持久索引 | **已確認** | 每次 today/week/month 從磁碟重算 |
| mtime 粗濾 | **已確認** | 活躍長 session mtime=現在 → 大檔整份重讀 |
| `serde_json::Value` | **已確認** | 每行 owned tree ≫ 幾個 u64 欄位 |
| `BufReader::lines()` | **已確認** | 每行新 `String`；`first_cwd_basename` 反而有 reuse |
| Codex 雙開檔 | **已確認** | head 8 行取 cwd 後再整檔 scan |
| 跨檔 `HashSet` 去重 | **已確認** | Claude `String` keys、Codex `(i64,u64)` 跨 session 累積 |
| `warmAnalytics` 三 range | **已確認** | 進入 Analytics/Share 後觸發；非每次冷啟動必跑，但一觸發 ≈ 三次全掃 |
| 島嶼 60s `refreshToday` | **已確認** | 固定 `fetchAnalytics("today")` 全包；與 warm 可能重疊 |
| 導致 peak RSS / 長期漂漲最大 | **待量測** | 合理懷疑；需 release 操作前後 Private Bytes 趨勢 |

**可能後果（分級）：**

- **已確認：** CPU 尖峰、parse 時間、大量短命 heap allocation（churn）。
- **待量測：** allocator 是否／多久保留容量使 host RSS 不立刻回落（機制常見，**保留量與回落時間**由階段 0 量測，不標已確認）。
- **待量測：** 相對 WebView2 基線，scan 的 peak 增量有多大；「越用越沉」是否存在。

### 2.2 WebView2 常駐基線 — **已確認（結構）**／細部 delta **待量測**

**位置：** `tauri.conf.json` main window + WebView2 runtime

| 設定 / 特性 | 影響 | 分級 |
|---|---|---|
| WebView2 多進程模型 | 常駐大頭 | **已確認** |
| `transparent: true` | 合成／GPU 可能更重 | **待量測** delta |
| 多字型 face | 資產成本 | 次要；延遲載入可評估 |
| 不重寫純原生就難 <100 MB 全家 | 產品現實 | 判斷，非硬指標 |

**目標應是：** 常駐穩定不漂漲；scan／Share 尖峰可控；關閉後可回落（回落時間軸見 §5）。

### 2.3 Share Preview 生命週期 + PNG 表示 — **已確認（缺口）**／第二窗增量 **待量測**

**位置：**

- Rust：`SharePreviewState`、`open_share_preview` / `update_share_preview` / `close_share_preview`
- 前端：`share-panel.ts`、`share-preview.ts`

| 事實 | 分級 |
|---|---|
| 主窗 rasterize → data URL → Rust 保存 → Preview 再 clone 給 `<img>` | **已確認** |
| base64 比 binary 大約 33%；story `@3×` 可到數 MB 級字串 | **已確認** 機制；大小 **待量測** |
| `close_share_preview` 只 destroy window，**未** `preview.clear()` | **已確認** |
| Preview 點擊／Esc 走 `getCurrentWindow().close()`，**不經** `close_share_preview` | **已確認**（`share-preview.ts`） |
| `save_share_png` 用 `Array.from(Uint8Array)` 展開 number array | **已確認** |
| 第二 WebView 再 +80–150 MB | **待量測**（v1 估太死；WebView2 可能共享 environment） |

### 2.4 前端 render / analyticsCache — **已確認（誘發重掃）**／payload 體積 **非優先**

| 項目 | 分級 | 說明 |
|---|---|---|
| `analyticsCache` 多 slice | **已確認** 誘發 warm；payload 本身 | Analytics 有明確上限（≤30 daily、24 hourly、少量 map、top8 專案）。**多 range cache 通常遠小於 WebView／PNG**；問題是誘發重複 scan，不是先做 LRU |
| 1s tick + `JSON.stringify(settings)` sig | 次要 | CPU/短命分配；小於 log 掃描 |
| `innerHTML` 重建 | 次要 | 已有 sig 節流 |
| 動態 `import("html-to-image")` | 合理 | 首次 share 才載 |

### 2.5 已相對健康（不必優先）— **已確認**

| 模組 | 狀態 |
|---|---|
| `Engine` history | `HISTORY_CAP=60` + 900s 窗 |
| Provider cache | 只 cache `Vec<Limit>`，不 cache response body |
| Snapshot IPC | 小結構，15–180s 輪詢 |
| Scheduler | 單執行緒 loop，無無界 queue |

---

## 3. 優化建議（規劃；記憶體主線尚未改產品行為）

優先級依 **已確認問題 × 風險**。標註尖峰／常駐，以及證據分級。

### 3.0 階段 0：純 baseline（**必須先於 1A**）— **已修正 v2「可並行」**

**禁止** instrumentation 與止血行為變更同批上線後才第一次量測——會失去真正 before/after。

嚴格順序（可很快，**一輪基線就夠**）：

```text
0a  只加 instrumentation（不改產品行為）
0b  release build + 固定情境跑一次 baseline（§5）
0c  記錄 metrics / Private Bytes / WS delta
─── 以上完成後才准 1A ───
1A  最小止血（§3.1）
0d  同一套情境重跑 → before/after
1B  scan coordinator（§3.1.1）
0e  再量測
```

階段 0 **不**與階段 1 並行改行為。可並行的只有：寫 instrumentation code 的同時準備 1A patch，但 **merge／發佈順序** 必須 0 完成 → 1A。

### 3.1 階段 1A：最小止血（**最優先開工**）— **已確認 高 ROI、低風險**

錨點：`src/main.ts` `warmAnalytics`（~L310）明確預熱三 range；60s `refreshToday` 會重要 today。

| # | 改動 | 說明 |
|---|---|---|
| 1A.1 | **停三 range warm** | 只抓目前 range；week/month 第一次點擊再抓。立刻砍掉 ~930 MB 累計讀的大頭 |
| 1A.2 | **Island 不需 aux 不掃** | aux 非 tokens/cost 時 **0** `fetchAnalytics`；需要時仍勿假輕量 API（後端全掃 = 無效） |
| 1A.3 | **Share 全 close 清 state** | 見下「Rust lifecycle 兜底」 |

1A 做完 → **同一情境重測** → 再進 1B。

#### Share 清理：Rust lifecycle 兜底（**首選**）— **已確認**

現況：`share-preview.ts` 裸 `getCurrentWindow().close()`；`close_share_preview`（`lib.rs`）不 `preview.clear()`。

**最佳做法不是只改前端 command：**

1. **Rust 在 Preview window `destroyed`／`close` 事件統一 `preview.clear()`**（系統關窗、Esc、錯誤路徑也不漏）  
2. 前端正常路徑可走「clear + close」command，只是便利，**不是唯一防線**  
3. TTL 或下次 `replace` 前 clear 作第二道  
4. 中期 file-backed preview（§3.4）

### 3.1.1 階段 1B：scan coordinator（mutex ≠ 完成）— **已修正 v2 用語**

**已修正：** 「一個 scan」／「mutex」**不等於** single-flight，也**消不掉**三次昂貴全掃。

只包 mutex 的結果會是：

```text
today 掃完 → week 掃完 → month 掃完
```

仍三次全量 I/O，只是不並行。必須拆成完整 coordinator：

| 機制 | 作用 |
|---|---|
| **Mutual exclusion** | 同時最多一個 full scan（避免尖峰疊加） |
| **Request coalescing** | 相同 `sources\|range` 進行中 → 共用同一個 in-flight future／結果 |
| **Short TTL cache** | 短時間內同 key **不得**再 parse JSONL |
| **Queue policy** | 排隊期間 range 已過時（使用者離開該 tab）→ **不要照跑**；進 `spawn_blocking` **之前**決策 |
| **Derive ranges（可選但最強）** | **一次**掃 month（或 union 窗）建日級 aggregate，再衍生 week/today — 才真正消除重複讀取 |

**取消語意：** `spawn_blocking` 開始後很難真 abort。策略 = **進 blocking 前**決定跑／不跑／合併；不要事後 abort 當主路徑。

前端 exact-key inflight fold **不夠**：擋不住 today refresh 與 week/month 交錯、也擋不住後端並行 `spawn_blocking`。

### 3.1.2 掃描閘門（正確性，索引階段才深做）— **已修正 v1 危險建議**

**禁止**單靠 Codex 路徑日期 `sessions/YYYY/MM/DD/` 整目錄 skip。  
舊日期 session 可能跨日 append。安全 watermark：path identity、size、mtime、last offset、必要時 hash、truncation/rewrite 偵測。目錄日期僅 **提示**。

### 3.2 階段 2 解析器降本 — **已確認 機制**；RSS 收益 **待量測**

| 改法 | 說明 |
|---|---|
| typed `serde` struct，忽略多餘欄位 | 不要完整 `Value` tree |
| 能 borrow 則 `#[serde(borrow)]` | 降 owned String |
| 全 scanner `read_line(&mut buf)` | 重用 buffer，停用 `lines()` |
| Codex cwd + token **單 pass** | 去掉雙開檔 |
| **Grok：略過巨大 content** | 真實 token／chunk 行最長約 **360 KB**，多為 `content.text`；只要 `totalTokens`／`modelId`／`timestamp`。typed／欄位過濾收益高 |
| **Grok sticky model** | **工作樹已改、待提交**（§1.4）；階段 2 勿回退成「只讀 token 同行 modelId」 |
| 維持字串候選過濾後再 JSON | `"usage"` / `token_count` / Grok `"totalTokens"`／`"modelId"` |
| 時間窗外 **early-continue** | JSONL 未必嚴格時間序；勿貿然 early-break |
| 保持單執行緒 scan | 勿 rayon 全檔並行（記憶體 ×N） |
| 補測試 | malformed、truncated、rewrite、cumulative reset、duplicate、跨日 session；**Grok 分離 model/token 形狀已有測** |

**`simd-json` 不列第一步。** typed serde + 再量測後，parser 仍是瓶頸再評估 dependency。

### 3.3 階段 3 增量索引 — **治本**；排在止血與 parser **之後**

重掃在階段 1–2 後仍不可接受，再導入持久索引。  
**不建議**未止血就直接上 SQLite。

```text
原始 JSONL  ──(首次/增量 watermark)──►  本地 aggregate store
使用者切 range  ────────────────────►  只讀 aggregate，O(天) 而非 O(檔案大小)
```

較穩健的最小模型（SQLite 合理）：

| 表 / 概念 | 內容 |
|---|---|
| `files` | provider、path key、size、mtime、last offset、tail/prefix hash、last cumulative counters |
| buckets 或精簡 events | 畫面需要的 day/hour、provider、model、agent、project basename、kind、token breakdown、cost |
| `dedup` | 必要時只存 request/event ID **hash** |
| 一致性 | watermark 與資料寫入 **同一 transaction** |

檔案縮小／重寫／fingerprint 不一致 → 刪該檔衍生資料並完整重建。

另需定義：timezone/DST、provider schema 升級、index migration、使用者「清除／重建索引」、index 存放路徑與隱私。

sidecar 每檔 summary、或進程內 + 磁碟 snapshot，可作較輕替代；跨檔 query 與一致性較弱。

### 3.4 階段 4 Share 與 WebView — 生命週期 **已確認**；移除第二窗 **待量測後決定**

優先 file-backed preview：

1. raster 寫入 app cache temp PNG  
2. Preview 只收受控 file／asset 識別（勿長時間存 base64）  
3. close/destroy 刪 temp + clear state  
4. 預覽 1×；正式 story 匯出才 3×  
5. 確認 Tauri 版 binary IPC 後，再定 `Uint8Array` 直傳或前端寫檔（停用 `Array.from` number 陣列）

是否拿掉第二 WebView：依實測增量與 UX，**不宜預先假定必須重做**。

### 3.5 階段 5 常駐微調 — 最後，收益預期小於止血

僅在上述完成後評估：

- `transparent` 的 GPU／WS delta  
- analytics／share chunk 延遲載入  
- Playfair 等 share 字型延遲載入  
- WebView2 runtime 版本差異  

不建議：拿掉 tray／single-instance；換成 CEF；為省記憶體重寫 Win32/Iced。

### 3.6 產品策略（可選，需拍板）

| 選項 | 取捨 |
|---|---|
| 預設 range = week | 降默認成本 |
| 分析回溯天數上限（7/30/90） | 使用者可控 |
| 背景低優先建索引 | 首次慢、之後快 |
| 排除超大 session／歸檔目錄 | 進階 |

### 3.7 低優先／暫不動

- `Engine` history cap  
- Provider HTTP cache  
- 1s countdown timer 微調  

---

## 4. 建議實作順序（v2.3 定稿）

```text
0   instrumentation（不改行為）→ release 純 baseline
1A  停 warm + Island 條件 refresh + Share Rust lifecycle 清 state
    → 同一情境重測
1B  scan coordinator：mutex + coalesce + TTL cache + queue policy
    （可選：month 一次衍生 week/today）
    → 再量測
2   typed streaming parser + buffer reuse + Grok 略過巨大 content
    → 再量測
3   仍慢才 SQLite／持久增量索引   ← 先別急
4   file-backed Share／binary IPC；第二窗去留看數據
5   透明窗／字型／chunk 常駐微調
```

| 階段 | 項目 | 預期 | 風險 | 證據 |
|---|---|---|---|---|
| **0. 純 baseline** | metrics only（`TOKENBAR_DEBUG` 掃 log）+ release 取樣 | 可對比 before/after | 低 | **instrumentation 已做**；正式 before 數字可選補跑 |
| **1A. 最小止血** | 停 warm；Island 不需 aux 不掃；Share destroy 清 state | 立刻砍無意義工作 | 低 | **已實作** |
| **1B. Coordinator** | exclusion + coalesce + TTL + queue；非裸 mutex | 消並行尖峰與重複 key 重掃 | 中 | **已實作**（`scan_coord.rs`；month 全量衍生 week/today 仍可選未做） |
| **2. Parser** | typed serde、reuse buffer、Codex 單 pass、Grok 大行 | 降 churn | 中 | 機制 **已確認** |
| **3. 索引** | watermark + buckets | 重掃消失 | 中高 | 1A/1B/2 後再決策 |
| **4. Share 載體** | file-backed、預覽 1× | 關窗不留 PNG | 中 | 清理 **已確認** |
| **5. 常駐** | 透明／chunk／字型 | 小幅 | 低–中 | 最後 |

**強制規則（v2.3）：**

1. **先一輪純 baseline，再改行為**（不要 0∥1 同時改行為）。  
2. **Mutex ≠ single-flight ≠ 消重複讀取**；1B 要完整 coordinator。  
3. **先別急 SQLite**；watermark／rewrite／跨日 append／DST／migration 正確性成本高。  
4. **最值得立即開工 = 1A**，不是 SQLite。

**最優先（不是 SQLite）：**

1. 階段 0 純 baseline  
2. 階段 1A：停 warm + 無效 Island refresh + Share Rust lifecycle  
3. 階段 1B：真正的 scan coordinator（含 cache／coalesce／queue）  

---

## 5. 驗收標準

絕對 MB 目標（如「閒置 ≤220 MB」）在階段 0 完成前 **暫不定**（**已修正** v1 假精密）。先採行為與相對標準。

| 指標 | 建議標準 | 分級 |
|---|---|---|
| Baseline 可比 | 1A／1B 前後用 **同一套** §5 情境 + 同一 instrumentation | **強制** |
| 重複相同 range | cache hit 不得重新 parse JSONL | 1B；**已確認** 可驗 |
| warm 三 range | 不得三次獨立全量掃描 | 1A；**已確認** 可驗 |
| 並行掃描 | 同時最多一個 full scan | 1B exclusion |
| 過時請求 | 進 blocking 前丟棄；不依賴 abort | 1B queue |
| 60s Island | 不需 aux 時 0 scan | 1A |
| Share lifecycle | Esc／系統關／command 關後 state 皆無 data URL | 1A；Rust 兜底 |
| Share 趨勢 | 開關 10 次 host Private Bytes **不單調增長** | **待量測** |
| indexed range query | backend p95 < 50 ms；含 IPC/UI p95 < 100 ms | 階段 3 後 |
| Parser 正確性 | malformed／truncated／rewrite／reset／duplicate／跨日有測 | 階段 2 |
| Grok `by_model` | 真實分離事件記到實際 `modelId` | **工作樹已改、待提交** |
| 記憶體回落 | 10／30／60 秒趨勢；勿只信單次 10s WS | **待量測** |

### 階段 0 量測情境（release build）

1. 啟動後閒置 5 分鐘  
2. today、week、month 各一次  
3. 三 range 連續切換  
4. 等 10 次 island refresh  
5. Share Preview 開關 10 次  
6. 每次操作後 10、30、60 秒取樣  

### 建議診斷欄位（opt-in / `TOKENBAR_DEBUG`）

- `files_considered`、`files_read`、`bytes_read`、`lines_read`  
- `candidate_lines`、`json_lines_parsed`、`elapsed_ms`  
- `range`、`sources`、cache hit/miss  
- 記憶體由**獨立取樣器**記 host + process tree 的 WS／Private Bytes（scanner 內單一 `peak_rss` **不能**代表 WebView2）

---

## 6. 根因一句話（修訂）

> **常駐高（已確認結構）：** WebView2 進程模型 + 透明常駐窗，屬預期成本。  
> **已確認可修的痛：** 對數百 MB 級 Claude／Codex JSONL 做無索引、可並行、可 warm 三次的全量 `serde_json::Value` 掃描 → 大量 I/O、parse、allocator churn；Share PNG 多份表示且關閉路徑未清 state。  
> **待量測：** 上述是否即「記憶體尖峰／越用越沉」的最大單一原因，以及第二 WebView 的實際 MB 增量。

---

## 7. 相關程式錨點

| 主題 | 位置 |
|---|---|
| Codex scanner | `src-tauri/src/analytics.rs`（`scan_codex` 等） |
| Claude / Grok scanner | 同檔 `scan_claude` / `scan_grok` |
| Grok sticky model | 同檔 `scan_grok_lines`、`grok_model_id_from_value`、`grok_token_from_value` |
| `Value` 解析 | `codex_token_event`、`scan_claude_lines`、Grok 仍 `Value`（階段 2 再降） |
| Codex cwd 雙開 | `first_cwd_basename` |
| blocking analytics | `src-tauri/src/lib.rs` `get_analytics` |
| 前端 fetch / warm / 60s today | `src/main.ts` |
| Share state / 第二窗 | `src-tauri/src/lib.rs` |
| Preview 裸 close | `src/share-preview.ts` |
| rasterize / `Array.from` | `src/share-panel.ts` |
| 透明主窗 | `src-tauri/tauri.conf.json` |
| Engine history | `src-tauri/src/engine.rs` |

---

## 8. 非目標

- 不改產品文案／設計  
- 本檔是規劃；**記憶體階段 0–5 仍待實作**  
- Grok sticky：**工作樹已改、待提交**，旁支正確性，不塞進記憶體 backlog  
- 不比較 Electron／純原生大重寫（除非產品目標變成常駐 <80 MB）  
- **REVIEW 檔不再當活文件**（見 §10）  

---

## 9. 下一步（定稿預設）

| 選項 | 範圍 |
|---|---|
| **A. 0 → 1A**（**立即開工**） | instrumentation + 純 baseline → 停 warm／Island 條件 refresh／Share Rust lifecycle → 重測 |
| **B. A + 1B** | scan coordinator（exclusion + coalesce + TTL + queue；可選 month 衍生） |
| **C. B + 階段 2** | typed parser + Grok 略過巨大 content → 再量測 |
| **D. 仍慢才索引** | SQLite watermark — **先別急** |
| **E. Share 載體／常駐** | file-backed、第二窗、透明窗等 |
| ~~Grok by_model sticky~~ | 工作樹已改；提交後結案 |

**預設路徑：A → 重測 → B → 重測 → C → 必要時 D。**

一句話：**方向正確；先純 baseline；mutex 不是 single-flight；最值得立刻做的是 1A，不是 SQLite。**

---

## 10. 文件角色

| 檔案 | 角色 |
|---|---|
| **`MEMORY-OPTIMIZATION.md`（本檔）** | **唯一活規劃／實作依據**（v2.3） |
| `MEMORY-OPTIMIZATION-REVIEW.md` | **歷史審查紀錄**（凍結）；論證過程保留，日後勿雙寫以免漂移 |
