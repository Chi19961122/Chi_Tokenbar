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
| `always_on_top` | `true` | 視窗是否置頂（設定區勾選：視窗置頂，改動即時生效、免重啟）。預設 `true` 對齊 `tauri.conf.json` 的 `alwaysOnTop`——視窗**建立時一律置頂**，所以 `false` 必須在啟動時由 `lib::run` 的 `apply_always_on_top` 覆寫回來，否則每次重開都會變回置頂。**與 `skipTaskbar: true` 的互動**：關掉置頂後視窗會被其他視窗蓋住，而它不在工作列上，唯一叫回的方法是系統匣選單的 Show / Hide（見下方 §3.1）。 |

定義：`src-tauri/src/config.rs`。

### 3.0 設定區的版面結構（`main.ts` `renderSettings`）

六列曾經平鋪成一張清單，找任何一項都得整份讀完。現在**依「使用者想改的是什麼」分三組**，組標題沿用 `.lsec-head`（額度列表的分組標題）——面板只用一套「分組」語彙，不要再發明第二套。

| 分組 | 內容 | 理由 |
|---|---|---|
| **啟動與視窗** | 開機自動啟動、視窗置頂 | TokenBar 何時出現、會不會被蓋住。**不叫「視窗」**：autostart 講的是啟動不是視窗，標錯正是讓設定找不到的原因 |
| **顯示與通知** | 顯示平台、通知門檻（警戒／危險） | 「你會被告知什麼」：哪些平台要出現、滿到什麼程度才值得打斷你 |
| **資料來源** | Claude 權杖更新、Codex 用量來源 | 數字從哪來。兩列都帶著使用者該權衡的代價（權杖輪替、網路查詢），所以讀起來是一個決定而非兩個無關的下拉 |

- 說明文字階層（**只用既有 token，不引入新色**）：`.snote` = `--text-dim`，中性說明；`.warn` = `--near` 琥珀，只給**真的會咬人**的那列（目前只有 Claude 權杖更新）。全部標琥珀等於全部都不顯眼。
- 版面：`.srow` 左邊 `.slabel`（標題 + 說明直向堆疊、`min-width:0` 讓長說明換行而非撐寬）、右邊控制項靠右。`.srow select` 上限 `max-width:170px`，否則最長的「本機 session 快照」會把列撐出 380px 被裁掉。
- `id` 是**契約**：`readSettingsForm()` 靠 `s-autostart`／`s-always-on-top`／`s-refresh`／`s-warn`／`s-crit`／`s-providers`／`s-codex-source` 讀回表單，改版面時不得更名。
- 實測（380px、mock preview）：設定區 310px、`scrollWidth == clientWidth == 378`、無水平捲軸、無任何元素越過面板邊界。高度預算見 §4「設定開啟」。

### 3.1 `always_on_top` 與系統匣 Show / Hide 的互動

`lib::toggle_main` 的決策抽成純函式 `lib::toggle_action(visible, focused)`（可測；視窗 API 在 `cargo test` 下無法驅動）：

| 視窗狀態 | 動作 |
|---|---|
| 可見且有焦點（在最前面） | `Hide` |
| 可見但無焦點（被其他視窗蓋住） | `Show` + `set_focus`（浮到最上層） |
| 已隱藏 | `Show` + `set_focus` |

判斷條件是「可見**且**有焦點」而不只是「可見」：置頂寫死時兩者等價，但一旦可以取消置頂，被蓋住的視窗仍然 `is_visible() == true`——只看可見會在使用者想叫回視窗時反而把它隱藏，而 `skipTaskbar: true` 表示沒有其他叫回的途徑，使用者得再點一次才看得到。`is_visible()` / `is_focused()` 查詢失敗時一律 fail toward `Show`：寧可多顯示，不可讓視窗無法救回。

### 3.2 島嶼隱藏鈕（只留系統匣）

島嶼右端有一個**隱藏鈕**（極簡「—」minimize 橫槓），hover 島嶼才浮現，按下 → `datasource::hideWindow()` → `getCurrentWindow().hide()`，畫面上只剩系統匣圖示。

- **為何在收合狀態**：擋到畫面的就是島嶼本身，要求先展開面板才能隱藏是本末倒置。面板展開時島嶼隱藏，故此鈕只在島嶼上。
- **叫得回來**：`hide()` 只是隱藏、**絕不可改成 `close()`**。隱藏後 `is_visible() == false` → 系統匣 Show / Hide 走 §3.1 的 `Show` + `set_focus` 分支（`tray_toggle_shows_a_hidden_window` 測試涵蓋）。`skipTaskbar: true` 讓系統匣選單成為**唯一**途徑。
- **不走 Tauri 指令**：`core:window:allow-hide` 已在 `capabilities/default.json` 授權，加指令只是把同一個呼叫再包一層。瀏覽器 preview 無 Tauri → no-op（同 `startWindowDrag`）。
- **與點擊展開／拖曳移動的共存**：路由集中在純函式 `island::islandIntent(target, dragged)`（`src/island.test.ts` 涵蓋）：

  | 情況 | 結果 |
  |---|---|
  | 按在隱藏鈕（含其中的 SVG） | `hide` |
  | 按在島嶼其他地方 | `expand` |
  | 這次手勢是拖曳（不論放開在哪） | `none` |

  **`dragged` 必須先判斷**：島嶼很小，拖曳很容易在隱藏鈕上放開；順序寫反會讓「只是想挪開島嶼」變成視窗消失，而唯一救援是系統匣選單。`pointerdown` 落在隱藏鈕時不武裝拖曳，否則 OS 拖曳會吃掉那一次 click。
- **CSS 契約**（`.ihide`）：`opacity` 與 `pointer-events` **必須同進退**——opacity:0 卻可點的按鈕會讓瞄準島嶼的點擊變成隱藏視窗。位置永遠保留（用 opacity 而非 display/width 顯隱），避免 hover 當下島嶼在游標底下重排。實測：靜止與 hover 島嶼寬皆 262.5px（並排模式視窗 340px，餘裕 77.5px）。

## 4. 內建參數（寫死，改需重編譯）

### 狀態機門檻（engine.rs）
| 參數 | 值 | 意義 |
|---|---|---|
| `NEAR_PCT` | 75% | util 達 75% → 狀態 Near（黃） |
| `LOCKED_PCT` | 100% | util 達 100% → Locked（紅） |
| `HISTORY_CAP` | 60 筆 | 每個 limit 保留的取樣數（給燃燒率估算用，約 15 分鐘） |

### 系統匣圖示與 tooltip（lib.rs）

| 項目 | 行為 |
|---|---|
| 圖示 | **app logo，靜態不變色**。`build_tray` 用 `app.default_window_icon()`（來源 `src-tauri/icon-source.png` → `tauri.conf.json` bundle icons）設定一次，之後**再也不動**；`update_tray` 只更新 tooltip |
| tooltip | 每輪（15 秒）更新，列出**每一條** limit（系統匣唯一不只顯示最危險那條的地方）。`SourceFailed` → `估算`、`Locked` → `LOCKED`、其餘 → `X% used` |

- 2026-07-15 前是 `capsule_icon(pct_left, rgb)` 每輪依最危險 limit 重畫的 32×32 燃料膠囊。改成 logo 是**刻意用「一眼看額度」換回通知區的 app 識別度**，使用者已知並接受此代價：數字退到 hover 一下的 tooltip，島嶼仍有彩色膠囊。**不要**以「折衷」為由把依狀態變色的圖示加回來。
- 隨之刪除的死碼：`capsule_icon`、`status_rgb`（只服務膠囊配色；面板／島嶼的顏色一律來自 `src/styles.css`，與 Rust 無關）、`worst()`（只有圖示那行在用）、`tauri::image::Image` import。
- tooltip 組字抽成純函式 `lib::tray_tooltip(snap)`：圖示靜態化後，它是系統匣**唯一**還帶數字的地方，因此值得測（`SourceFailed` 的 `util` 是 0.0 佔位值，印成 `0% used` 會讀成「還很多」）。

### 通知（lib.rs）
| 參數 | 值 | 意義 |
|---|---|---|
| `NOTIFY_SUPPRESS_SECS` | 1800 秒（30 分） | 同一個 limit 通知後 30 分鐘內不再通知 |
| `SOURCE_FAIL_SUPPRESS_SECS` | 21600 秒（6 小時） | 來源失效通知的抑制窗。**刻意不沿用上面的 30 分鐘**：額度警告的數字一直在動、重複提醒有意義；「請重新登入」是要使用者動手的事，修好之前每半小時彈一次只是騷擾。來源恢復時去重 key 會被清掉，所以「壞掉→修好→又壞掉」仍會再通知一次，不必等這個窗到期 |

來源失效通知每個 provider 最多一則（`cc.5h` 與 `cc.week` 會同時失效，逐 limit 發會跳兩則一樣的）。內文直接用 `Limit.hint` —— 與面板共用同一份白話文案，不會走針。

### 重新登入（lib.rs `relogin` 指令）
面板在**登入類**失敗時才顯示「重新登入」按鈕（由後端 `FailureStage::action()` 決定，非前端猜文案）；連不上 Claude 時不顯示，因為按了也沒用、反而誤導。

按鈕啟動官方的 `claude auth login --claudeai`（互動式，會開瀏覽器）。TokenBar **不自行實作 OAuth**：官方有正門，而自行輪替 token 可能把使用者的 Claude Code 登出（見 `anthropic.rs` 開頭）。

已知限制：`claude` 常不在 TokenBar 的 PATH 上（GUI 程式繼承的是檔案總管／開機自動啟動的環境，且 Claude Code 可能整個裝在 WSL）。叫不動時面板會降級顯示 `claude auth login` 指令並附複製鈕，不是跳一個沒有出路的錯誤。

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
| 設定開啟 | `body.settings-open` **收起分析層**(`#subtabs`/`#toggles`/`#analytics`,與 compact 隱藏的是同三個)。額度列表**刻意保留**:顯示平台與通知門檻會即時改變它,看得到自己剛做了什麼;分析頁對設定毫無反應,開著只是高度。實測(safe 情境、完整模式):設定關 753px、設定開 **692px**;若不收起分析層則為 1063px,超過 1080p 工作區預算(~1016px)會被裁掉 |
| 預設停靠 | **右下角**(工作區右下、工作列上方,邊距 8px) |
| 展開方向 | 以視窗右下角為錨點,**向上/向左長**,並夾在工作區內 |
| 拖曳吸邊 | 靠近上/下/左/右邊 40px 內放開即吸附(邊距 8px);以工作區為準,不會蓋到工作列 |
| 精簡切換 | header ⊟(切精簡)/⊞(切完整);精簡 = 隱藏 subtabs/toggles/analytics/tok-min rate |
| 島嶼內容 | 依 `providers`:並排時 Claude/Codex 各取該供應商最危險一條(鎖定>警戒>util 高者),品牌 icon + 膠囊 + %左;右側輔助 = 今日燒速 tok/min(每 60 秒更新)。單一平台時視窗 collapsed 寬 270,否則 340(未知值走並排 → 維持 340) |
| 島嶼隱藏鈕 | 最右端「—」,hover 島嶼才浮現(位置永遠保留,不重排),按下 → 只留系統匣圖示。詳見 §3.2;無資料的空島嶼也有(一樣擋畫面)。島嶼 hover 時整顆不再半透明(`.island:hover { opacity: 1 }`)——淡化是為了沒人在看它的時候 |
| 品牌配色 | Claude = `--claude` 橘 #d97757(星芒 icon、面板分組標題);Codex = `--accent` 紫 #a78bfa + 藍紫漸層雲朵 icon。icon 來源:lobehub/lobe-icons v1.91.0(MIT)官方 SVG,vendor 在 `src/assets/*.svg` 本地打包(不走 CDN,離線可用;Codex 白色底板已移除) |

## 5. 資料來源路徑

| Provider | 來源 | 更新時機 |
|---|---|---|
| Claude Code | usage API（token 來自 `~/.claude/.credentials.json`） | 每 180 秒（手動 ⟳ 可立即） |
| Codex | `~/.codex/sessions/**/rollout-*.jsonl` 最新檔尾端的 `rate_limits` | 只在 Codex 執行時寫入；TokenBar 每 15 秒重讀 |
| Codex（即時／自動） | `https://chatgpt.com/backend-api/wham/usage` | 每 180 秒唯讀查詢一次；手動 ⟳ 可提早查詢（最短間隔 5 秒）。只在設定選擇 `live` 或 `auto` 時使用 |

## 6. 除錯

- `TOKENBAR_DEBUG=1` 環境變數：stderr 每輪印 `[tb]` 各 limit 的 util/status/runway。
- 啟用時，Claude 取數失敗會多印一行 `[tb] anthropic fetch failed: <stage>`，指出**精確**失敗階段。
  面板上給使用者看的是同一階段的白話版本（`user_hint()`），兩者刻意分開：stage 精確但含術語，面板文案不含術語。

  | stage 字串 | 意義 |
  |---|---|
  | `credentials_file` | 讀不到 `~/.claude/.credentials.json`（未登入 Claude Code，或家目錄不存在） |
  | `credentials_shape` | 檔案在，但不是預期結構（JSON 壞掉、缺 `claudeAiOauth`、token 欄位空字串） |
  | `refresh_disabled` | token 已過期，但 `allow_token_refresh` 為 false（預設）→ 誠實降級，不冒輪替風險 |
  | `refresh_http_<code>` | refresh 端點回錯誤狀態碼（`401`/`403` = refresh token 已失效） |
  | `refresh_transport_<kind>` | refresh 連不上（見下方 kind） |
  | `refresh_json` | refresh 回應不是預期 JSON，或缺 `access_token` |
  | `usage_http_<code>` | usage 端點回錯誤狀態碼（`401`/`403` 認證、`429` 頻率、`5xx` 伺服器） |
  | `usage_transport_<kind>` | usage 連不上——**HTTPS 攔截／企業代理最常停在這裡** |
  | `usage_json` | usage 回應不是合法 JSON |
  | `usage_shape` | 連線與 JSON 都正常，但一條 limit 都解不出來 → 官方改了 schema |

  `<kind>`（來自 `ureq::ErrorKind`）：`dns`／`connection_failed`／`proxy_connect`／`proxy_unauthorized`／
  `invalid_proxy_url`／`bad_header`／`bad_status`／`io`／`invalid_url`／`unknown_scheme`／
  `too_many_redirects`／`insecure_request`／`other`。

  **機密**：stage 只帶錯誤種類與 HTTP 狀態碼。新增變體時不得放入 token、email、account id 或 response body
  （`Error::Status(code, _)` 只取 `code`，不碰 body）。
- 瀏覽器 preview（非 Tauri）自動進 mock 模式，devbar 可切 safe / near / locked / degraded / stale / empty 情境。

## 7. 發行版外觀一致性

免安裝 exe、NSIS 與 MSI 都由相同的 `dist` 前端資產打包；島嶼膠囊的唯一配色來源是 `src/styles.css`。不得依安裝方式加入不同的 CSS 或程式分支。
