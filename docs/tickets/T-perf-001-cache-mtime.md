# T-perf-001 [perf] 分析快取指紋改依 session log mtime 失效（取代純 TTL）

先讀 `AGENTS.md` 硬邊界與 `CLAUDE.md` 鐵則。來源：`docs/Atoll 資源優化.md` §未做第 2 項。

## 模式宣告
只實作本票行為與資料。可沿用現有樣式。禁止大規模 redesign。不碰機密憑證函式。違反範圍白名單＝作廢重來。

## 現況 → 問題
分析結果目前只靠 **Rust 端 TTL 600s** ＋ 手動 ⟳ 清快取（`lib.rs` Stage 1B，`get_analytics` ~L248-283；cache key 已含 sources）。缺點兩面：
- **偏舊**：TTL 內即使使用者剛跑 Claude/Codex 產生新用量，也要等最多 600s 或手動 ⟳ 才會反映。
- **白算**：TTL 到期後即使來源檔沒變，仍冷算一次整份 analytics。

## 目標
給快取加一個「來源指紋」：來源 session log 檔沒動 → 一直吃快取（比 600s 更久也行）；一動到 → 立刻失效重算。TTL 從「主要失效機制」降級成安全上限。

## 範圍（只准動這些檔案）
- `src-tauri/src/lib.rs`（Stage 1B 快取層：指紋比對 + 失效邏輯）
- `src-tauri/src/analytics.rs` 或相關 provider 檔（僅在需要蒐集來源檔路徑/mtime 時）

## 規格
1. **指紋定義**：對當前 sources 選集實際會讀的每個檔（Claude `~/.claude` 下的 usage jsonl、Codex 快照/live 對應檔），取 `(path, mtime, len)` 組成一個可比對的指紋值（tuple/hash 皆可）。檔案不存在也要能穩定表示（None/0），不得 panic。
2. **命中規則**：`get_analytics(force=false)` 時，同 cache key 下若**指紋與快取一致**且**未超過 TTL 上限**→ 直接回快取；否則重算並更新快取＋指紋。
3. **TTL 角色**：保留現有 600s 當**上限保險**（防 mtime 不動但內容邏輯需要刷新的極端情況），不是主要失效條件。指紋不同時忽略 TTL 直接重算。
4. **force / ⟳ 行為不變**：`force=true` 一律重算並刷新指紋。sources 選集改變仍照現有邏輯清相關快取。
5. **不得讀取 token 內容**：只看檔案 metadata（mtime/len），嚴禁為算指紋而讀入或印出憑證/token 內容（CLAUDE.md 機密鐵則）。
6. 併發：沿用現有 same-key coalesce，指紋計算要放在能被 coalesce 覆蓋的位置，避免每個請求各自 stat 一輪造成抖動可接受但別重算。

## Out of scope
- 不動 provider 掃描/解析語意、engine、ranking、成本/計數邏輯。
- 不動前端 `analyticsCache`（FE 仍照 range|filter key；本票只做後端指紋）。
- 不改設定 UI。

## Build / Verify（commit 前必過）
    測試: cargo test --manifest-path src-tauri/Cargo.toml   （PATH 先加 %USERPROFILE%\.cargo\bin）
    型別: npx tsc --noEmit
    前端: npm test
    建置備註(本機): ureq 用 default-features=false, features=["json","native-tls"]（避 ring/lib.exe 編譯失敗）；
                    建議 $env:CARGO_TARGET_DIR = "C:\Users\<you>\cargo-targets\atoll" 再建置。

驗收：

| 情境 | 做什麼 | 期望 |
| --- | --- | --- |
| 來源沒變 | 連開 Usage 多次、跨 600s | 吃快取、不冷算（`TOKENBAR_DEBUG=1` 看 `[tb]` 無重算）；比舊行為更少算 |
| 來源有變 | 跑一次 Claude/Codex 讓 log mtime 更新後開 Usage | 不必等 TTL / 不必按 ⟳ 就反映新值 |
| ⟳ | 按重整 | 一律重算 |

## 回鏈
- 來源: `docs/Atoll 資源優化.md` §未做-2

## 硬邊界
只動快取失效機制與指紋蒐集。行為變更僅限「何時重算」，輸出的 analytics 數值/schema 不變。碰機密表面＝作廢。
