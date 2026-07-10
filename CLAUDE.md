# TokenBar — 專案指引

Windows 常駐的 AI coding 額度 runway 監控器(Tauri 2 + vanilla TS)。監控 Claude Code 與 Codex 的 5h/週限制,island pill + 系統匣燃料膠囊 + 展開面板 + 用量分析。

## 真相來源(改任何行為前先讀)
- `Ai_Assistant/TokenBar UX Spec v3.md` — 行為/狀態機/演算法的唯一真相(§編號在程式註解中被引用)
- `Ai_Assistant/data-sources-findings.md` — 兩個 provider 的實測資料 schema 與安全注意
- `Ai_Assistant/HANDOFF.md` — 目前進度快照與待辦
- 目錄約定:`src/`+`src-tauri/` = 程式碼、`Ai_Assistant/` = AI 產出文件與規範;安裝檔在 repo 外同層 `..\TokenBar-release\`

## 常用指令
- 測試:`cargo test --manifest-path src-tauri\Cargo.toml`(PATH 需先加 `%USERPROFILE%\.cargo\bin`)
- 開發:`npm run tauri dev`(設 `TOKENBAR_DEBUG=1` 可在 stderr 看 `[tb]` 每輪數值)
- 打包:`npm run build:release` → 安裝檔集中複製到 repo 外的 `..\TokenBar-release\`(NSIS/MSI/免安裝 exe;原始產物仍在 `src-tauri\target\release\bundle\`)
- 圖示重生:`npm run tauri icon src-tauri/icon-source.png`

## 鐵則與陷阱
- **Port 1420 互斥**:`tauri dev` 與瀏覽器 preview 都要 1420(tauri.conf devUrl + vite strictPort),同時只能跑一個;打包前先 `taskkill /IM tokenbar.exe /F` 與 `taskkill /IM TokenBar-portable.exe /F`(exe 檔案鎖;`..\TokenBar-release\` 的免安裝版行程名是 TokenBar-portable)。
- **瀏覽器 preview = mock 模式**:非 Tauri 環境自動用 `src/mock.ts` 的情境(devbar 可切 safe/near/locked/degraded/stale/empty);驗證 UI 用 `preview_eval` 查 DOM,screenshot 常因 1s tick 逾時。
- **機密**:`~/.claude/.credentials.json` 與 `~/.codex/auth.json` 的 token 任何情況不得印出/寫 log(部分遮蔽也不行)。
- **Codex 本機來源數值語意**(providers/codex.rs,勿回退):快照 `resets_at` 已過 → util=0 + Idle;視窗未到期但檔案 >15min 舊 → 保留最後已知值 + Stale。本機快照只在使用者跑 Codex 時更新;需要即時值可在設定切 `codex_usage_source` 為 live/auto(providers/codex_live.rs,唯讀查詢)。
- **Claude refresh 是 opt-in**(settings `allow_token_refresh`,預設 false):refresh 會輪替 token,已實作原子寫回並實測不影響 Claude Code 登入,但保持使用者自選;設定改動即時生效(排程器每輪重讀)。
- 前端顯示一律 `% left`(剩餘/油量隱喻,膠囊與橫條填剩餘);內部 canonical 與排名一律 util%(已用)。
- 視覺:Geist/Geist Mono 由原型 bundle 抽出(`public/fonts/` + `src/fonts.css`),調色盤 token 在 `src/styles.css` :root,對齊 Live Island 原型。
