# DESIGN-SPEC — <專案名稱>

> 這份是「合同」：跟任何文件、圖、skill 打架時，以這份為準。所有數值**從 Stitch 匯出碼抽，不准看圖用肉眼猜**；圖只拿來理解意圖。

## 來源

* Stitch 匯出碼路徑: `design/stitch/export/`
* Stitch design.md: `design/stitch/design.md`
* 選定方向: <A|B|C>（要跟 PLAN 的 chosen_direction 一致）
* 參考圖: `design/refs/`

## Design Tokens

### 色（寫 hex）

| Token       | 值   | 用途     |
| ----------- | --- | ------ |
| primary     | `#` | 主行動、重點 |
| secondary   | `#` |        |
| bg          | `#` | 頁面底    |
| surface     | `#` | 卡片、面板  |
| text        | `#` | 主文字    |
| text-muted  | `#` | 次要文字   |
| border      | `#` |        |
| destructive | `#` | 刪除、危險  |

### 字級

| Token   | 大小 / 行高 / 字重 | 用途   |
| ------- | ------------ | ---- |
| display |              | 大標   |
| h1      |              |      |
| body    |              | 內文   |
| label   |              | 表單標籤 |
| caption |              | 輔助小字 |

### 間距階（4～5 級，取好名字）

| Token   | 值   |
| ------- | --- |
| space-1 |     |
| space-2 |     |
| space-3 |     |
| space-4 |     |

### 圓角、陰影（各不超過 3 級）

| Token               | 值   |
| ------------------- | --- |
| radius-sm / md / lg |     |
| shadow-sm / md / lg |     |

## 共用元件清單（名稱對齊元件庫，如 shadcn `Button` variant）

| 元件     | 狀態要寫齊                                | 備註  |
| ------ | ------------------------------------ | --- |
| Button | default / hover / disabled / loading |     |
| Input  | default / focus / error / disabled   |     |
| Card   |                                      |     |
| Nav    |                                      |     |
| Table  | 空資料時長怎樣                              |     |
| Modal  |                                      |     |

<本專案最複雜的那個元件，狀態矩陣單獨展開寫。>

## 頁面 → 元件對照表

| 頁面（route） | 用到的元件 | 區塊順序 |
| --------- | ----- | ---- |
| `/`       |       |      |

## 每頁三態

| 頁面  | loading              | empty（沒資料）    | error       |
| --- | -------------------- | ------------- | ----------- |
| `/` | <skeleton / spinner> | <提示文案 + 引導動作> | <錯誤訊息 + 重試> |

## 深色模式與 RWD

* 深色模式: <做 / 不做>（寫死，防做一半）
* RWD 斷點: <支援哪幾檔>
* 手機上 nav 怎麼收:
* 手機上 table 怎麼收:

## Do / Don't（最多 5 條）

* Do:
* Don't: <例：紫色漸層、三等分卡片、ghost 當主行動>

## 對比度自檢

* [ ] 主文字對背景 ≥ AA
* [ ] 按鈕文字對按鈕底 ≥ AA
* [ ] muted 文字仍可讀

* * *

## （情境 B 專用）舊 → 新對照表

> Codex 照這張表機械替換，不自由發揮。

| 舊 token / 類名 | 新 token | 備註  |
| ------------ | ------- | --- |
|              |         |     |

* * *

## 填完自檢（全勾完才算定稿）

* [ ] 色 / 字級 / 間距 / 圓角陰影全部從匯出碼抽的
* [ ] 元件狀態矩陣至少 Button + Input + 最複雜那個
* [ ] 每頁三態都有寫
* [ ] 深色模式、RWD 斷點寫死了
* [ ] Do/Don't ≤ 5 條
* [ ] 匯出路徑、參考圖路徑在「來源」節
* [ ] 跟 PLAN 的 chosen_direction 一致
* [ ] （情境 C）新功能入口、角色可見性有對應 PLAN
