# Atoll 記憶體最佳化審查補充（歷史紀錄）

> **狀態：凍結歷史審查紀錄（2026-07-19）**  
> **活規劃／實作依據：`docs/MEMORY-OPTIMIZATION.md` v2.3**  
> 本檔保留論證過程，**勿再雙寫**新結論，以免與主文件漂移。  
> 審查對象（歷次）：`docs/MEMORY-OPTIMIZATION.md` v1→v2→v2.2

---

## 0. 移交摘要（讀這段即可）

歷次審查的有效結論已併入主文件 v2.3。其中 v2.3 額外採納的硬性修正：

1. **階段 0 必須先純 baseline**，不可與 1 同時改行為（否則無 before/after）。  
2. **mutex ≠ single-flight**；要 exclusion + coalesce + TTL + queue；month 一次衍生才消重複讀。  
3. **階段 1 拆 1A／1B**；立即開工 1A。  
4. **Share：Rust window lifecycle 兜底清 state**。  
5. Grok sticky：**工作樹已改、待提交**（勿寫「已合進／已合併」）。  
6. **先別急 SQLite**。  

以下正文為歷史複核全文，供追溯。

---

## 1. 結論（歷史）

原文件的主方向正確：目前最值得優先處理的是 Analytics 重複掃描，以及 Share Preview 的 PNG／WebView 生命週期。尤其前端會依序預熱 today、week、month，島嶼又每 60 秒要求一次完整 today Analytics，確實造成大量重複磁碟讀取、JSON 解析與短命記憶體配置。

但目前證據足以確認的是：

- 大量重複 I/O；
- JSON parse 與 allocator churn；
- Share PNG 存在多份表示與生命週期未完整清理；
- WebView2 是常駐記憶體的重要結構成本。

目前證據**不足以直接確認**「Analytics 是記憶體尖峰或越用越沉的最大根因」。Scanner 是逐行處理，不會同時把約 500 MB log 全部載入記憶體；真正的 Private Bytes／Working Set 變化仍需在 release build 中量測。

因此建議把原文件定位調整為：

> 已確認的重掃效能問題，加上待量測驗證的記憶體假設。

長期方向仍可採增量索引，但不建議在完成量測、停止重複掃描與修正 Share 生命週期之前，直接導入 SQLite。

### 1.1 原文件 v2 追蹤結果（歷史）

原文件後續修訂為 v2，已吸收本審查的主要意見，包括：

- 增加「已確認／待量測／已修正」證據分級；
- Grok 改為只統計 scanner 真正讀取的 `**/updates.jsonl`；
- 明確區分累計磁碟讀取與同時 RAM 占用；
- 補上不同 range 可能並行、輕量 endpoint 必須有輕量資料源；
- 補上 Preview 裸 close 不經 Rust command 的生命週期缺口；
- 移除 SQLite 必須優先與第二 WebView 固定增加 80–150 MB 等過滿結論；
- 將絕對 MB 目標延後到 release baseline 完成後再決定。

因此，`docs/MEMORY-OPTIMIZATION.md` v2 現在已可作為後續規劃依據。本文件保留作為獨立複核紀錄，不再表示 v2 仍有 Grok pattern 統計錯誤。

#### 已關閉的小幅措辭建議（v2 後續修訂）

複核時曾指出兩處標級略強；主文件已收斂，**不再阻擋開工**：

1. ~~§1.1「約 200 MB 級屬常見結構成本」標「已確認」~~ → 主文件改 **工程判斷（非硬量測）**；僅本機 WebView2 占比標已確認。
2. ~~§2.1「allocator 保留容量 → host RSS 不立即回落」標「已確認」~~ → 主文件僅 churn 標已確認；保留量／回落時間改 **待量測**。

### 1.2 Grok 語意複核（2026-07-19，第三輪）— **歷史**

重新檢查 Grok 檔量：**只算 `updates.jsonl` 無誤**。另發現 **正確性** 問題（不改記憶體優先序）：

| 事實 | 說明 |
|---|---|
| 10 檔／~9.3 MB | 與 pattern 一致 |
| ~1.6k+ token event | 主路徑 `params._meta.totalTokens` |
| `modelId` 分離 | 在先前 `params.update._meta.modelId`；token 同行 **0** 筆 co-locate |
| 舊 scanner | 只在 totalTokens 同 meta 找 model → 全退 `"grok"`；`by_model` 失真 |
| 舊測試 | 只覆蓋 co-locate，假綠 |
| 最長 token 行 | 約 **360 KB**（巨大 content）→ 強化階段 2 typed／略欄位理由 |

**修法：** per-file `current_model`、update 行更新、token 沿用、分離結構測試。  
**狀態（以主文件 v2.3 為準）：** 工作樹已改、`cargo test grok_` 12 passed、**待 git commit** — 勿寫「已合進／已合併」。

記憶體結論不受影響；Grok 不是 I/O 主因。

### 1.3 第四輪定稿意見（→ 主文件 v2.3）

1. 階段 0 與 1 **不可**同時改行為；先 instrumentation + 純 baseline。  
2. mutex 只是 exclusion；要 coalesce／TTL／queue／可選 month 衍生。  
3. 1A 最小止血 → 重測 → 1B coordinator。  
4. Share 以 Rust destroy/close 兜底清 state。  
5. 先別急 SQLite。  
6. 本 REVIEW 凍結；主文件為唯一活規劃。

---

## 2. 已由程式碼確認的事實

### 2.1 Analytics 掃描

- Codex、Claude、Grok 都先以檔案 mtime 粗略篩選，再逐行讀取符合條件的檔案。
- 候選行使用 `serde_json::Value` 解析；雖非整檔一次載入，但會產生大量短命配置。
- Codex 為取得 cwd，先額外打開檔案讀取開頭 8 行，之後再重新打開並掃描。
- Claude 與 Codex 都有跨檔 `HashSet` 去重狀態。
- `get_analytics` 透過 `spawn_blocking` 執行，因此不會直接堵塞 UI thread，但不同 range 的請求仍可能同時存在，彼此競爭磁碟、CPU 與記憶體。

程式錨點：

- `src-tauri/src/analytics.rs:644`：Codex scanner
- `src-tauri/src/analytics.rs:756`：Codex `serde_json::Value` 解析
- `src-tauri/src/analytics.rs:832`：Codex cwd 額外 head-read
- `src-tauri/src/analytics.rs:884`：Claude scanner
- `src-tauri/src/analytics.rs:930`：Claude `serde_json::Value` 解析
- `src-tauri/src/analytics.rs:1137`：Grok scanner
- `src-tauri/src/lib.rs:126`：`get_analytics` blocking task

### 2.2 前端重複要求 Analytics

- `warmAnalytics()` 會依序抓 today、week、month。
- warm 不是每次 app 啟動都必然執行；它在進入 Analytics 或 Share 流程後觸發。
- Island 啟動時會抓一次 today，之後每 60 秒再抓一次。
- 前端只會折疊「完全相同 key」的請求；today refresh 仍可能與 week／month scan 重疊。

程式錨點：

- `src/main.ts:289`：單一 range fetch 與 exact-key inflight folding
- `src/main.ts:306`：三 range warm
- `src/main.ts:1157`：60 秒 today refresh

### 2.3 Share Preview

- 主 WebView rasterize PNG 成 data URL。
- data URL 被傳入 Rust 並保存在 `SharePreviewState.data_url`。
- Preview WebView 透過 IPC 取得一份 clone，再交給 `<img>` 解碼。
- `close_share_preview` 只 destroy window，沒有清除 state。
- 更重要的是，使用者在 Preview 中點擊或按 Esc 時，前端直接呼叫 `getCurrentWindow().close()`，正常關閉流程未必經過 `close_share_preview`。
- 儲存 PNG 時會把 `Uint8Array` 展開成 JS number array，再經 IPC 交給 Rust。

程式錨點：

- `src-tauri/src/lib.rs:74`：`SharePreviewState`
- `src-tauri/src/lib.rs:293`：取得／clone preview data URL
- `src-tauri/src/lib.rs:322`：關閉 Preview command
- `src/share-preview.ts:27`：Preview 視窗直接關閉
- `src/share-panel.ts:185`：rasterize 流程
- `src/share-panel.ts:265`：PNG 儲存與 `Array.from`

---

## 3. 本機資料量重新核對

首次量測日期：2026-07-19；同日完成第二次追蹤複核。數字會隨 session log 增長而變化。

### 3.1 Scanner 實際會讀取的檔案類型

| Source | 檔案數 | 大小 | 備註 |
|---|---:|---:|---|
| Claude `projects/**/*.jsonl` | 268 | 258.8 MB | 第二次複核未變 |
| Codex `rollout-*.jsonl` | 147 | 247.9 MB | 相較 v2 快照自然增加 1 檔／約 0.4 MB |
| Grok `updates.jsonl` | 10 | 9.3 MB | v2 已修正 pattern；相較 9.0 MB 快照自然增長 |
| **有效 pattern 合計** | 425 | **約 516.0 MB** | 第二次複核快照 |

Grok 目錄內所有 JSONL 在第二次複核時約 52 檔、18.8 MB，但目前程式只掃描名為 `updates.jsonl` 的 10 檔、約 9.3 MB。原文件 v2 已正確反映這個區別；v1 的 18 MB／51 檔是歷史問題，現在已解決。

### 3.2 依目前 mtime 閘門，各 range 會通過的資料

| Range | Claude | Codex | Grok | 合計 |
|---|---:|---:|---:|---:|
| today | 59.2 MB | 11.2 MB | 3.0 MB | **73.4 MB** |
| week | 189.3 MB | 155.3 MB | 9.3 MB | **353.9 MB** |
| month | 258.6 MB | 236.1 MB | 9.3 MB | **504.0 MB** |

若一次進入 Analytics 後完整 warm 三個 range，依第二次複核約會產生 **931.3 MB 的累計磁碟讀取**。這足以確認現有設計有顯著 I/O、CPU 與 allocation churn 問題。

它不代表程式同時持有 931.3 MB，也不能單靠這組數字推導 peak RSS。

---

## 4. 技術注意事項（原文件 v2 已大致採納）

### 4.1 區分 I/O、配置 churn 與常駐記憶體

`BufReader::lines()` 每行建立新 `String`，`serde_json::Value` 再建立 owned tree；每行完成後多數配置可被釋放或重用。實際影響可能包括：

- CPU parse 時間；
- 大量短命 heap allocations；
- allocator 保留容量，使 Rust process RSS 不立即下降；
- 與其他 scan 重疊時的瞬間峰值；
- 磁碟 cache 與 OS working set 變化。

因此應先量測，再判定哪一項是主要問題。不可將「讀取 500 MB」寫成「同時使用 500 MB RAM」。

### 4.2 不要直接以 Codex 目錄日期排除舊 session

Codex 路徑日期可作為提示，但不能單獨作為 skip 條件。舊日期建立的 session 可能跨日或被重新使用；若路徑日期舊但檔案仍有 append，直接跳過會漏資料。

安全做法是使用：

- path／file identity；
- size；
- mtime；
- last parsed offset；
- 必要時 tail／prefix hash；
- truncation 或 rewrite 偵測。

### 4.3 輕量 endpoint 必須有輕量資料來源

只把回傳型別縮成 `tok_per_min` 與 `total_cost_usd`，若後端仍呼叫完整 scanner，I/O 與 parse 成本幾乎不變。

合理方案包括：

- 從已存在的日內 aggregate cache 讀取；
- 透過增量 watermark 更新 aggregate；
- `tok_per_min` 專用 tail scanner，但必須保留 Codex／Grok 累計值的前一個 baseline；
- Island 不顯示 tokens／cost aux 時，完全不要啟動此 refresh。

### 4.4 Share 關閉必須涵蓋所有路徑

只在 `close_share_preview` 增加 `preview.clear()` 不足以涵蓋 Preview 自己關閉、系統關閉或意外 destroy。

建議：

1. 由 Rust 統一處理 close/destroy event 並清除 state；或
2. Preview 前端改呼叫一個「clear state + close window」command；
3. state 再加 TTL 或下一次 replace 前 clear 作防線；
4. 最終改為 file-backed preview，避免長時間保存 base64。

### 4.5 第二個 WebView 的增量需實測

第二個 Preview 一定會增加頁面、renderer/controller 與 decoded bitmap 成本，但未必另起一整套 browser + GPU process；WebView2 可能共享既有 environment。原文件的 `+80–150 MB` 應標示為待量測假設，而不是固定預期。

### 4.6 Analytics 前端 cache 不是優先記憶體來源

Analytics payload 有明確上限：最多 30 個 daily buckets、24 個 hourly buckets、少量 model/agent maps，以及 top 8 + other 專案。多 range cache 本身通常遠小於 WebView 與 PNG。

前端 cache 的主要問題是它誘發 warm scans，不是 payload 留存在 JS heap。除非實測 payload 或 source 組合累積異常，暫時不必優先做 LRU。

### 4.7 Working Set 不應作為唯一指標

直接加總多個 WebView2 process 的 Working Set 可能重複計入共享頁面。建議至少同時記錄：

- 每個 process 的 Working Set；
- Rust host Private Bytes；
- 整個 Atoll process tree 的 Private Bytes／private working set；
- 操作前後 delta；
- 重複操作後是否單調增長。

目前「閒置 ≤220 MB」與「關 Share 後 10 秒回到 ±10%」都應先視為暫定目標，等 release build 基線與自然波動量測完成再定案。

---

## 5–7. 實作順序／驗收／最終建議（歷史）

> **已過時。** 以 `MEMORY-OPTIMIZATION.md` v2.3 §3–§5、§9 為準：  
> `0 純 baseline → 1A 最小止血 → 重測 → 1B coordinator → 2 parser → 再量測 → 必要時 SQLite`。  
> 最優先立刻開工 = **1A**，不是 SQLite；mutex ≠ single-flight。
