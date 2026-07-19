# T-932 [fix] 分析頁時間全改本地時區（root cause 見 F-15）

先讀 `docs/RUNBOOK.md` 與 `AGENTS.md` 硬邊界。使用者定案：分析頁的日/時分桶**全部改用本地系統時區**（不再 UTC）。

## 現象 → 根因
分桶目前分兩套時鐘：主圖（每日/每時長條 + 窗界 + 日標籤）走 **UTC**；`hourly_by_day` 與 records（busiest hour）走 **local**。→ UTC+8 使用者每時圖（UTC 小時）與 footnote「busiest hour」（本地小時）差 8 小時對不上；每日圖凌晨~08:00 前用量算進昨天(UTC)。根因＝`analytics.rs` 用 `chrono::DateTime::from_timestamp(ts,0)`（UTC）取日/時，而 records 那條用 `.with_timezone(&Local)`。

## 要改（全部統一到 Local）

**`src-tauri/src/analytics.rs`：**
1. **`book()`（~L328-362）**：把 `let Some(dt) = from_timestamp(ts,0)` 之後**先轉本地** `let dt = dt.with_timezone(&chrono::Local);`，讓後續 `dt.format("%Y-%m-%d")`（日桶 L337）與 `dt.hour()`（時桶 L347-348）都是本地。`hourly_by_day`（L349-352）本來就用 `local`，改成直接用這個已轉本地的 `dt`（移除多餘的第二次 `with_timezone`，兩者一致）。
2. **`date_str()`（L202-206）**：`from_timestamp(ts,0).with_timezone(&chrono::Local).format("%Y-%m-%d")`——日標籤改本地。
3. **窗界（L484-491）**：`utc_midnight` 改**本地午夜**。用 `chrono::Local::now()` 的 `date_naive().and_hms_opt(0,0,0)` 再經 `chrono::Local.from_local_datetime(&naive).single()` 取 unix ts（`use chrono::TimeZone`）；DST 不明時（`.single()` 為 None）fallback 回原 `now - now.rem_euclid(86400)`。`start = local_midnight - days_back*86400`。台灣無 DST，`start + i*86400` 經 `date_str`（本地）得正確本地日。
4. 確認沒有其他 `from_timestamp(...).format`/`.hour()` 殘留走 UTC（grep `from_timestamp`、`rem_euclid(86400)`、`Utc::now`）；`now = Utc::now().timestamp()` 當「現在的 unix 秒」保留無妨，只要日/時**格式化**走本地。

**`src/analytics.ts`：**
5. **`weekdayMon()`（~L104-107）**：`new Date(date + "T00:00:00Z")` → `new Date(date + "T00:00:00")`（本地）、`getUTCDay()` → `getDay()`；更新註解（改為「對齊後端本地日分桶」）。這樣月熱力圖的星期對齊本地日。

## 測試（analytics.rs）
6. 修會壞的 **hourly 索引斷言**（餵 `...THH:00:00Z` 斷言 `acc.hourly[HH]`）：L1553-1556、L1609、L1837-1840、L1885-1886。改成**時區無關**：加一個 test helper 產「今天本地某小時」的 rfc3339（例：`chrono::Local.with_ymd_and_hms(2026,7,17,H,0,0).unwrap().to_rfc3339()`），把那些 `"2026-07-17T0X:00:00Z"` 字面換成本地 H 小時，使 `hourly[H]` 在任何機器時區都成立。**不要**只把斷言硬改成別的索引（那會綁死跑測試機器的時區）。
7. 跨桶加總類測試（cost sum、breakdown.input、total_tokens、by_project、range_start_day vs daily index 相對比對）多半時區無關，不需動；跑過確認。`max_hour_is_one_date_hour...`（L1943）直接建 `hourly_by_day` 不經 book，不受影響。

## Build / Verify（commit 前必過）
- `cargo test --manifest-path src-tauri/Cargo.toml`（PATH 先加 `%USERPROFILE%\.cargo\bin`）全綠。
- `npx tsc --noEmit` 乾淨、`npm test`（vitest）綠。
- 語意自檢：本地 UTC+8 下，每時圖尖峰小時 = footnote busiest hour（同一時鐘）；每日「today」桶＝本地今天。

## 硬邊界
只動 `analytics.rs` + `analytics.ts`（及其測試）。不動 provider 掃描、engine、ranking、其他後端；不碰機密憑證函式。行為變更僅限日/時**歸屬時區**，不改 token 計數/成本邏輯。

## 模式宣告
一般實作（Rust 時區 + 測試），不動 secret 表面。違反範圍白名單＝作廢重來。
