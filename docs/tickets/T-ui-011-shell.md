# T-ui-011 — Shell：面板外框、StatusPill、SectionHeader、BottomBar、segmented、選單
status: done

`視覺遷移模式。只改外觀 / tokens / 共用元件樣式。禁止改 API、資料流、route path。服從 DESIGN-SPEC.md。`

> 依賴：T-ui-010 done。

## 目標

面板骨架換成編輯部風：外框（10px 圓角、shadow-panel、1px border）、頂部 StatusPill 列、編號式 SectionHeader（01/02 + 大寫標題 + serif 斜體副句）、BottomBar（時鐘+動作鈕）、segmented 控制（黑白反白選中）、設定選單。

## 範圍（只准動這些檔案）

* `src/styles.css`（shell/共用元件樣式；island 區塊禁改）
* `src/panel.ts`（僅 header/section/底列的 markup 與 class，不動資料流）
* `src/contextmenu.ts`（選單樣式 class）
* `src/i18n.ts`（若需新增 editorial 副句文案 key；en/zh-TW 齊）
* `src/panel.test.ts`（受影響斷言同步）

## 規格

照 DESIGN-SPEC §共用元件清單之 StatusPill / SectionHeader / BottomBar / Menu 列與字級表：

1. StatusPill：24px `#18181B` 方塊 + 粉紅閃電（既有 app glyph 可沿用單色化進方塊）；右側狀態膠囊 `{狀態色}12` 底、`{狀態色}30` 框、1.5px ping 慢脈動點（2.5s）、11px/700 tabular「N% left」。
2. SectionHeader：pt-28/pb-16、頂髮絲線；編號 11px/600 faint + 標題 13px/700 uppercase +0.16em；副句 15px serif italic `#52525B`（i18n key，zh-TW 也走 serif italic 排版但文案中文）。
3. BottomBar：左綠點 3s 脈動 + 時鐘 10px tabular；右三鈕 28px、hover `#F4F4F5`、active scale .95。
4. segmented（tabs/range/subtab 共用）：未選 `#71717A`、選中 `#09090B` 底 `#FAFAFA` 字、6px 圓角。
5. Menu（contextmenu）：白底、`#E4E4E7` 1px 框、6px 圓角、shadow 不得超過 shadow-panel、條目 11px、選中左緣 2px 粉紅。
6. 高度契約：面板總高與 `#analytics` 300px 不變；380px 無橫向溢出。

## SPEC / PLAN 依據

* DESIGN-SPEC §共用元件清單、§字級、§間距、§Do/Don't

## Out of scope（這張票不碰）

* GaugeCard（201）、Usage 圖表（202）、share（203）、island、後端

## Build / Verify

    檢查:   npm test && npm run build && cargo test --manifest-path src-tauri\Cargo.toml

驗收：

| 開哪個 URL | 做什麼 | 期望看到 |
| --- | --- | --- |
| http://localhost:1420 | 看頂列/區段頭/底列/選單/分頁切換 | 對齊 SPEC 樣式；devbar 各情境膠囊色正確；無 console 錯誤 |

### Attempt 1

    ERROR: Selected model is at capacity. Please try a different model.
    （gpt-5.6-sol 容量滿載，非票內容問題；工作區已清回乾淨狀態重試）
