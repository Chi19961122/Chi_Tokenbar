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
| **Slint 空殼**(軟渲染) | 膠囊顯示 | 1 | **~18MB** | **~3–5MB** |

egui/Slint 空殼＝只有膠囊 + 額度列表、全 mock，**無** API/掃描/analytics/tray/share。
Slint spike 建於 `atoll-slint/`(Slint 1.17.1,`renderer-software` 確認無 GPU/skia/femtovg;PrintWindow 截圖確認膠囊正常渲染 `C 22% / X 0%`)。

## 結論

1. **egui 重寫 private 只省 ~26–35MB。** egui 的 GL context + 字型 atlas 自己就 ~100MB private 打底，抵銷掉拿掉 WebView 的大部分好處。
2. **WS 差(~208MB)是假象。** Chromium 多行程的共享/可回收頁被每個行程各算一次；private commit(真實額外成本)差很小。使用者在工作管理員看到「差不多」正是此故。
3. **省 RAM 大頭早已用便宜招數吃掉**：v0.9.2 webview trim(Private -38%, 219→135)＋資源優化輪的 idle-destroy(隱藏久了 destroy webview → 近 tray-only，比任何**常駐**原生殼還低)。
4. **egui 重寫成本 vs 效益完全不成比例**：要在 immediate-mode egui 重建整個前端 + providers + analytics(圖表/月熱力圖硬骨頭) + UX Spec v3 狀態機 + Geist/glass 視覺，換 ~30MB。**否決 egui 重寫。**

## Slint 實測翻盤（2026-07-20 已量）

egui 敗在 GL 打底；改用 **Slint 軟體渲染器**（`renderer-software`，dep tree 確認無 skia/femtovg/wgpu/glow）後：
- **膠囊顯示態 WS ~18MB / Private ~3–5MB，單行程。**
- 對比：Tauri ~344MB WS、egui ~136MB WS → **Slint 比 Tauri 少 ~95%、比 egui 少 ~87%。**
- 截圖確認真有渲染（不是空白窗）。

**這是 egui 沒有的結構性優勢**：軟渲染完全沒有 GPU driver / GL context / 字型 atlas 的私有配置，所以打底極低。

**但仍是空殼數字。** 加上真功能(providers/analytics 圖表/熱力圖/tray/settings/i18n/UX 狀態機)後 RAM 會漲，但「無 GPU/無 WebView」的結構優勢會保留——就算漲到 2–4 倍，估仍在 ~40–80MB，**遠低於 Tauri 的 135MB+**。

## 決定（2026-07-20 使用者拍板：**凍結**）

egui 否決不變。Slint 已證軟渲染 ~18MB 可行，但整體重寫成本仍大（整個前端 + IPC + analytics 渲染要在 Slint 重做）。使用者選 **③ 凍結**：保留 spike 當已證資產，暫不遷，等哪天 RAM 變硬需求再啟動。當時評估的三檔（存查）：
1. 正式立案分階段遷移 Slint（風險集中、回報大）。
2. 再做「真功能」Slint 驗證票：加 1–2 真 provider + 一張真圖表量 RAM，確認結構優勢不被真功能吃掉，再決定全遷。← 若日後解凍，這是全遷前該過的閘門。
3. **凍結（已選）**：保留 spike，暫不遷。

- egui prototype：凍結，`atoll-egui/` 保留。
- Slint spike：保留於 `atoll-slint/`（已證軟渲染 ~18MB 可行）。
- Tauri 目前仍為正式路。
