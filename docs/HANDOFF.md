# HANDOFF — TokenBar（2026-07-18 交接）

> 換新對話用。讀完這份 + `docs/ROUND-v080.md` + `docs/FEEDBACK.md` 就能接手。流程走 `frontend-drive` skill 的迭代輪（階段 R）。

## 一句話現況

v0.8 輪只剩**分享（Share）改版**未做（T-913 定案 → T-914+T-915 實作）；其餘八項回饋全數提交完成。main 領先 origin/main **73 個 commit、未推、未打包**。package.json 版號還是 **0.6.0**，收尾時要 bump **0.8.0**。

## 環境（動手前必讀）

- **不要殺使用者正在跑的 dev**：`tokenbar.exe` + vite(port 1420) 由使用者自己的 `npm run tauri dev` 實例管理。子代理要開 mock preview 一律用 `--port 5200+`。
- 背景 shell 先 `export PATH="$HOME/.cargo/bin:$PATH"`，否則 cargo 找不到。
- 三套檢查：`npx tsc --noEmit -p tsconfig.json`／`npx vitest run`（現 129）／`cargo test --manifest-path src-tauri/Cargo.toml`（現 176）。
- codex 派工：`nohup bash -c 'codex exec -s workspace-write -c model_reasoning_effort="…" --skip-git-repo-check "$(cat docs/tickets/T-xxx-brief.md)" </dev/null > log; echo $? > exit標記' & disown`，waiter 等標記檔。
- **硬邊界**：`providers/anthropic.rs` 機密檔（secret-neutral，不碰憑證函式）；`providers/codex.rs` 快照語意；island `--island-*` 與 `.island` 樣式；六款分享卡自帶 `--share-*`（主題不變）；§0 隱私＝專案名不進分享面。

## 待辦：分享改版（唯一未結）

### 先等使用者拍板四個設計決定（HTML 比稿在 `design/refs/share-redesign-preview.html`，直接開瀏覽器看）

1. **卡片額度 %**：用 app 慣例的「剩餘 %」（62/58/45），還是比稿現畫的「已用 %」（38/42/55）？
2. **額度儀表**：只放 Island Card 一款，還是更多款也放？
3. **署名日期格式**：月年（`JUL 2026`）還是完整日期（`2026.07.18`）？
4. **六款方向取捨**：statement/diagnostics/minimal/fuel/island_card/wa 哪些過、哪些要改？

### 定案後開兩張工單（一批做，同動 share 檔，序列不並行）

- **T-914 [arch]**：「戰報」全面改稱「分享」；從分析頁 subtab 移出 → header 齒輪旁新增分享 icon，開啟為**整頁模式**（架構同 T-902 設定整頁：`body.share-open` 隱藏其他、先渲染後 fitWindow、頁籤可逃出）。注意 i18n 的 `subtab.report` 等 key 與現有 report subtab 的移除。
- **T-915 [visual]**：六款模板照定案重設計實作進 `src/share.ts` + `src/share.css`（16:9 + 9:16 都要）。保持主題不變（只用 `--share-*`）、§0（不讀 by_project）、匯出解析度（auto 1200×675、story 1080×1920）。

收完 → 總驗收（起 dev、使用者真機驗收 T-906 預覽視窗/島嶼直式/分享新版面）→ bump 0.8.0 → `npm run build:release`（安裝檔落 `..\TokenBar-release\`）。

## 本輪已完成（v0.7 + v0.8，供追溯）

v0.7（`6c2130a`起）：雙主題、設定整頁、控件 seg 化、分析高度解耦、島嶼直式、9:16 分享卡、預覽點擊放大、分析垂直節奏。
v0.8：T-908 拆分頁底部被擋（flex-fill）、T-909 暗色綠降亮、T-910 更新頻率+429退避、T-911 圖表 X/Y 軸+hover、T-912 八種活動類型、T-916 五源多選、**T-918 Grok 退回 usage-only**（限額卡移除，前端渲染鏈沉睡待 xADI 額度 API；見 FEEDBACK.md）。

## 待議小項（非阻斷）

- 月成本 tile 大值（如 `$25.37K`）在 accent tile 會貼邊——既有問題，可另開 responsive value sizing 工單。
- T-906 預覽視窗、島嶼直式排列：codex/mock 無法驗，靠使用者真機確認觀感。
