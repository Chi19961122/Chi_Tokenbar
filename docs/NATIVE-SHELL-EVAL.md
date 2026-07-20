# 原生殼重寫評估（決策閘門）

| | |
| --- | --- |
| 日期 | 2026-07-20 |
| 問題 | 為砍常駐 RAM，值不值得把 Atoll 從 WebView 殼改成原生殼（egui / Slint）？ |
| 方法 | `scripts/measure-memory.ps1`：走 PPID 抓行程樹，加總 `WorkingSet64` + `PrivateMemorySize64`(=private commit)。冷啟 settle ~15-20s 取尾樣本。 |

## 實測（當前建置，同法可比）

| 建置 | 狀態 | 行程數 | WS | Private(commit) |
| --- | --- | --- | --- | --- |
| Tauri **v0.9.3** | 冷啟/隱藏(webview 仍在) | 7 | ~344MB | **~126MB** |
| Tauri v0.9.2 | 膠囊顯示(shown,先前輪) | 7 | ~372MB | ~135MB |
| Tauri v0.9.1 | shown(trim 前) | 7 | ~434MB | ~219MB |
| **egui 空殼** | 膠囊顯示 | 1 | ~136MB | **~100MB** |

egui 空殼＝只有膠囊 + 額度列表、全 mock，**無** API/掃描/analytics/tray/share。

## 結論

1. **egui 重寫 private 只省 ~26–35MB。** egui 的 GL context + 字型 atlas 自己就 ~100MB private 打底，抵銷掉拿掉 WebView 的大部分好處。
2. **WS 差(~208MB)是假象。** Chromium 多行程的共享/可回收頁被每個行程各算一次；private commit(真實額外成本)差很小。使用者在工作管理員看到「差不多」正是此故。
3. **省 RAM 大頭早已用便宜招數吃掉**：v0.9.2 webview trim(Private -38%, 219→135)＋資源優化輪的 idle-destroy(隱藏久了 destroy webview → 近 tray-only，比任何**常駐**原生殼還低)。
4. **egui 重寫成本 vs 效益完全不成比例**：要在 immediate-mode egui 重建整個前端 + providers + analytics(圖表/月熱力圖硬骨頭) + UX Spec v3 狀態機 + Geist/glass 視覺，換 ~30MB。**否決 egui 重寫。**

## 唯一還值得的原生路：Slint（未實測）

只有「膠囊**恆常顯示**、且 RAM 是硬需求」時，原生殼才有意義。此情境下 Slint 是唯一候選：
- 軟體渲染，**無 GL context / 字型 atlas GPU 打底** → private 有機會壓到 egui 的 100MB 以下。
- 宣告式 UI，視覺還原比 egui 容易。
- 先前輪估 WS 50~90MB，但**是估算，非實測**。

**閘門規則：不得憑估算決定重寫。** 要繼續，先做一張最小 Slint 膠囊 spike（同 `measure-memory.ps1` 量顯示/隱藏兩態），拿真數字對比 Tauri v0.9.3，才談整體重寫。

## 現況決定

- egui prototype：凍結 spike，`atoll-egui/` 保留、README 已寫 abandon/freeze。
- Tauri 續為正式路；RAM 便宜招數已到頂，再降只能等 Slint spike 的真數字。
