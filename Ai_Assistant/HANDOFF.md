# HANDOFF — 進度快照(2026-07-17)

## 2026-07-17(晚):v0.6 輪 Wave0+1 — 正確性三連修 + PR 紀錄 + 3D 熱力圖(程式碼完成、未打包)

- **流程**:改走 /webapp-frontend 票據流(docs/PLAN.md、docs/tickets/、docs/DESIGN-SPEC.md);實作全由 codex exec 逐票執行(分離行程,一票一 commit)。
- **T-fix-001 Claude 去重**:scan_claude 全域 HashSet,key 優先序 requestId→message.id→uuid;resume/fork 副本只計一次;無 id 照計。
- **T-fix-002 Codex 增量**:token_count 逐事件「累計轉增量」按事件時間歸屬 daily/hourly(跨午夜修正);(ts,total) 全域防 fork replay,重複事件仍作差分基準;分項(input/cached/output/reasoning)各自差分。舊 tail-read `last_total_usage` 移除。providers/codex.rs 零 diff。
- **T-fix-003 分項計價**:claude_rates vendored 表($/Mtok,快取 2026-06-24;fable/opus/sonnet/haiku 四家族,input/output/cache_read/cache_write 5m+1h);cache_creation 有 5m/1h 細項分別計價,否則當 5m;Codex cached 0.1× 折扣;未知模型 blended fallback;**總量口徑不變、零外連**。
- **T-feat-004 PR 紀錄**:Analytics.records{maxDay,maxHour,streakDays,prNow};maxHour 用獨立 (date,hour) map(hourly[24] 是跨日彙總不能用);prNow 排除本小時比較;stats 子頁三 tile + PR NOW badge;戰報只加 streakDays/maxDayTokens 兩數字(§0 無專案名,已逐行驗)。
- **T-feat-005 3D 熱力圖**:three@0.185 僅 heat3d.ts import、動態載入(主 chunk +0.78KB gzip;three 獨立 chunk 134KB);OrbitControls+raycaster tooltip;on-demand render 無常駐 rAF;切離完整 dispose;WebGL 不可用靜默回 2D 藏 toggle;view 存 localStorage 預設 2d。
- **測試 153 Rust + 81 前端全綠**;票與 Attempts 全記錄在 docs/tickets/。
- **經驗**:codex exec 非 TTY 會等 stdin(要 </dev/null);背景工具 10 分鐘上限跑不完一張票 → nohup 分離 + exit 標記檔 + Monitor。
- 尚未真人驗證:真機 3D 熱力圖(WebView2 GPU/ANGLE)、380px 版面、修正後數字 vs v0.1.2 舊版對比、PR tile 真資料。
- **Wave2 視覺(方向 D 極簡編輯部)待使用者確認 DESIGN-SPEC 四裁決後拆票**;打包待使用者決定。

## 2026-07-17:三樣態優化 階段 E — 多工具(v0.5.0,計畫全階段收官;程式碼完成、未打包)

- **勘察結論(全文在 data-sources-findings.md §4)**:OpenCode 本機未裝(文件化格式:storage/message/*/​*.json,tokens 欄位有;無官方 limit 檔 → 僅 Usage);Gemini CLI 本機無用量檔(~/.gemini/ 是 Antigravity IDE 的 .pb,不採用;預設無 token 落檔;僅 Usage)。**兩家 Limits 判準都不成立,只做 Usage。**
- scanner 依文件化格式實作(oc_record/gemini_record 純函式 + 假資料測試 11 個);執行期目錄不存在回空;**Gemini 只吃 *.jsonl 天然避開 oauth_creds.json**;不以目錄存在列帳號卡(~/.gemini/ 與 Antigravity 共用,會出 0 假卡)。
- settings `tool_opencode`/`tool_gemini`(default true=偵測到就顯示);**tool_* 與 providers 是兩條軸**(client vs 額度池),互不縮限,已註明。agent key 用顯示名(OpenCode/Gemini CLI)與既有一致。
- **順手硬化(補驗 LOW)**:`sanitize_share_filename` 拒絕 Windows 保留裝置名(con/nul/com1…)、ADS 冒號、結尾點;結尾空白由既有 trim 消毒。+1 測試。
- **測試 139 Rust + 75 前端**;C+/D 補跑對抗驗證皆 CONFIRMED。
- 尚未真人驗證:實裝 OpenCode/Gemini 機器上的真實 schema 比對(目前依文件化假設);全計畫的實機驗證清單見各階段段落。
- **打包待使用者決定**(build:release 會 taskkill 執行中的 tokenbar.exe)。

## 2026-07-17:三樣態優化 階段 D — 戰報 Share(v0.4.0,程式碼完成、未打包)

- **入口**:Usage 第 5 個 subtab「Share / 戰報」,渲染進既有 300px `#analytics` 盒(沿用視窗尺寸/locale 重繪機制,零額外佈線)。獨立 `shareCache` 綁 `ui.shareRange`,與分析的 `ui.range` 分離。
- **資料層 `buildShareData(range)`**:純函式;totalTokens/totalCostUsd(恆標 est.)/byAgent(僅 >0)/byModel/periodLabel/可選 quotaNote。**§0 驗證過**:測試在假資料塞 byProject 證明無法滲入輸出;拆分 % =佔區間總量;quotaNote 的限額 % 帶「left/剩」尾綴區隔、預設 off(island_card 預設 on)。
- **六模板 `share.ts`**:視覺依已核可概念稿(share-raw 原檔改造),16:9 1200×675;**9:16 直式列後續**(概念稿已有,計畫本階段不做)。偏離:fuel 第二欄改佔比 %(無 per-model 成本,不捏造);wa 朱印「量」與 CUMULATIVE LEDGER 固定不譯。
- **匯出**:html-to-image(D2,唯一新增前端依賴,lazy chunk)`document.fonts.ready` 後 render;Tauri 走新 command `save_share_png`(檔名消毒:剝路徑、拒 `..`,寫 Downloads);mock 走 `<a download>`;複製走 ClipboardItem。**無新增 Tauri plugin**。
- **設定**:`share_style`(statement)/`share_range`(week)serde default + 遷移測試;選了即記住。**修掉一個既有坑**:`readSettingsForm` 原回傳純 literal,任何設定變更會把不在表單裡的欄位洗掉——已改 `...settings` 展開保留(這對日後每個新設定欄位都重要)。
- **測試 127 Rust + 75 前端**;gen:noto 218 glyphs 64.7KB;mock preview 目視過(statement/wa 卡面、風格/區間切換、Quota line、Save/Copy 鈕)。
- 尚未真人驗證:WebView2 實機匯出的字型嵌入(特別是 zh 卡面)、save_share_png 真實寫檔、剪貼簿在 Tauri 的行為。

## 2026-07-17:三樣態優化 階段 C+ — Usage 進階維度(v0.3.2,程式碼完成、未打包)

- **活動熱力圖**:`heatCells` 純函式(daily 30 桶 → GitHub 式格;首日非週一前置空格),overview 僅 month 顯示;軸標固定英文(同島嶼短標哲學,避語系洩漏)。**無新後端聚合**,直接用階段 C 的日桶。
- **活動類型(勘察結論,寫在 analytics.rs scan_* 註解)**:Claude log 的 `message.content[]` tool_use 有標準工具名 → 可分類 `edit/read/run/other`(每則訊息記單一主類別,tie-break edit>read>run>other);**Codex 的 token_count 是回合累計、與工具事件分開記,無法歸屬 → byKind 不含 Codex、不出假類別**(§計畫硬規定)。donut % 分母=sum(byKind)=已分類總量(單 provider 前提下唯一自洽解讀)。
- **專案維度**:Claude 用 projects/<slug> 目錄名、Codex 用 session `payload.cwd` basename → 兩家都進 byProject;top-8+`__other__` 合併。**§0 硬限制**:types.ts 與 analytics.rs 欄位旁都註明「buildShareData 禁止引用」——階段 D 實作者請遵守。
- **版面**:#analytics 300px 高度契約不變,改內部捲動;byKind/byProject 空 → section 整個不渲染。
- **測試 121 Rust + 59 前端**;gen:noto 189 glyphs 58.9KB。
- **驗證備註**:本階段 fresh-context 對抗驗證因帳號額度中斷,改由 orchestrator 本機完成(四道驗收 + 保護檔零 diff + §0 註解 + 隱私掃描 + woff2);**完整對抗驗證待額度重置後補跑**(重點:heatCells 週對齊邊界、分類 tie-break、top-8 邊界、Windows 反斜線 basename)。
- 尚未真人驗證:真本機 log 的分類分佈與 slug 顯示品質、380px 真視窗的熱力圖/donut 版面、真 Codex cwd 歸屬。

## 2026-07-17:三樣態優化 階段 C — Usage 詳細模式(v0.3.1,程式碼完成、未打包)

- **額度單行摘要**:Usage 頂部 `buildQuotaSummary` 純函式產出(provider 色點+英文短標+% left,pctLeft 與列表同源),點擊展開完整列表;session 記憶不持久化。**設定開啟時強制完整列表**(設定改動要即時反映在額度列表,v0.1.5 驗收行為)。
- **month range**:後端 `range=="month"` → days_back=29(30 個 UTC 日桶),新欄位 `range_start_day`=首個有活動日;前端 `monthStartNote` 僅在起始日≠視窗首日時標「自 {MM-DD} 起」,stackedDaily 裁前導空日。**不新增 command/快取層**,沿用 compute_with(providers 過濾天然生效)。
- **subtab 收斂**:daily→overview(主圖即日堆疊)、models+agents→單一 Breakdown(group toggle 切 Model/Agent);最終 overview/Breakdown/hourly/stats,進 Usage 第一眼=累計總覽。
- **圖表可讀性**:Breakdown 橫條 `512.4M · 65%`(分母=區間總量,`sharePct` 除零安全);日堆疊 hover title(30 根不擠爆)。header 降噪:compact 隱藏 Refresh 倒數。
- **測試 113 Rust + 51 前端**;驗證 CONFIRMED;mock preview 目視過(摘要展開/Breakdown 標籤/Month 30 柱)。
- 尚未真人驗證:380px 真視窗的摘要換行、真本機 log 不足 30 天的起始日註記、展開收合的 fitWindow 高度。

## 2026-07-17:三樣態優化 階段 A+B(v0.3.0,程式碼完成、未打包)

計畫:`三樣態優化計畫-執行版.md`(階段 A、B 已全勾)。分支 `feat/three-modes-v030`,commits c236702(A)/06c6e32(B)+ 驗收修正。兩階段各過 fresh-context 對抗驗證(CONFIRMED)+ mock preview 逐情境目視(safe/near/locked/degraded/stale/empty × zh/en × 倒數/時刻)。

- **階段 A(i18n 雙語回歸,推翻 v0.2.0 全英文)**:`src/i18n.ts` en+zh-TW 各 ~99 key,`satisfies` 編譯期強制等集;`resolveLocale`(system 看 navigator.language,zh* → zh-TW);切語系全量重繪(render 冪等 + 1s tick)。**Noto Sans TC 是精準子集不是全字型**:`scripts/gen-noto-subset.mjs` 從 zh 字典抽實際用到的 CJK(~178 字)+ 日期字,`subset-font`(wasm,免 python)產 54KB woff2;`fonts.css` 以 `unicode-range` 限 CJK → en 模式瀏覽器根本不下載。**改字典後要重跑 `npm run gen:noto`**。後端通知只認 `locale=="zh-TW"` 給中文(Rust 不可靠讀 OS 語系,"system" 一律英文,註解有寫)。島嶼短標(5h/wk/模型短名)固定英文不進字典(D1)。
- **階段 B(島嶼矩陣 + Limits 精簡)**:settings +5 欄位(`expand_default`/`island_pin_claude`/`island_pin_codex`/`island_aux`/`reset_display`,個別 serde default 缺欄不炸)。島嶼決策抽純函式:`pickIslandLimit`(auto=worst;**釘了無資料→null→「—」,絕不靜默退 auto**)、`islandText`(normal=`{left}%` 無短標;near/locked=`{short} {left|0}% · {reset}` —— locked 也帶短標,不然不知道鎖的是哪個視窗)、`fmtResetRel`/`fmtResetClock`(手工雙語日字表、固定 locale;**順手刪了 v0.2.1 洩漏的 fmtClock/fmtReset/fmtHM**)。detail view 與 pace 文案全刪、relogin 留列表列。右鍵選單 `contextmenu.ts`:Tauri 原生(`core:menu:default` 權限)+ DOM fallback(preview 可驗;Escape/外點/失焦都會關)。aux cost_today 走 analytics today、60s 快取、失敗不出 0。
- **測試 110 Rust + 37 前端**;`tsc`/`build` 綠。
- **尚未真人驗證**(mock 驗不到,需 `npm run tauri dev` 或裝新版):① 原生右鍵選單實機行為(DOM fallback 已驗);② zh 通知實際文案;③ Noto 子集在真視窗的渲染;④ 安裝版設定檔從 0.2.1 遷移(缺欄填預設有測試,但真檔案沒跑過)。
- **未打包**:版號已推 0.3.0(package.json/tauri.conf.json/Cargo.toml),`npm run build:release` 未跑(會 taskkill 使用者正在跑的 tokenbar.exe,留給使用者決定時機)。

## 2026-07-15:視窗與外觀四項(v0.1.5 已發佈)

使用者一次提了四項:置頂可選、縮小到系統匣、系統匣圖示改 logo、設定區重新設計。

- **bc20d8b 視窗置頂改為可選**:`alwaysOnTop` 原本寫死在 tauri.conf.json。新增 `always_on_top` 設定,**預設 true 保持現有行為**;視窗建立時一律置頂,故 false 必須在啟動時由 `apply_always_on_top` 覆寫回來。**一併修掉本功能引入的坑**:`toggle_main` 原本「可見就隱藏」—— 置頂寫死時「可見」等於「看得到」,但能取消置頂後,被蓋住的視窗仍是 `is_visible()==true`,而 `skipTaskbar:true` 表示沒有其他叫回途徑,點系統匣反而藏得更徹底。改為「可見**且有焦點**才隱藏」,決策抽成純函式 `toggle_action(visible, focused)`;查詢失敗一律 fail toward Show(寧可多顯示,不可讓視窗救不回來)。
- **d7521a4 島嶼隱藏鈕**:島嶼最右端「—」,hover 才浮現。路由抽成 `islandIntent(target, dragged)`,**dragged 先判斷** —— 島嶼很小,拖曳很容易在隱藏鈕上放開,順序寫反會讓「只是想挪開島嶼」變成視窗消失。**必須用事件委派**:`renderIsland` 每秒重寫 innerHTML,綁在按鈕上的 listener 活不過下一個 tick。
- **d7521a4 系統匣改靜態 app logo**:取代依剩餘量填色的膠囊。**使用者明確選擇「純 logo 不變色」並知悉會失去「一眼看額度」**(有問過,不要自作主張加回變色)。`capsule_icon`/`status_rgb`/`worst()` 已移除。tooltip 抽成 `tray_tooltip(snap)` 並補測 —— 圖示靜態後它是系統匣唯一還帶數字的地方。CLAUDE.md 第 3 行的「系統匣燃料膠囊」已同步更正。
- **d7521a4 設定區三組**:「啟動與視窗」「顯示與通知」「資料來源」(autostart 講的是啟動不是視窗,標錯正是讓設定找不到的原因)。**開設定時收起分析層**:分組後設定區 310px,疊在額度列表+300px 分析區上實測 **1063px**,超過 1080p 工作區(~1016px)會被裁 —— 比舊平鋪版 949px 還糟。改為隱藏分析層後 692px。額度列表刻意保留(顯示平台/門檻會即時改變它,看得到自己剛做了什麼);分析頁對設定毫無反應,開著只是高度。
- **新增前端測試**:vitest + jsdom(devDep,執行期依賴不變)。專案原本零前端測試,`islandIntent` 需要真的 `closest()` 走訪才測得到。指令 `npm test`,見 CLAUDE.md。
- **測試 99 → 104 Rust + 8 前端**。

**尚未真人驗證**(cargo test 驗不到,需實際跑 GUI):① 關掉置頂後視窗真的被蓋住、系統匣叫得回來;② 隱藏鈕按下後從系統匣救回;③ logo 在系統匣的實際外觀;④ 顯示平台三種設定下五處(島嶼/面板/系統匣/通知/分析)一致。

## 2026-07-15:HTTPS 根憑證 + 白話失敗提示 + 主動通知(v0.1.4 已發佈)

起因是使用者在**另一台機器**上 Claude 一律 SourceFailed,但同一份憑證用 curl 打 API 回 200。根因不是 OAuth 失效,而是 `ureq` 走編譯期寫死的 `webpki-roots`、**完全不讀 Windows 憑證存放區**;PowerShell/curl 走 schannel 所以看得到被攔截注入的憑證。有企業代理/防毒 HTTPS 攔截/自簽根憑證的機器就會中。**是環境差異,不是程式邏輯 bug。**

- **50b9460 HTTPS 改用系統根憑證**:`ureq` 加 `native-certs` feature。**注意是「取代」不是「疊加」**(ureq `rtls.rs:62-86`:啟用後從 `RootCertStore::empty()` 開始、只載系統憑證,內建 Mozilla 清單完全不用;`rtls.rs:75` 自帶警告「系統憑證一張都載不到時所有 HTTPS 都會失敗」)。實測 `cargo tree` 出現 rustls-native-certs→schannel。
- **17cb08f 白話失敗提示**:新增 `FailureStage`,同一列舉兩種輸出 —— `label()` 走 `TOKENBAR_DEBUG` stderr(精確、含狀態碼),`user_hint()` 走 UI(白話、不含術語)。狀態機不動,仍是 §7 單一 SourceFailed。**踩雷**:計畫原本的測試用 `unwrap_err()`,那會要求 `Creds: Debug` —— 而 `Creds` 裝著 token,加 `Debug` 正是鐵則禁止的洩漏面;改用 `.err()`,`Creds` 永遠不可列印。另修掉一個謊言:面板原說「改用本機估算」但 `degraded_limits` 只回 util:0.0 佔位值、**根本沒估算**,徽章改「無法取得」。(§7 要求的本機估算實作從未做過,見 backlog。)
- **ee39d06 主動通知 + 一鍵重新登入**:SourceFailed 的 util 恆為 0.0,而 `fire_notifications` 只在 util>=warn/crit 或 Locked 時發 → **原本永遠不會通知**,使用者不開面板就不知道壞了。現在會發,內文共用同一份 hint。抑制窗 6h(非既有的 30 分,那是為額度警告設計的),恢復時清去重 key。按鈕只在**登入類**失敗出現(後端 `FailureStage::action()` 決定,非前端比對文案 —— 連不上時給登入按鈕是誤導)。**已查證**:`claude auth login --claudeai` 是官方子指令(互動式、開瀏覽器),`claude auth status --json` 唯讀且不含 token,**無非互動式登入**;因此不自行實作 OAuth(自行輪替 token 可能把使用者的 Claude Code 登出)。**安全**:自行從 PATH 解析 claude 再直接當 program 執行,不用 `cmd /C start` —— 後者會把路徑當 shell 語法重解析,PATH 含 `&` 的目錄會被切成第二條指令。
- **c108176 runway 灌水修正**(意外發現,比原需求重要):`HISTORY_CAP=60` 只依筆數淘汰、無時間上限,而 `compute_runway` 取 front/back 兩點算斜率 → 任何取樣空窗後 front 是老樣本、斜率被稀釋。實測 2h 空窗後報 11.2 小時、真值 25 分鐘(**26.9 倍**,失效方向最糟)。**筆電闔蓋睡眠**就會觸發,與平台切換無關。修法是把原意圖明確化(60×`POLL_SECS`15=900s,「最近 15 分鐘」本來就是意圖,筆數上限只是取樣規律時的等價代理),新增 `HISTORY_WINDOW_SECS=900` 依時間淘汰;另非 Normal 狀態不寫入 history(佔位值會污染斜率)。正常路徑逐位元不變。
- **測試 33 → 92**。**踩雷**:曾有一輪的新測試是**同義反覆**(斷言述詞而非行為),突變測試證明把判斷式反相後仍 47/47 全過、一個都沒抓到。已重寫為注入式真路由測試。**之後新增測試一律要能通過突變驗證**(改壞實作、確認測試變紅)。
- 實測 v0.1.4 release 產物:Claude(cc.5h 3%/cc.week 13%/Fable 20%)與 Codex(週 0%)均回真值、無 SourceFailed —— 證明 native-certs 在本機沒搞砸。

## 2026-07-14:全域「顯示平台」過濾(island_mode → providers)

- **做了什麼**:原本只作用於島嶼的 `island_mode` 升級為**全域**設定 `providers`(`both`/`claude`/`codex`)。選定平台後**島嶼、面板、系統匣 tooltip、通知、排名、分析頁全部只呈現該平台**;被關掉的平台連 poll(codex_live/codex 本機/anthropic)與分析頁的目錄掃描都跳過。設定 UI 標籤「島嶼顯示」→「**顯示平台**」。行為約束寫入 UX Spec §13.10 與 §8。
- **架構(勿回退)**:**只在排程器過濾一次** —— `lib.rs` 合併兩家 limits 之後、`engine.ingest()` 之前呼叫 `apply_provider_filter`。下游(panel.ts 空分組自動跳過、tray tooltip 與 `fire_notifications` 直接遍歷 `snap.limits`、`ranking.rs` 只從傳入 limits 挑)全部自動一致,**不得在各消費點各寫一份過濾**(必漏一處)。**分析頁是唯一例外**:它不吃 Snapshot、直接掃 `~/.codex/sessions/**` 與 `~/.claude/projects/**`,所以 `analytics::compute_with(range, filter)` 自行依同一設定跳過掃描;`compute(range)` 保留為「顯示全部」的薄包裝。
- **⚠️ 更正舊記載(本檔原第 28 行,「舊存檔值 worst 一律 fallback 成並排」的成因寫錯了)**:那**不是 serde 做的**。`#[serde(default)]` 是**容器層級**屬性 —— 它只在欄位**缺失**時套用 `impl Default`,**完全不驗證值的內容**,`island_mode: String` 會原封不動吃下 `"worst"` 或任何字串。真正在兜底的是 `island.ts` 的 else 分支(非 `claude`/`codex` 一律當並排),**後端從未驗證過這個值**。把過濾搬到後端後那層意外保護就消失了 —— 若 `match` 少了 catch-all,殘留的 `"worst"` 會把兩個平台都濾掉、**整個 app 變空白**。故 `apply_provider_filter` 與 analytics 的 `scans_*` 都有明確 catch-all:**只有完全相符的 `claude`/`codex` 才縮限,未知值(`worst`/空字串/大小寫不符如 `CLAUDE`)一律顯示全部,永不回空**。已有專門的回歸測試擋這件事。
- **舊設定遷移**:`config::load_from_str()`(自 `load()` 抽出的純函式,可測)先解析成 `Value`,遇「有 `island_mode` 但無 `providers`」時把值搬過去再反序列化 —— 舊使用者的偏好不會無聲退回預設;`providers` 存在時以它為準。`island_mode` 保留為 deprecated 欄位但 `skip_serializing`(只讀一次做遷移,不再寫回、執行期不讀)。
- **順手修的兩個地雷**:①`codex_usage_source="live"` + `providers="claude"` 時,跳過 live poll 會讓 `choose_limits` 走 `_ =>` 分支呼叫 `degraded_limits()`,**憑空生出兩條假的 SourceFailed Codex 列**(過濾雖會濾掉,仍已改為不建構);②`main.ts` 的 `collapsedW()` 原為 `=== "both" ? 340 : 270`,未知值會給 270 卻仍渲染並排 → 改為只有完全相符的 `claude`/`codex` 才用 270,與 island.ts 分支一致。
- **測試**:`cargo test` **47/47 通過**(原 33 + 新增 14:config 遷移 4、`apply_provider_filter` 5、analytics 過濾路由 5)。`npm run build` exit 0。
- **⚠️ 已知問題(已量測,尚未修,待使用者決定)**:`Engine.history` 依 limit id 保存且**只以筆數封頂(`HISTORY_CAP=60`)、不以時間封頂**。平台被過濾掉期間該 provider 停止取樣,但舊樣本仍留著;切回來後 `compute_runway` 的 front 仍是空窗前的樣本,**斜率被長基線稀釋 → 燃速嚴重低估、runway 過度樂觀**,且要等 60 筆新樣本(約 15 分鐘)才會被推出去。實測:空窗 2h 後真實 5 分鐘燒 40→50%,**回報 runway 37650s(~10.5h)vs 真值 1500s(~25min),差 25 倍**。修法選項:切換時清掉該 provider 的歷史(比留半截誠實),或給 history 加時間上限。

## 2026-07-13:單一實例鎖(v0.1.3)
- **問題**:重複點啟動會疊出多個常駐實例。**修正**:加 `tauri-plugin-single-instance`(v2),註冊為 **builder 第一個 plugin**(官方要求);callback 在既有實例裡 `show()+set_focus()` main 視窗,第二個實例自動退出。純 Rust 外掛、不需改 capabilities。
- **實測**:先啟動 1 個 tokenbar,再啟動 portable(同 app id)→ portable 立即退出、tokenbar 維持 1 個。已發 Release **v0.1.3**(Latest,含 portable+setup+MSI),0.1.1/0.1.2 安裝檔在 `TokenBar-release/archive/`。安裝版已升 0.1.3 在跑。

## 2026-07-13:修正 Codex 5h 誤標 / 不顯示(v0.1.2)
- **問題**:使用者回報 Codex「5h 沒有 token 顯示」。實測(7/13)發現 Codex 改了 rate_limits schema — `primary`/`secondary` 不再固定對應 5h/週。現況只回**週視窗**(放在 `primary`,`window_minutes:10080`,util 3%),`secondary` 為 null,**完全不回 5h(300)視窗**(今天整個 session 檔 54 筆全是 10080;live API `/wham/usage` 同步:`primary_window`=604800s 週、`secondary_window` null)。舊版硬把 `primary`→「Codex·5h」,於是把週的 3% 誤標成 5h,真正的 5h 反而消失。
- **修正**:`providers/codex.rs` 與 `providers/codex_live.rs` 都改成**依視窗長度分類**(<24h→codex.5h、否則→codex.week),不再依 primary/secondary 位置;snapshot 只有一個視窗時只顯示該視窗(正確標籤),不再湊數。codex.rs 另修:degenerate snapshot(只有 credits、兩窗皆 null)產出空集時繼續往舊檔找,不再直接回空。新增 4 個回歸測試,`cargo test` 31/31 通過。
- **對使用者的答案**:目前顯示「5h 沒 token」是**正解** — Codex 端現在根本沒回 5h 視窗(可能該視窗閒置未觸發);之前看到的「Codex·5h 3%」其實是**週**的數字被誤標。修正後會正確顯示「Codex·週 3%」(reset ~7/20),有 5h 時才會多一條 5h。
- **自動適應強化**(同日追加):解析改為**掃描整個 rate_limits/rate_limit 物件、撈出所有 window 形狀欄位**,不再依 `primary`/`secondary` 鍵名或位置;依 window_minutes 分類(≈300→codex.5h、≈10080→codex.week,其他長度用時長自動命名 codex.min{n} / Codex·{h}h|{d}d,不丟棄)。本機+live 共用 `codex::classify`。cargo test 33/33(含「改鍵名照抓」「未知長度自動命名」兩個未來相容測試)。
- **已 commit + push + 發 Release**:commit 75d8fc0(main);GitHub Release **v0.1.2** 已發佈為 Latest,首次附**免安裝版 TokenBar-portable.exe** + setup.exe + MSI(https://github.com/Chi19961122/Chi_Tokenbar/releases/tag/v0.1.2)。安裝版已靜默升級到 0.1.2 並在跑。
- **collect-installers.mjs 強化**:主資料夾只留當前版安裝檔,舊版(TokenBar_x.y.z_*)自動移入 `../TokenBar-release/archive/`;歸檔在複製後掃描(Tauri bundle 目錄會累積舊版產物,複製前歸檔會被抵銷)。0.1.1 已進 archive/。

## 目前狀態:全部里程碑完成,v0.1.1 已發佈並在跑

- **v0.1.1 發佈**(2026-07-10):GitHub Release https://github.com/Chi19961122/Chi_Tokenbar/releases/tag/v0.1.1(latest),含 setup.exe + MSI。版本號三處(package.json/tauri.conf/Cargo.toml)升 0.1.1,commit dfb450c。v0.1.0 後的新功能:Codex 即時用量來源、Claude 權杖更新下拉即時生效、島嶼固定配色。**踩雷紀錄**:搬移專案目錄後 Rust `target/` 快取含舊絕對路徑會導致 build 失敗(os error 3 讀 permissions toml),需先 `cargo clean` 全量重編(一次性)。
- **容器化目錄結構**(2026-07-10 晚):`C:\Coding\TokenBar\` 現為容器,內含 `TokenBar-Src\`(整個 git repo,即現在的專案根)與 `TokenBar-release\`(安裝檔)。**重要:專案根已從 `C:\Coding\TokenBar` 下移到 `C:\Coding\TokenBar\TokenBar-Src`**,之後開 Claude Code / 編輯器要指到 TokenBar-Src。collect-installers.mjs 用相對 `../TokenBar-release` 不受影響(仍輸出到容器內的 TokenBar-release)。
- **目錄重組**(2026-07-10 晚):repo 內約定為 `src/`+`src-tauri/`(程式碼)、`Ai_Assistant/`(原 docs/,AI 產出文件與規範);CLAUDE.md/AGENTS.md 因工具自動載入需求留在 repo 根目錄。安裝檔在 repo 外同層(`../TokenBar-release`,即 `C:\Coding\TokenBar\TokenBar-release\`);.gitignore 仍保留 `release/` 一行防有人改回。歷史紀錄中的 docs/ 路徑一律讀作 Ai_Assistant/、根目錄 release/ 讀作 ../TokenBar-release/。
- **Codex 即時來源 + 設定整理 + 目錄整理**(2026-07-10 晚):使用者自行實作 codex_live.rs(local/live/auto 三來源,修正本機快照過舊顯示 0% 的問題);Claude 權杖更新改為下拉且**即時生效**(allow_refresh 改為每輪從 settings 重讀,不再需要重啟);AGENTS.md 修復(原為 Claude→Codex 誤植的壞檔);舊規格歸檔至 docs/archive/;新增 `npm run build:release` + scripts/collect-installers.mjs,安裝檔集中到根目錄 release/(gitignored)。

- M0–M7 全部完成(scaffold、Codex provider、burn-rate 引擎、Live Island 視覺、Anthropic provider+降級、面板下鑽、第三層分析、通知/設定/autostart)。
- Polish 完成:island 拖曳+吸邊、tray rich tooltip、綠色方塊 logo(`src-tauri/icon-source.png`)。
- **數值修正完成**(使用者回報數值不準的 bug):Codex 快照過期/陳舊語意,見 CLAUDE.md 鐵則。19/19 cargo 測試通過。
- **手動更新完成**(2026-07-10):面板 header 新增 ⟳ 按鈕(Tauri 指令 `refresh_now` → mpsc channel 喚醒排程器,Claude 快取視同過期、5s 防連打下限)與「X 前更新」標籤(每秒刷新)。參數總表新增 `docs/CONFIG.md`。
- **視窗改造完成**(2026-07-10):預設停靠右下角(work area,避開工作列);展開面板高度自動符合內容(右下角錨點向上長,fitWindow + resizeAnchored);吸邊新增底邊;header 新增 ⊟/⊞ 精簡模式切換(settings.json `compact`,只顯示額度列表)。
- **推上 GitHub + 首個 Release**(2026-07-10):公開 repo `Chi19961122/Chi_Tokenbar`(origin/main),MIT LICENSE(版權人 Chi19961122)+ 重寫 README(電池比喻、實機截圖在 docs/screenshots/)。Release `v0.1.0` 已發佈,含 NSIS setup.exe(2.8MB)+ MSI(3.9MB)。註:工作目錄有一份壞掉的 `AGENTS.md`(Claude→Codex 誤植,未追蹤、未推),待決定修/刪。
- **git 版控啟用**(2026-07-10):main 分支,初始 commit e043b2e(116 檔);島嶼顯示選項移除「自動(最危險)」,僅剩並排/僅 Claude/僅 Codex(舊存檔值 worst 一律 fallback 成並排)。**⚠️ 2026-07-14 更正**:此處當時把 fallback 成因記成 serde 的行為,是錯的 —— `#[serde(default)]` 只補**缺失**欄位、不驗證值,兜底的其實是 `island.ts` 的 else 分支;詳見本檔 2026-07-14 條目。另 `island_mode` 已於 2026-07-14 被全域設定 `providers` 取代。
- **島嶼第三輪微調**(2026-07-10):右側輔助改為今日燒速 tok/min(移除 ↻ 倒數與總量);供應商識別改用品牌 icon,島嶼與面板分組標題都套用;Claude 主題色從綠改為品牌橘 `--claude` #d97757。icon 改用 lobehub/lobe-icons v1.91.0 官方 SVG(claude-color/codex-color),vendor 在 src/assets/ 本地打包、Codex 白底板移除(手繪版已淘汰)。**陷阱已修**:SVG 漸層 id 是文件全域,隱藏的島嶼副本會搶走 id 且 display:none 內的 defs 不生效 → 面板 Codex 雲朵無填色;icons.ts 現在每個實例注入唯一 id 後綴。
- **高度鎖定 + 島嶼強化**(2026-07-10 第二輪回饋):自動縮放改為「進入模式時量一次後鎖定」(展開/切精簡/開關設定才重算),點分頁與每秒 tick 不再 resize → 消除卡頓;#analytics 固定 300px 讓所有分頁同高;移除捲軸(overflow hidden)。島嶼改為可配置(settings `island_mode`,預設 both;**2026-07-14 起改為全域設定 `providers`**):Claude/Codex 並排膠囊(各取該供應商最危險一條)+ ↻重置倒數 + 今日總 tokens(60s 更新);視窗 collapsed 寬 340(並排)/270(單一)。
- **usage API 已改版 + Fable 顯示完成**(2026-07-10):API 新增結構化 `limits` 陣列(session/weekly_all/weekly_scoped),`parse_limits_array` 通用解析(Opus 沿用 `cc.opus`,其他模型 scoped 週限制 → `cc.w.<slug>`,如 Fable → `cc.w.fable`「Weekly · Fable」),舊欄位當 fallback。dev 實測 API 回傳 Fable 6% 正常顯示。23/23 cargo 測試通過(新增 4 個解析測試)。schema 詳見 data-sources-findings.md。
- Release 產物(2026-07-10 打包,`npm run build:release` 自動集中到 `C:\Coding\TokenBar\TokenBar-release\`):
  - `TokenBar_0.1.0_x64-setup.exe`(推薦安裝)
  - `TokenBar_0.1.0_x64_en-US.msi`
  - `TokenBar-portable.exe`(免安裝,常駐,行程名 TokenBar-portable,~30MB RAM)

## 實測過的關鍵事實
- Claude `allow_token_refresh` 已由使用者啟用(`%APPDATA%\TokenBar\settings.json`),refresh 實測成功、原子寫回 `.credentials.json`、Claude Code 登入未受影響。Claude 四條限制顯示真值(當時:5h 88% Near、週 45%)。
- Codex 最新 session 檔停在 7/4:5h 視窗 → 0% Idle;週視窗(~7/11 重置)→ 12% Stale。此為本機來源能給的最誠實答案。
- `seven_day_opus` 該次回應為 null(未顯示 Opus 條)— 正常,API 沒給就不顯示。

## 待辦 backlog(使用者尚未要求,提案性質)
1. island near 態是否補回 runway 文字(`~22m`)— 目前照設計截圖只有膠囊+%,使用者若想要一行可加回。
2. Stats 頁帳號 email/方案目前是佔位(`—`)— 可從 `.claude/.claude.json` 或 Codex `auth.json` 讀真值。
3. Codex `credits` 欄位(plus 帳號為 null)— 有值時可加第三條「Credits」限制。
4. 分析的 Codex 逐日歸因很粗(整個 session 累計記在 mtime 那天)— 可改逐 `token_count` 差分歸因。
5. 通知門檻/鎖定實際觸發還沒真實演練過(邏輯已實作+抑制 30 分鐘)。
6. 開機啟動時「僅系統匣、不彈 island」選項。
7. **runway 的平滑窗長是產品決策,尚未拍板**(2026-07-15 提出)。`compute_runway` 取 15 分鐘窗的 front/back 兩點,等於「過去 15 分鐘的平均燃速」。因此閒置 14 分鐘後突然開燒,runway 要約 15 分鐘才會完全反映新速度,這段期間偏樂觀;反之剛燒完就停手則偏悲觀。**這是平滑的固有取捨(延遲 vs 雜訊),不是 bug** —— coding agent 的用量本來就是脈衝式(送一次訊息燒一波、然後讀個兩分鐘),若改用瞬時斜率,runway 會在「5 分鐘!」與「閒置」之間劇烈跳動,那是雜訊不是訊號。規格 §4.3 只說「照目前速度」+ 一律標 `~`,**未定義「目前」的窗長**。
   - 要調的話選項有:縮短窗長(更靈敏、更跳)、EWMA(降低延遲但保留部分平滑)、或窗內最小平方回歸。**任何一種都會改變正常路徑的數字**,屆時 `engine.rs` 的 golden 測試 `regular_sampling_runway_is_unchanged`(現釘 7515s)必須刻意重算基準值,不是把它刪掉。
   - 注意勿把此項與已修的空窗 bug(c108176)混為一談:那個是窗**無上限**導致斜率被 2 小時前的樣本稀釋(26.9 倍),屬於設計意圖被違反;本項是設計意圖本身該不該調整。

## 若要繼續開發
先讀 `CLAUDE.md`(指令、port 1420 互斥、機密鐵則),行為規格在 `Ai_Assistant/TokenBar UX Spec v3.md`,資料層事實在 `Ai_Assistant/data-sources-findings.md`。前端 mock 情境切換在瀏覽器 preview 的 devbar。
