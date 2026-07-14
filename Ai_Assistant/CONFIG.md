# TokenBar 設定與參數規格書

本文件列出 TokenBar 所有會影響行為的設定與內建參數：哪些可以由使用者調整、哪些寫死在程式裡（附檔案位置，改了要重新編譯）。行為細節的唯一真相仍是 `TokenBar UX Spec v3.md`，本文件只做「數值總表」。

## 1. 更新頻率（最常被問）

| 項目 | 數值 | 出處 | 說明 |
|---|---|---|---|
| 排程器輪詢週期 | **15 秒** | `src-tauri/src/lib.rs` `POLL_SECS` | 背景執行緒每 15 秒跑一輪：讀 Codex 本機檔 + 問 Claude + 更新 UI/系統匣 |
| Claude 網路查詢快取 | **180 秒** | `src-tauri/src/providers/anthropic.rs` `REFRESH_SECS` | 15 秒輪詢中，Claude 的 usage API 最多每 3 分鐘真正打一次網路，其餘回快取 |
| 手動更新最小間隔 | **5 秒** | `anthropic.rs` `FORCE_MIN_SECS` | 按 ⟳ 會立刻跑一輪並繞過 180 秒快取，但 5 秒內連按不會重複打 API |
| Codex 資料 | 每輪（15 秒）重讀 | `src-tauri/src/providers/codex.rs` | 讀本機 session 檔，無網路呼叫；但檔案只在使用者跑 Codex 時才會更新 |
| 前端畫面重繪 | 每 1 秒 | `src/main.ts` boot() | 倒數計時、「X 前更新」等純顯示更新，不觸發任何資料抓取 |

> 換句話說：**Codex 數字最快 15 秒更新一次（且要 Codex 有在跑）；Claude 數字自動更新最快 3 分鐘一次，手動按 ⟳ 可立即更新。**

## 2. 手動更新（⟳ 按鈕）

- 位置：展開面板 header（rate 右側），旁邊顯示「X 前更新」（取自 snapshot 的 `updated_at`，每秒刷新）。
- 行為：前端呼叫 Tauri 指令 `refresh_now` → 喚醒排程器立即跑一輪，Claude 快取視同過期（但保留 5 秒防連打下限）。
- 轉圈動畫在下一筆 snapshot 到達時停止（3 秒保險逾時）。
- 瀏覽器 mock 模式：⟳ 只是重發當前情境並刷新時間戳。

## 3. 使用者可調設定（settings.json）

檔案位置：`%APPDATA%\TokenBar\settings.json`（Windows），由面板 ⚙ 設定區讀寫，即時生效（autostart 除外，寫入時套用）。

| 欄位 | 預設 | 說明 |
|---|---|---|
| `allow_token_refresh` | `false` | 允許 TokenBar 更新 Claude OAuth token（設定區下拉：關閉／開啟，改動即時生效、免重啟）。**opt-in**：refresh 會輪替 token，理論上可能影響 Claude Code 登入（已實作原子寫回並實測無礙，仍保持自選）。關閉時 token 過期就顯示「估算」degraded 狀態 |
| `autostart` | `false` | 開機自動啟動 |
| `warn_pct` | `75` | 通知門檻：util% 到達即發「warning」系統通知 |
| `crit_pct` | `90` | 通知門檻：util% 到達即發「critical」系統通知（LOCKED 也算 critical） |
| `compact` | `false` | 展開面板預設用精簡模式（只有額度列表，隱藏分析分頁）；由 header 的 ⊟/⊞ 按鈕切換並自動記住 |
| `providers` | `"both"` | **顯示平台（全域）**：`both`（兩個都顯示）／`claude`（只顯示 Claude）／`codex`（只顯示 Codex）；⚙ 設定區可切、即時生效。作用範圍是**整個 app**：島嶼、面板、系統匣 tooltip、通知、排名、分析頁全部只呈現選定平台，被關掉的平台連 poll／檔案掃描都跳過。<br>**未知值一律「顯示全部」**（`worst`、空字串、大小寫不符如 `CLAUDE`、手改打錯字皆是）——只有完全相符的 `claude`／`codex` 才會縮限，永不產生空畫面。 |
| `island_mode` | — | **DEPRECATED（2026-07-14）**，已被 `providers`取代。只在載入時讀一次做遷移：舊檔有 `island_mode` 且**無** `providers` 時，值搬到 `providers`（`providers` 存在時以它為準）；遷移後不再寫回 settings.json（`skip_serializing`），執行期完全不讀。 |
| `codex_usage_source` | `"local"` | Codex 用量來源：`local` 只讀本機 session 快照（預設，零網路請求）／`live` 讀已登入帳號的即時用量／`auto` 優先即時、失敗才回本機快照。選擇 `live` 或 `auto` 才會進行唯讀網路查詢，不會生成模型回應或輪替權杖。 |

定義：`src-tauri/src/config.rs`。

## 4. 內建參數（寫死，改需重編譯）

### 狀態機門檻（engine.rs）
| 參數 | 值 | 意義 |
|---|---|---|
| `NEAR_PCT` | 75% | util 達 75% → 狀態 Near（黃） |
| `LOCKED_PCT` | 100% | util 達 100% → Locked（紅） |
| `HISTORY_CAP` | 60 筆 | 每個 limit 保留的取樣數（給燃燒率估算用，約 15 分鐘） |

### 通知（lib.rs）
| 參數 | 值 | 意義 |
|---|---|---|
| `NOTIFY_SUPPRESS_SECS` | 1800 秒（30 分） | 同一個 limit 通知後 30 分鐘內不再通知 |

### 燃燒率 / runway（burnrate.rs）
| 參數 | 值 | 意義 |
|---|---|---|
| `MIN_SAMPLES_FOR_RUNWAY` | 3 筆 | 至少 3 個取樣才敢投影「~empty in X」 |
| `IDLE_SLOPE` | 1e-5 %/秒 | 斜率低於此視為閒置，不投影 |
| `DEFICIT_EPS` | 1% | 超前均線 1% 以上才算 in deficit（防抖） |

### Island 顯示切換（ranking.rs）
| 參數 | 值 | 意義 |
|---|---|---|
| `HYSTERESIS_PCT` | 5% | 新的「最危險 limit」要贏過目前顯示者 5 個百分點才換 |
| `MIN_DWELL_SECS` | 45 秒 | 目前顯示的 limit 至少停留 45 秒才可能被換掉 |

### Codex 讀檔（providers/codex.rs）
| 參數 | 值 | 意義 |
|---|---|---|
| `TAIL_BYTES` | 512 KB | 只讀 session 檔尾端，避免每輪重讀數十 MB |
| `STALE_SECS` | 15 分鐘 | 視窗未到期但檔案超過 15 分鐘沒動 → 顯示 Stale（保留最後已知值）；視窗已過期 → util=0 + Idle |
| `MAX_FILES` | 5 | 最多往回找 5 個最新 session 檔 |

### Claude 資料來源(providers/anthropic.rs)
| 參數 | 值 | 意義 |
|---|---|---|
| 憑證路徑 | `~/.claude/.credentials.json` | 唯讀取 token；**任何情況不得印出/寫 log** |
| Usage API | `https://api.anthropic.com/api/oauth/usage` | 未文件化端點,唯讀,不輪替 token |
| Token API | `https://console.anthropic.com/v1/oauth/token` | 只在 `allow_token_refresh=true` 且 token 快過期時使用 |

### 視窗尺寸與定位（src/datasource.ts、src/main.ts）
| 項目 | 行為 |
|---|---|
| collapsed(island) | 340 × 52(並排模式)/270 × 52(單一模式),邏輯 px |
| expanded(面板) | 寬 380 固定;**高度在進入模式時量一次後鎖定**(展開、切精簡/完整、開關設定時才重算),點分頁與每秒更新**絕不**調整視窗;無捲軸,超出裁切 |
| 分析區 | `#analytics` 固定 300px(實測最高分頁 stats 299px),所有分頁同高 → 切分頁零縮放 |
| 預設停靠 | **右下角**(工作區右下、工作列上方,邊距 8px) |
| 展開方向 | 以視窗右下角為錨點,**向上/向左長**,並夾在工作區內 |
| 拖曳吸邊 | 靠近上/下/左/右邊 40px 內放開即吸附(邊距 8px);以工作區為準,不會蓋到工作列 |
| 精簡切換 | header ⊟(切精簡)/⊞(切完整);精簡 = 隱藏 subtabs/toggles/analytics/tok-min rate |
| 島嶼內容 | 依 `providers`:並排時 Claude/Codex 各取該供應商最危險一條(鎖定>警戒>util 高者),品牌 icon + 膠囊 + %左;右側輔助 = 今日燒速 tok/min(每 60 秒更新)。單一平台時視窗 collapsed 寬 270,否則 340(未知值走並排 → 維持 340) |
| 品牌配色 | Claude = `--claude` 橘 #d97757(星芒 icon、面板分組標題);Codex = `--accent` 紫 #a78bfa + 藍紫漸層雲朵 icon。icon 來源:lobehub/lobe-icons v1.91.0(MIT)官方 SVG,vendor 在 `src/assets/*.svg` 本地打包(不走 CDN,離線可用;Codex 白色底板已移除) |

## 5. 資料來源路徑

| Provider | 來源 | 更新時機 |
|---|---|---|
| Claude Code | usage API（token 來自 `~/.claude/.credentials.json`） | 每 180 秒（手動 ⟳ 可立即） |
| Codex | `~/.codex/sessions/**/rollout-*.jsonl` 最新檔尾端的 `rate_limits` | 只在 Codex 執行時寫入；TokenBar 每 15 秒重讀 |
| Codex（即時／自動） | `https://chatgpt.com/backend-api/wham/usage` | 每 180 秒唯讀查詢一次；手動 ⟳ 可提早查詢（最短間隔 5 秒）。只在設定選擇 `live` 或 `auto` 時使用 |

## 6. 除錯

- `TOKENBAR_DEBUG=1` 環境變數：stderr 每輪印 `[tb]` 各 limit 的 util/status/runway。
- 瀏覽器 preview（非 Tauri）自動進 mock 模式，devbar 可切 safe / near / locked / degraded / stale / empty 情境。

## 7. 發行版外觀一致性

免安裝 exe、NSIS 與 MSI 都由相同的 `dist` 前端資產打包；島嶼膠囊的唯一配色來源是 `src/styles.css`。不得依安裝方式加入不同的 CSS 或程式分支。
