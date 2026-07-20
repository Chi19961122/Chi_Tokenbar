# T-perf-002 [perf] 離開分析頁時主動拆大 DOM（不再只靠 hide/destroy）

先讀 `AGENTS.md` 硬邊界與 `CLAUDE.md` 鐵則。來源：`docs/Atoll 資源優化.md` §未做第 3 項。

## 模式宣告
只實作本票行為與資料。可沿用現有樣式。禁止大規模 redesign。對照現有 view 切換 flow。違反範圍白名單＝作廢重來。

## 現況 → 問題
分析頁的大 DOM（圖表 / 月熱力圖 / tiles）目前只在整個 WebView **hide/destroy**（閒置卸載）時才釋放。使用者在視窗**仍開著**但切到別的 view（Limits／Settings 整頁）時，`#analytics` 那坨重 DOM 仍掛在記憶體。

## 目標
離開分析 view 時主動清掉 `#analytics` 的重 DOM 釋放記憶體；回到分析頁時從既有快取重繪（`renderAnalyticsInto` 已存在，不必重打 IPC）。

## 範圍（只准動這些檔案）
- `src/main.ts`（view 切換 / 分析層 mount-unmount）

## 規格
1. **卸載點**：在切離分析 view 的既有切換函式裡，把 `#analytics.innerHTML` 清空（連 skeleton），釋放圖表 canvas/DOM。找現有控制哪個 view 顯示的地方掛勾，不要新造一套路由。
2. **回頁重繪**：回到分析 view 時，若 `analyticsCache` 有當前 range 的 payload → 直接 `renderAnalyticsInto` 重繪（無 IPC、無閃 skeleton 過久）；沒有才走現有 fetch + skeleton 路徑。
3. **量測高度**：`#analytics` 是 mode-locked 固定高度盒（見 `analytics-height`）；清空/重繪不得破壞視窗量測或造成高度跳動。清空後若視窗仍要維持該區塊高度，保留容器、只清內容。
4. **狀態一致**：`ui.range` / 子 tab 等選取狀態不因卸載遺失；回頁沿用。
5. 不動閒置 hide/destroy 既有邏輯（那是 WebView 層，本票是同一視窗內的 view 層），兩者要能共存不打架。

## Out of scope
- 不動後端 / IPC / analytics 計算。
- 不動閒置卸載（`lib.rs` idle destroy）與 poll 排程。
- 不改視覺樣式 / tokens。

## Build / Verify（commit 前必過）
    型別: npx tsc --noEmit
    前端: npm test
    手驗: npm run tauri dev  （TOKENBAR_DEBUG=1 可看 [tb]）
    Port 1420 互斥：同時只跑一個 dev/preview。

驗收：

| 做什麼 | 期望 |
| --- | --- |
| 開 Usage → 切 Limits/Settings | `#analytics` 內容被清空（DevTools 查無圖表節點），記憶體工作組下降 |
| 切回 Usage（10 分內、來源沒變） | 秒重繪，不冷算、不長時間 skeleton |
| 反覆切來切去 | 無高度跳動、無狀態遺失、無殘留舊圖 |

## 回鏈
- 來源: `docs/Atoll 資源優化.md` §未做-3

## 硬邊界
只動前端 view 卸載/重繪。行為變更僅限「離頁釋放 DOM、回頁自快取重繪」，不改數值、樣式、後端。
