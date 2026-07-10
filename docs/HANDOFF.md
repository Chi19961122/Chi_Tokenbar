# HANDOFF — 進度快照(2026-07-10)

## 目前狀態:全部里程碑完成,修正版已打包並在跑

- M0–M7 全部完成(scaffold、Codex provider、burn-rate 引擎、Live Island 視覺、Anthropic provider+降級、面板下鑽、第三層分析、通知/設定/autostart)。
- Polish 完成:island 拖曳+吸邊、tray rich tooltip、綠色方塊 logo(`src-tauri/icon-source.png`)。
- **數值修正完成**(使用者回報數值不準的 bug):Codex 快照過期/陳舊語意,見 CLAUDE.md 鐵則。19/19 cargo 測試通過。
- **手動更新完成**(2026-07-10):面板 header 新增 ⟳ 按鈕(Tauri 指令 `refresh_now` → mpsc channel 喚醒排程器,Claude 快取視同過期、5s 防連打下限)與「X 前更新」標籤(每秒刷新)。參數總表新增 `docs/CONFIG.md`。
- **視窗改造完成**(2026-07-10):預設停靠右下角(work area,避開工作列);展開面板高度自動符合內容(右下角錨點向上長,fitWindow + resizeAnchored);吸邊新增底邊;header 新增 ⊟/⊞ 精簡模式切換(settings.json `compact`,只顯示額度列表)。
- **推上 GitHub**(2026-07-10):公開 repo `Chi19961122/Chi_Tokenbar`(origin/main),含 MIT LICENSE + 重寫 README(實機截圖在 docs/screenshots/)。註:工作目錄有一份壞掉的 `AGENTS.md`(Claude→Codex 誤植,未追蹤、未推),待決定修/刪。
- **git 版控啟用**(2026-07-10):main 分支,初始 commit e043b2e(116 檔);島嶼顯示選項移除「自動(最危險)」,僅剩並排/僅 Claude/僅 Codex(舊存檔值 worst 一律 fallback 成並排)。
- **島嶼第三輪微調**(2026-07-10):右側輔助改為今日燒速 tok/min(移除 ↻ 倒數與總量);供應商識別改用品牌 icon,島嶼與面板分組標題都套用;Claude 主題色從綠改為品牌橘 `--claude` #d97757。icon 改用 lobehub/lobe-icons v1.91.0 官方 SVG(claude-color/codex-color),vendor 在 src/assets/ 本地打包、Codex 白底板移除(手繪版已淘汰)。**陷阱已修**:SVG 漸層 id 是文件全域,隱藏的島嶼副本會搶走 id 且 display:none 內的 defs 不生效 → 面板 Codex 雲朵無填色;icons.ts 現在每個實例注入唯一 id 後綴。
- **高度鎖定 + 島嶼強化**(2026-07-10 第二輪回饋):自動縮放改為「進入模式時量一次後鎖定」(展開/切精簡/開關設定才重算),點分頁與每秒 tick 不再 resize → 消除卡頓;#analytics 固定 300px 讓所有分頁同高;移除捲軸(overflow hidden)。島嶼改為可配置(settings `island_mode`,預設 both):Claude/Codex 並排膠囊(各取該供應商最危險一條)+ ↻重置倒數 + 今日總 tokens(60s 更新);視窗 collapsed 寬 340(並排)/270(單一)。
- **usage API 已改版 + Fable 顯示完成**(2026-07-10):API 新增結構化 `limits` 陣列(session/weekly_all/weekly_scoped),`parse_limits_array` 通用解析(Opus 沿用 `cc.opus`,其他模型 scoped 週限制 → `cc.w.<slug>`,如 Fable → `cc.w.fable`「Weekly · Fable」),舊欄位當 fallback。dev 實測 API 回傳 Fable 6% 正常顯示。23/23 cargo 測試通過(新增 4 個解析測試)。schema 詳見 data-sources-findings.md。
- Release 產物(2026-07-10 11:33,含手動更新/視窗改造/模式鎖定高度/精簡模式/島嶼並排+lobe-icons 官方品牌 icon+漸層 id 修正/Fable):
  - `src-tauri\target\release\bundle\nsis\TokenBar_0.1.0_x64-setup.exe`(推薦安裝)
  - `src-tauri\target\release\bundle\msi\TokenBar_0.1.0_x64_en-US.msi`
  - `src-tauri\target\release\tokenbar.exe`(免安裝,新版已啟動常駐,~27MB RAM)

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
先讀 `CLAUDE.md`(指令、port 1420 互斥、機密鐵則),行為規格在 `docs/TokenBar UX Spec v3.md`,資料層事實在 `docs/data-sources-findings.md`。前端 mock 情境切換在瀏覽器 preview 的 devbar。
