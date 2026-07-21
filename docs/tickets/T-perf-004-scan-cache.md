# T-perf-004 — 增量掃描快取:未變檔案跳過重解析
status: done

> 實作偏離備註(2026-07-21,詳 HANDOFF 與 scan_cache.rs 檔頭):① 快取存 per-file「解析後事件」而非規格 1 的 per-file 聚合——跨檔去重(規格 5)在聚合形態下無法正確(加總裡減不出重複 key 的貢獻),事件形態可重播 production booking 邏輯得 byte-identical 結果。② 指紋 hash 用 std DefaultHasher 固定 seed 而非 SHA256(本地一致性檢查非安全邊界,守住只加 flate2 一行依賴的約束)。③ 成本刻意不入快取(book 時以當輪 pricing override 重算),改價無需失效快取。待真機驗收:峰值 RSS 不倒退、TOKENBAR_DEBUG=1 兩輪 hit/parsed 行為。

`只實作本票行為與資料。可沿用現有樣式。禁止大規模 redesign。對照 PLAN flow。`

> 來源:Nanako0129/TokenBar(tokscale-core message_cache.rs)借鏡評估(2026-07-21)。對手做法:per-file 指紋(取樣 hash 而非全檔 hash)+ 磁碟快取 + schema 版本號,parser 改版自動整批失效。
> 現況:analytics 每輪 refresh 重掃 `~/.claude` / `~/.codex` 全部 log(已有 tail-read 減量);log 隨使用累積,掃描成本線性成長。記憶體優化輪(v0.9.3)的峰值 RAM 成果不可倒退。

## 目標

同一檔案內容沒變就不重解析:掃描改為「指紋比對 → 命中用快取聚合結果、未命中才解析」。**正確性優先**:快取命中路徑與全量重掃路徑的最終數字必須逐位一致。

## 範圍(只准動這些檔案)

* `src-tauri/src/analytics.rs`(掃描入口接快取層)
* `src-tauri/src/scan_cache.rs`(新檔:指紋、序列化、失效)
* `src-tauri/src/lib.rs`(僅 wiring)

## 規格

1. **粒度**:per source file(Claude 每個 session JSONL、Codex 每個 rollout JSONL)快取「該檔解析後的聚合中間產物」(per-file 的 daily/hourly/byModel/byKind/byProject 局部累計,型別沿用現有聚合結構)。合併多檔局部累計 = 現行全掃結果。
2. **指紋**:`(檔案大小, mtime, 頭 4KB SHA256, 尾 4KB SHA256)`。任一不符 → 重解析該檔。追加寫入的活躍檔(尾 4KB 必變)天然失效,符合 JSONL append 型態。
3. **磁碟格式**:`%LOCALAPPDATA%\Atoll\scan-cache.json.gz`(serde_json + flate2;**不引入 bincode**,新依賴僅 flate2,若 Cargo.toml 已有壓縮 crate 則沿用)。頂層帶 `schema: u32`;**任何動到解析/聚合邏輯的未來票都必須 bump schema**(在檔頭註解寫死這條規則)。schema 不符或檔壞 → 整批丟棄重建,不炸。
4. **上限與修剪**:快取檔 >32MB 或條目對應的 source 檔已消失 → 修剪/重建。寫入原子(temp + rename),頻率每輪最多一次,且僅在本輪有新解析時寫。
5. **跨帳去重注意**:T-fix-001 的 Claude 全域去重(requestId/message.id/uuid HashSet)是跨檔狀態——per-file 快取必須把「本檔貢獻的 dedup keys」一起存,合併時先重建全域集合再去重,否則 resume/fork 副本會在快取命中路徑重複計數。**這是本票最大的正確性風險,測試必須覆蓋。**
6. **記憶體**:載入快取後峰值 RSS 不得高於現行全掃(快取是聚合產物不是原始訊息,量應遠小);驗收實測。
7. 設定不加開關(行為透明);`TOKENBAR_DEBUG=1` 時 stderr 印 `[tb] scan cache: N hit / M parsed`。

## 測試

- 同一 fixture 集:全掃 vs「第一輪建快取、第二輪全命中」→ 聚合結果逐欄位相等(黃金測試)。
- fixture 檔追加一行 → 只該檔重解析,總結果 = 全掃。
- 跨檔重複 requestId(fork 情境)→ 快取路徑不重複計數(規格 5)。
- schema bump → 快取整批失效重建。
- 快取檔壞/截斷 → 靜默重建,結果正確。

## Out of scope(這張票不碰)

* providers/(Limits 路徑本來就輕,不快取)
* 掃描排程/節流邏輯(scan_coord.rs 不動)
* 不做 watch/inotify(維持輪詢)

## Build / Verify

    前置:   PATH 加 %USERPROFILE%\.cargo\bin
    檢查:   cargo test --manifest-path src-tauri\Cargo.toml
    前端:   npm test && npm run build

驗收:

| 檢查 | 做什麼 | 期望看到 |
| --- | --- | --- |
| cargo test | 後端測試 | 黃金測試(快取 vs 全掃逐欄位相等)+ dedup 跨檔測試全過 |
| 手動 | TOKENBAR_DEBUG=1 連續兩輪 refresh | 第二輪幾乎全 hit,數字與第一輪一致 |
| 實測 | 工作管理員峰值 RSS | 不高於 v0.9.3 現值 |
