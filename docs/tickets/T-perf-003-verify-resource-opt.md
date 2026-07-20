# T-perf-003 [verify] 資源優化回歸驗收（已出貨 Tray-first / 閒置卸載 / 分析快取）

先讀 `AGENTS.md` 與 `CLAUDE.md`。來源：`docs/Atoll 資源優化.md` §驗收。純驗證票：只跑、只量、只回報 CONFIRMED / REFUTED，不改程式。

## 背景（已實作，需回歸確認）
| 項目 | 行為 | 設定/程式 |
| --- | --- | --- |
| Tray-first | 預設啟動不顯示島嶼；左鍵點系統匣開關 | `window_mode`: `tray_only` \| `island_always` |
| 閒置卸載 | 隱藏超過 N 分 destroy 主視窗；再點匣重建 | `webview_idle_min`: 0/5/10/15（預設 10） |
| 分析快取 | Rust TTL 600s；FE key 不綁 snapshot | `get_analytics(force)`；⟳ 清快取重算 |
| 閒置 poll | 視窗不可見 60s；可見 15s | `lib.rs` scheduler |
| 隱藏不掃 today | island aux 只在視窗可見時拉 analytics | `main.ts` |
設定 UI：**啟動與視窗** → 視窗模式／閒置卸載介面。存檔 `%APPDATA%\Atoll\settings.json`。

## 量測定義
主程式 + 所有 `msedgewebview2` **私人工作組**加總（Private Working Set）。

## 驗收清單（逐項 CONFIRMED/REFUTED + 數據）
1. 啟動只有系統匣，無島嶼；左鍵點匣可開／關。
2. Usage 於 10 分鐘內連開兩次 → **秒開**（吃快取）；按 ⟳ → 會重算。
3. 隱藏超過 idle 閾值後 → `msedgewebview2` 行程/RAM **下降**；再點匣可恢復 UI。
4. 僅 tray（視窗不可見）時 → CPU 近 0、poll 週期變慢（60s）。

## Build / Verify
    前端測試: npm test
    後端測試: cargo test --manifest-path src-tauri/Cargo.toml  （PATH 先加 %USERPROFILE%\.cargo\bin）
    手驗建置: $env:CARGO_TARGET_DIR = "C:\Users\<you>\cargo-targets\atoll"; npm run tauri dev
    建置備註: ureq default-features=false, features=["json","native-tls"]（避 ring/lib.exe 失敗）。
    行程量測: 工作管理員 / Get-Process 看 atoll + msedgewebview2 Private Working Set。

## Out of scope
不改任何程式；只在其中一項 REFUTED 時回報現象＋數據，由主線決定是否開修正票。

## 回鏈
- 來源: `docs/Atoll 資源優化.md` §驗收
