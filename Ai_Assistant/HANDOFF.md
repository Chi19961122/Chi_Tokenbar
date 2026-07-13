# HANDOFF — 進度快照(2026-07-13)

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
- **git 版控啟用**(2026-07-10):main 分支,初始 commit e043b2e(116 檔);島嶼顯示選項移除「自動(最危險)」,僅剩並排/僅 Claude/僅 Codex(舊存檔值 worst 一律 fallback 成並排)。
- **島嶼第三輪微調**(2026-07-10):右側輔助改為今日燒速 tok/min(移除 ↻ 倒數與總量);供應商識別改用品牌 icon,島嶼與面板分組標題都套用;Claude 主題色從綠改為品牌橘 `--claude` #d97757。icon 改用 lobehub/lobe-icons v1.91.0 官方 SVG(claude-color/codex-color),vendor 在 src/assets/ 本地打包、Codex 白底板移除(手繪版已淘汰)。**陷阱已修**:SVG 漸層 id 是文件全域,隱藏的島嶼副本會搶走 id 且 display:none 內的 defs 不生效 → 面板 Codex 雲朵無填色;icons.ts 現在每個實例注入唯一 id 後綴。
- **高度鎖定 + 島嶼強化**(2026-07-10 第二輪回饋):自動縮放改為「進入模式時量一次後鎖定」(展開/切精簡/開關設定才重算),點分頁與每秒 tick 不再 resize → 消除卡頓;#analytics 固定 300px 讓所有分頁同高;移除捲軸(overflow hidden)。島嶼改為可配置(settings `island_mode`,預設 both):Claude/Codex 並排膠囊(各取該供應商最危險一條)+ ↻重置倒數 + 今日總 tokens(60s 更新);視窗 collapsed 寬 340(並排)/270(單一)。
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

## 若要繼續開發
先讀 `CLAUDE.md`(指令、port 1420 互斥、機密鐵則),行為規格在 `Ai_Assistant/TokenBar UX Spec v3.md`,資料層事實在 `Ai_Assistant/data-sources-findings.md`。前端 mock 情境切換在瀏覽器 preview 的 devbar。
